use mlua::{Function, Lua, RegistryKey, Table, Thread, ThreadStatus, Value};
use signal_auras_core::{
    BindingMode, CapabilityKind, CapabilitySet, ControllerRegistration, ControllerRegistrationKind,
    ControllerRegistrationSet, DiagnosableError, ErrorPhase, HeldCondition, MacroAction,
    MouseButton, ProcessName, ScopeSelection,
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveWindowMetadata {
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LuaHostRequest {
    Sleep { duration_ms: u64 },
    Log { level: LuaLogLevel, message: String },
    ActiveWindow { include_title: bool },
    FindWindow { processes: Vec<String> },
    ActivateWindow { handle: String },
    WaitActive { handle: String, timeout_ms: u64 },
    Input { action: MacroAction },
}

impl LuaHostRequest {
    pub fn required_capability(&self) -> Option<CapabilityKind> {
        match self {
            Self::Sleep { .. } => Some(CapabilityKind::Timer),
            Self::Log { .. } => None,
            Self::ActiveWindow { .. } => Some(CapabilityKind::ActiveWindowMetadata),
            Self::FindWindow { .. } | Self::ActivateWindow { .. } | Self::WaitActive { .. } => {
                Some(CapabilityKind::WindowActivation)
            }
            Self::Input { .. } => Some(CapabilityKind::SynthesizedInput),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LuaLogLevel {
    Debug,
    Info,
    Warn,
}

impl LuaLogLevel {
    pub fn parse(level: &str) -> Result<Self, DiagnosableError> {
        match level.trim() {
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" | "warning" => Ok(Self::Warn),
            other => Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("unsupported Lua log level '{other}'"),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LuaHostResponse {
    Unit,
    Bool(bool),
    ActiveWindow(ActiveWindowMetadata),
    WindowHandle(Option<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LuaCallbackStep {
    Yielded(LuaHostRequest),
    Complete,
}

pub struct LuaCallbackCoroutine {
    thread: RegistryKey,
}

pub struct ImperativeLuaController {
    lua: Lua,
    registrations: ControllerRegistrationSet,
    callbacks: BTreeMap<String, RegistryKey>,
}

impl ImperativeLuaController {
    pub fn load_source(source: &str) -> Result<Self, DiagnosableError> {
        let lua = Lua::new();
        install_sandbox(&lua)?;

        let registrations = Rc::new(RefCell::new(Vec::new()));
        let callbacks = Rc::new(RefCell::new(BTreeMap::new()));
        install_sa_api(&lua, Rc::clone(&registrations), Rc::clone(&callbacks))?;

        let source = lua_compatible_controller_source(source);
        lua.load(&source)
            .set_name("signal-auras-controller")
            .exec()
            .map_err(lua_error)?;

        let registrations = ControllerRegistrationSet::new(registrations.take())?;
        for registration in registrations.registrations() {
            if !callbacks.borrow().contains_key(&registration.callback) {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!(
                        "controller callback '{}' is registered but not defined",
                        registration.callback
                    ),
                ));
            }
        }
        Ok(Self {
            lua,
            registrations,
            callbacks: callbacks.take(),
        })
    }

    pub fn registrations(&self) -> &ControllerRegistrationSet {
        &self.registrations
    }

    pub fn start_callback(&self, name: &str) -> Result<LuaCallbackCoroutine, DiagnosableError> {
        let function = self
            .callbacks
            .get(name)
            .ok_or_else(|| {
                DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!("Lua callback '{name}' is not defined"),
                )
            })
            .and_then(|key| self.lua.registry_value::<Function>(key).map_err(lua_error))?;
        let thread = self.lua.create_thread(function).map_err(lua_error)?;
        Ok(LuaCallbackCoroutine {
            thread: self.lua.create_registry_value(thread).map_err(lua_error)?,
        })
    }

    pub fn resume_callback(
        &self,
        coroutine: &LuaCallbackCoroutine,
        response: LuaHostResponse,
        capabilities: &CapabilitySet,
    ) -> Result<LuaCallbackStep, DiagnosableError> {
        let thread = self
            .lua
            .registry_value::<Thread>(&coroutine.thread)
            .map_err(lua_error)?;
        if thread.status() == ThreadStatus::Finished {
            return Ok(LuaCallbackStep::Complete);
        }
        let value: Value = thread
            .resume(lua_response(&self.lua, response)?)
            .map_err(lua_error)?;
        if thread.status() == ThreadStatus::Finished {
            return Ok(LuaCallbackStep::Complete);
        }
        let request = parse_host_request(value)?;
        if let Some(required) = request.required_capability() {
            if !capabilities.contains(required) {
                return Err(DiagnosableError::new(
                    ErrorPhase::CapabilityProbe,
                    format!("Lua host API requires capability '{required}'"),
                )
                .with_capability(required.legacy_capability()));
            }
        }
        Ok(LuaCallbackStep::Yielded(request))
    }
}

fn lua_compatible_controller_source(source: &str) -> String {
    rewrite_lua_keyword_table_key(source, "repeat")
}

fn rewrite_lua_keyword_table_key(source: &str, keyword: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let keyword_bytes = keyword.as_bytes();
    let mut index = 0usize;
    let mut string_quote = None;
    while index < source.len() {
        let byte = bytes[index];
        if let Some(quote) = string_quote {
            output.push(byte as char);
            if byte == b'\\' {
                if let Some(next) = bytes.get(index + 1) {
                    output.push(*next as char);
                    index += 2;
                    continue;
                }
            } else if byte == quote {
                string_quote = None;
            }
            index += 1;
            continue;
        }

        if byte == b'"' || byte == b'\'' {
            string_quote = Some(byte);
            output.push(byte as char);
            index += 1;
            continue;
        }

        if bytes[index..].starts_with(keyword_bytes)
            && is_lua_identifier_boundary(bytes, index, keyword.len())
            && next_non_whitespace(bytes, index + keyword.len()) == Some(b'=')
        {
            output.push_str("[\"");
            output.push_str(keyword);
            output.push_str("\"]");
            index += keyword.len();
            continue;
        }

        output.push(byte as char);
        index += 1;
    }
    output
}

fn is_lua_identifier_boundary(bytes: &[u8], start: usize, len: usize) -> bool {
    let before = start.checked_sub(1).and_then(|index| bytes.get(index));
    let after = bytes.get(start + len);
    !before.is_some_and(is_lua_identifier_byte) && !after.is_some_and(is_lua_identifier_byte)
}

fn is_lua_identifier_byte(byte: &u8) -> bool {
    byte.is_ascii_alphanumeric() || *byte == b'_'
}

fn next_non_whitespace(bytes: &[u8], mut index: usize) -> Option<u8> {
    while let Some(byte) = bytes.get(index) {
        if !byte.is_ascii_whitespace() {
            return Some(*byte);
        }
        index += 1;
    }
    None
}

fn install_sandbox(lua: &Lua) -> Result<(), DiagnosableError> {
    let globals = lua.globals();
    for denied in [
        "io", "os", "package", "require", "debug", "dofile", "loadfile", "load",
    ] {
        globals.set(denied, Value::Nil).map_err(lua_error)?;
    }
    Ok(())
}

fn install_sa_api(
    lua: &Lua,
    registrations: Rc<RefCell<Vec<ControllerRegistration>>>,
    callbacks: Rc<RefCell<BTreeMap<String, RegistryKey>>>,
) -> Result<(), DiagnosableError> {
    let sa = lua.create_table().map_err(lua_error)?;

    for (name, kind) in [
        ("hotkey", ControllerRegistrationKind::Hotkey),
        ("motion", ControllerRegistrationKind::Motion),
        ("press", ControllerRegistrationKind::Press),
        ("timer", ControllerRegistrationKind::Timer),
        ("shutdown", ControllerRegistrationKind::Shutdown),
    ] {
        let registrations = Rc::clone(&registrations);
        sa.set(
            name,
            lua.create_function(move |_, table: Table| {
                let registration = parse_registration_table(kind, table).map_err(mlua_error)?;
                registrations.borrow_mut().push(registration);
                Ok(())
            })
            .map_err(lua_error)?,
        )
        .map_err(lua_error)?;
    }

    let callback_store = Rc::clone(&callbacks);
    sa.set(
        "callback",
        lua.create_function(move |lua, (name, function): (String, Function)| {
            let key = lua.create_registry_value(function)?;
            callback_store
                .borrow_mut()
                .insert(name.trim().to_string(), key);
            Ok(())
        })
        .map_err(lua_error)?,
    )
    .map_err(lua_error)?;

    let input = lua.create_table().map_err(lua_error)?;
    sa.set("input", input).map_err(lua_error)?;
    let window = lua.create_table().map_err(lua_error)?;
    sa.set("window", window).map_err(lua_error)?;
    let state = lua.create_table().map_err(lua_error)?;
    state
        .set(
            "track",
            lua.create_function(|_, _: Table| Ok(()))
                .map_err(lua_error)?,
        )
        .map_err(lua_error)?;
    sa.set("state", state).map_err(lua_error)?;
    lua.globals().set("sa", sa).map_err(lua_error)?;

    lua.load(
        r#"
        function sa.sleep(ms)
          return coroutine.yield({ op = "sleep", ms = ms })
        end

        function sa.log(message)
          return coroutine.yield({ op = "log", level = "info", message = tostring(message) })
        end

        function sa.log_debug(message)
          return coroutine.yield({ op = "log", level = "debug", message = tostring(message) })
        end

        function sa.log_warn(message)
          return coroutine.yield({ op = "log", level = "warn", message = tostring(message) })
        end

        function sa.window.active(options)
          options = options or {}
          return coroutine.yield({ op = "window_active", title = options.title == true })
        end

        function sa.window.find(options)
          options = options or {}
          return coroutine.yield({ op = "window_find", processes = options.processes or {} })
        end

        function sa.window.activate(handle)
          return coroutine.yield({ op = "window_activate", handle = handle })
        end

        function sa.window.wait_active(handle, timeout_ms)
          return coroutine.yield({ op = "window_wait_active", handle = handle, timeout_ms = timeout_ms })
        end

        function sa.input.key(key)
          return coroutine.yield({ op = "input_key", key = key })
        end

        function sa.input.text(text)
          return coroutine.yield({ op = "input_text", text = text })
        end

        function sa.input.key_down(key)
          return coroutine.yield({ op = "input_key_down", key = key })
        end

        function sa.input.key_up(key)
          return coroutine.yield({ op = "input_key_up", key = key })
        end

        function sa.input.mouse_click(button)
          return coroutine.yield({ op = "input_mouse_click", button = button })
        end
        "#,
    )
    .set_name("signal-auras-host-api")
    .exec()
    .map_err(lua_error)?;
    Ok(())
}

fn parse_registration_table(
    kind: ControllerRegistrationKind,
    table: Table,
) -> Result<ControllerRegistration, DiagnosableError> {
    let trigger = table
        .get::<Option<String>>("trigger")
        .map_err(lua_error)?
        .unwrap_or_else(|| {
            if kind == ControllerRegistrationKind::Shutdown {
                "shutdown".to_string()
            } else {
                String::new()
            }
        });
    let callback = table
        .get::<Option<String>>("callback")
        .map_err(lua_error)?
        .or(table.get::<Option<String>>("handler").map_err(lua_error)?)
        .ok_or_else(|| {
            DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "controller callback is required",
            )
        })?;
    let mode = BindingMode::parse(
        table
            .get::<Option<String>>("mode")
            .map_err(lua_error)?
            .as_deref(),
    )?;
    let scope = parse_scope_field(table.get::<Value>("scope").map_err(lua_error)?)?;
    let required_capabilities = explicit_capabilities(&table)?
        .unwrap_or_else(|| fallback_required_capabilities(kind, mode, &scope));
    let requires_held = match table.get::<Value>("requires_held").map_err(lua_error)? {
        Value::Table(held) => HeldCondition::parse(string_array(held).unwrap_or_default()),
        _ => HeldCondition::new(Vec::new()),
    }?;
    Ok(
        ControllerRegistration::new(kind, trigger, scope, mode, callback, required_capabilities)?
            .with_requires_held(requires_held),
    )
}

fn parse_scope_field(value: Value) -> Result<ScopeSelection, DiagnosableError> {
    let Value::Table(table) = value else {
        return Ok(ScopeSelection::ExplicitGlobal);
    };
    if table.get::<Option<bool>>("global").map_err(lua_error)? == Some(true) {
        return Ok(ScopeSelection::ExplicitGlobal);
    }
    let processes = match table.get::<Value>("processes").map_err(lua_error)? {
        Value::Table(processes) => string_array(processes)
            .unwrap_or_default()
            .into_iter()
            .map(ProcessName::parse)
            .collect::<Result<Vec<_>, _>>()?,
        _ => Vec::new(),
    };
    if processes.is_empty() {
        Ok(ScopeSelection::ExplicitGlobal)
    } else {
        ScopeSelection::process_list(processes)
    }
}

fn explicit_capabilities(table: &Table) -> Result<Option<CapabilitySet>, DiagnosableError> {
    match table.get::<Value>("capabilities").map_err(lua_error)? {
        Value::Table(capabilities) => {
            let mut parsed = Vec::new();
            for name in string_array(capabilities)? {
                parsed.push(parse_capability_name(&name)?);
            }
            Ok(Some(CapabilitySet::new(parsed)))
        }
        _ => Ok(None),
    }
}

fn parse_capability_name(name: &str) -> Result<CapabilityKind, DiagnosableError> {
    match name.trim() {
        "global_shortcut" => Ok(CapabilityKind::GlobalShortcut),
        "composite_pointer_observation" => Ok(CapabilityKind::CompositePointerObservation),
        "composite_pointer_consumption" => Ok(CapabilityKind::CompositePointerConsumption),
        "active_process_metadata" => Ok(CapabilityKind::ActiveProcessMetadata),
        "active_window_metadata" => Ok(CapabilityKind::ActiveWindowMetadata),
        "window_activation" => Ok(CapabilityKind::WindowActivation),
        "synthesized_input" => Ok(CapabilityKind::SynthesizedInput),
        "timer" => Ok(CapabilityKind::Timer),
        "screen_read" => Ok(CapabilityKind::ScreenRead),
        other => Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("unknown Lua controller capability '{other}'"),
        )),
    }
}

fn fallback_required_capabilities(
    kind: ControllerRegistrationKind,
    mode: BindingMode,
    scope: &ScopeSelection,
) -> CapabilitySet {
    let mut required = Vec::new();
    match kind {
        ControllerRegistrationKind::Hotkey => required.push(CapabilityKind::GlobalShortcut),
        ControllerRegistrationKind::Motion | ControllerRegistrationKind::Press => {
            required.push(CapabilityKind::CompositePointerObservation);
            if mode == BindingMode::Consume {
                required.push(CapabilityKind::CompositePointerConsumption);
            }
        }
        ControllerRegistrationKind::Timer => required.push(CapabilityKind::Timer),
        ControllerRegistrationKind::Shutdown => {}
    }
    if matches!(scope, ScopeSelection::ProcessList { .. }) {
        required.push(CapabilityKind::ActiveProcessMetadata);
    }
    CapabilitySet::new(required)
}

fn string_array(table: Table) -> Result<Vec<String>, DiagnosableError> {
    table
        .sequence_values::<String>()
        .map(|value| value.map_err(lua_error))
        .collect()
}

fn parse_host_request(value: Value) -> Result<LuaHostRequest, DiagnosableError> {
    let Value::Table(table) = value else {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "Lua callback yielded an unsupported host request",
        ));
    };
    let op = table.get::<String>("op").map_err(lua_error)?;
    match op.as_str() {
        "sleep" => Ok(LuaHostRequest::Sleep {
            duration_ms: table.get::<u64>("ms").map_err(lua_error)?,
        }),
        "log" => Ok(LuaHostRequest::Log {
            level: LuaLogLevel::parse(&table.get::<String>("level").map_err(lua_error)?)?,
            message: table.get::<String>("message").map_err(lua_error)?,
        }),
        "window_active" => Ok(LuaHostRequest::ActiveWindow {
            include_title: table
                .get::<Option<bool>>("title")
                .map_err(lua_error)?
                .unwrap_or(false),
        }),
        "window_find" => Ok(LuaHostRequest::FindWindow {
            processes: match table.get::<Value>("processes").map_err(lua_error)? {
                Value::Table(processes) => string_array(processes)?,
                _ => Vec::new(),
            },
        }),
        "window_activate" => Ok(LuaHostRequest::ActivateWindow {
            handle: table.get::<String>("handle").map_err(lua_error)?,
        }),
        "window_wait_active" => Ok(LuaHostRequest::WaitActive {
            handle: table.get::<String>("handle").map_err(lua_error)?,
            timeout_ms: table.get::<u64>("timeout_ms").map_err(lua_error)?,
        }),
        "input_key" => Ok(LuaHostRequest::Input {
            action: MacroAction::key(table.get::<String>("key").map_err(lua_error)?)?,
        }),
        "input_text" => Ok(LuaHostRequest::Input {
            action: MacroAction::text(table.get::<String>("text").map_err(lua_error)?)?,
        }),
        "input_key_down" => Ok(LuaHostRequest::Input {
            action: MacroAction::key_down(table.get::<String>("key").map_err(lua_error)?)?,
        }),
        "input_key_up" => Ok(LuaHostRequest::Input {
            action: MacroAction::key_up(table.get::<String>("key").map_err(lua_error)?)?,
        }),
        "input_mouse_click" => Ok(LuaHostRequest::Input {
            action: MacroAction::mouse_click(MouseButton::parse(
                table.get::<String>("button").map_err(lua_error)?,
            )?),
        }),
        _ => Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("unsupported Lua host request '{op}'"),
        )),
    }
}

fn lua_response(lua: &Lua, response: LuaHostResponse) -> Result<Value, DiagnosableError> {
    match response {
        LuaHostResponse::Unit => Ok(Value::Nil),
        LuaHostResponse::Bool(value) => Ok(Value::Boolean(value)),
        LuaHostResponse::WindowHandle(Some(handle)) => Ok(Value::String(
            lua.create_string(&handle).map_err(lua_error)?,
        )),
        LuaHostResponse::WindowHandle(None) => Ok(Value::Nil),
        LuaHostResponse::ActiveWindow(metadata) => {
            let table = lua.create_table().map_err(lua_error)?;
            table.set("title", metadata.title).map_err(lua_error)?;
            Ok(Value::Table(table))
        }
    }
}

fn lua_error(error: mlua::Error) -> DiagnosableError {
    DiagnosableError::new(ErrorPhase::ScriptValidation, error.to_string())
}

fn mlua_error(error: DiagnosableError) -> mlua::Error {
    mlua::Error::RuntimeError(error.message)
}
