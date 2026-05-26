use signal_auras_core::{
    AutomationDefaults, BindingDefinition, BindingMode, BindingTrigger, CompositeTrigger,
    DiagnosableError, ErrorPhase, HotkeyId, InputProviderBackend, InputProviderConfig,
    InputProviderMode, InputProviderOutput, LuaAutomationConfiguration, MacroAction,
    MacroDefinition, ModifierSet, MotionDefinition, MotionTrigger, MouseButton, MouseTrigger,
    ProcessName, RepeatDefinition, RepeatInterval, ScriptScope, WheelDirection,
};
use std::{fs, path::Path};

const DENIED_TOKENS: &[&str] = &[
    "io.", "os.", "require", "package", "debug", "dofile", "loadfile", "load(", "socket",
    "luaposix",
];

pub fn load_lua_file(path: &Path) -> Result<LuaAutomationConfiguration, DiagnosableError> {
    let source = fs::read_to_string(path).map_err(|error| {
        DiagnosableError::new(
            ErrorPhase::ScriptLoad,
            format!("cannot read Lua file '{}': {error}", path.display()),
        )
    })?;
    load_lua_source(&source)
}

pub fn load_lua_source(source: &str) -> Result<LuaAutomationConfiguration, DiagnosableError> {
    for token in DENIED_TOKENS {
        if source.contains(token) {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("Lua sandbox denies ambient API token '{token}'"),
            ));
        }
    }
    if !source.contains("return")
        || !(source.contains("hotkeys")
            || source.contains("bindings")
            || source.contains("motions"))
    {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "Lua script must return a table with hotkeys, bindings, or motions",
        ));
    }

    let scope = parse_scope(source)?;
    let leader = parse_leader(source)?;
    let defaults = parse_defaults(source)?;
    let input_provider = parse_input_provider(source)?;
    let mut bindings = parse_hotkeys(source)?
        .into_iter()
        .map(|(hotkey, macro_definition)| {
            BindingDefinition::new(
                BindingTrigger::keyboard(hotkey),
                BindingMode::Consume,
                macro_definition,
            )
        })
        .collect::<Vec<_>>();
    bindings.extend(parse_bindings(source)?);
    let motions = parse_motions(source, &defaults, leader.is_some())?;
    LuaAutomationConfiguration::with_bindings_and_motions(
        scope,
        leader,
        defaults,
        input_provider,
        bindings,
        motions,
    )
}

fn parse_scope(source: &str) -> Result<Option<ScriptScope>, DiagnosableError> {
    let Some(scope_index) = source.find("scope") else {
        return Ok(None);
    };
    let end = [
        source.find("defaults"),
        source.find("input_provider"),
        source.find("hotkeys"),
        source.find("bindings"),
        source.find("motions"),
    ]
    .into_iter()
    .flatten()
    .filter(|index| *index > scope_index)
    .min()
    .unwrap_or(source.len());
    let scope_source = &source[scope_index..end];
    if scope_source.contains("global") {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "script-declared global scope is not accepted in v1",
        ));
    }
    let mut processes = Vec::new();
    for value in quoted_strings(scope_source) {
        processes.push(ProcessName::parse(value)?);
    }
    Ok(Some(ScriptScope::processes(processes)?))
}

fn parse_defaults(source: &str) -> Result<AutomationDefaults, DiagnosableError> {
    let Some(defaults_body) = table_body_after(source, "defaults")? else {
        return Ok(AutomationDefaults::default());
    };
    Ok(AutomationDefaults::new(
        field_u64(defaults_body, "inter_action_delay_ms")?.unwrap_or(0),
    ))
}

fn parse_leader(source: &str) -> Result<Option<signal_auras_core::MotionToken>, DiagnosableError> {
    let leader = field_string(source, "leader")
        .map(signal_auras_core::MotionToken::parse)
        .transpose()?;
    if leader
        .as_ref()
        .is_some_and(|token| !matches!(token, signal_auras_core::MotionToken::Key(_)))
    {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "leader must be a concrete key token",
        ));
    }
    Ok(leader)
}

fn parse_input_provider(source: &str) -> Result<Option<InputProviderConfig>, DiagnosableError> {
    let Some(provider_body) = table_body_after(source, "input_provider")? else {
        return Ok(None);
    };
    let backend =
        InputProviderBackend::parse(field_string(provider_body, "backend").ok_or_else(|| {
            DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "input_provider requires backend",
            )
        })?)?;
    let mode = InputProviderMode::parse(field_string(provider_body, "mode"))?;
    let output = InputProviderOutput::parse(field_string(provider_body, "output"))?;
    let acknowledge_risk = field_string(provider_body, "acknowledge_risk");
    match (backend, mode) {
        (InputProviderBackend::Evdev, InputProviderMode::Observe | InputProviderMode::Grab) => {
            if field_string(provider_body, "devices") == Some("all") {
                return InputProviderConfig::evdev_all(mode, output, acknowledge_risk).map(Some);
            }
            let devices_body = table_body_after(provider_body, "devices")?.ok_or_else(|| {
                DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    "input_provider requires devices",
                )
            })?;
            let devices = quoted_strings(devices_body)
                .into_iter()
                .map(std::path::PathBuf::from)
                .collect::<Vec<_>>();
            InputProviderConfig::evdev(devices, mode, output).map(Some)
        }
    }
}

fn parse_hotkeys(source: &str) -> Result<Vec<(HotkeyId, MacroDefinition)>, DiagnosableError> {
    let mut result = Vec::new();
    let mut cursor = source;
    while let Some(start) = cursor.find("[\"") {
        cursor = &cursor[start + 2..];
        let Some(end) = cursor.find("\"]") else {
            break;
        };
        let hotkey = HotkeyId::parse(&cursor[..end])?;
        cursor = &cursor[end + 2..];
        let Some(macro_start) = cursor.find("macro") else {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "hotkey must map to a macro",
            ));
        };
        cursor = &cursor[macro_start..];
        let Some(block_start) = cursor.find('{') else {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "macro must use table constructor",
            ));
        };
        let macro_body = &cursor[block_start + 1
            ..cursor[block_start + 1..]
                .find('}')
                .map(|end| block_start + 1 + end)
                .unwrap_or(cursor.len())];
        let actions = parse_actions(macro_body)?;
        result.push((hotkey, MacroDefinition::new(actions)?));
        cursor = &cursor[block_start + 1..];
    }
    Ok(result)
}

fn parse_bindings(source: &str) -> Result<Vec<BindingDefinition>, DiagnosableError> {
    let Some(bindings_body) = table_body_after(source, "bindings")? else {
        return Ok(Vec::new());
    };
    let mut result = Vec::new();
    for entry in top_level_tables(bindings_body) {
        result.push(parse_binding_entry(entry)?);
    }
    Ok(result)
}

fn parse_motions(
    source: &str,
    defaults: &AutomationDefaults,
    leader_defined: bool,
) -> Result<Vec<MotionDefinition>, DiagnosableError> {
    let Some(motions_body) = table_body_after(source, "motions")? else {
        return Ok(Vec::new());
    };
    let mut result = Vec::new();
    for entry in top_level_tables(motions_body) {
        result.push(parse_motion_entry(entry, defaults, leader_defined)?);
    }
    Ok(result)
}

fn parse_motion_entry(
    source: &str,
    defaults: &AutomationDefaults,
    leader_defined: bool,
) -> Result<MotionDefinition, DiagnosableError> {
    let mode = BindingMode::parse(field_string(source, "mode"))?;
    let trigger_body = table_body_after(source, "trigger")?.ok_or_else(|| {
        DiagnosableError::new(ErrorPhase::ScriptValidation, "motion requires trigger")
    })?;
    let trigger = MotionTrigger::parse(quoted_strings(trigger_body))?;
    if trigger.contains_leader() && !leader_defined {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "motion trigger uses <Leader> but leader is not configured",
        ));
    }
    let inter_action_delay_ms =
        field_u64(source, "inter_action_delay_ms")?.unwrap_or(defaults.inter_action_delay_ms);

    let before_repeat = source
        .find("repeat")
        .map(|index| &source[..index])
        .unwrap_or(source);
    let macro_definition = if before_repeat.contains("macro") {
        Some(parse_macro_field(before_repeat)?)
    } else {
        None
    };
    let repeat = table_body_after(source, "repeat")?
        .map(parse_repeat_definition)
        .transpose()?;
    MotionDefinition::new(
        trigger,
        mode,
        macro_definition,
        repeat,
        inter_action_delay_ms,
    )
}

fn parse_repeat_definition(source: &str) -> Result<RepeatDefinition, DiagnosableError> {
    let while_held_body = table_body_after(source, "while_held")?.ok_or_else(|| {
        DiagnosableError::new(ErrorPhase::ScriptValidation, "repeat requires while_held")
    })?;
    let interval_body = table_body_after(source, "interval_ms")?.ok_or_else(|| {
        DiagnosableError::new(ErrorPhase::ScriptValidation, "repeat requires interval_ms")
    })?;
    let min = field_u64(interval_body, "min")?.ok_or_else(|| {
        DiagnosableError::new(ErrorPhase::ScriptValidation, "repeat interval requires min")
    })?;
    let max = field_u64(interval_body, "max")?.ok_or_else(|| {
        DiagnosableError::new(ErrorPhase::ScriptValidation, "repeat interval requires max")
    })?;
    let macro_definition = parse_macro_field(source)?;
    Ok(RepeatDefinition::new(
        MotionTrigger::parse(quoted_strings(while_held_body))?,
        RepeatInterval::new(min, max)?,
        macro_definition,
    ))
}

fn parse_binding_entry(source: &str) -> Result<BindingDefinition, DiagnosableError> {
    let mode = BindingMode::parse(field_string(source, "mode"))?;
    let trigger_body = table_body_after(source, "trigger")?.ok_or_else(|| {
        DiagnosableError::new(ErrorPhase::ScriptValidation, "binding requires trigger")
    })?;
    let trigger = parse_binding_trigger(trigger_body)?;
    let macro_definition = parse_macro_field(source)?;
    Ok(BindingDefinition::new(trigger, mode, macro_definition))
}

fn parse_binding_trigger(source: &str) -> Result<BindingTrigger, DiagnosableError> {
    let modifiers = table_body_after(source, "modifiers")?
        .map(|body| ModifierSet::parse(quoted_strings(body)))
        .transpose()?
        .unwrap_or_default();
    let mouse_body = table_body_after(source, "mouse")?;
    let key = field_string(source, "key");
    let button = mouse_body.and_then(|body| field_string(body, "button"));
    let wheel = mouse_body.and_then(|body| field_string(body, "wheel"));
    let primary_count =
        usize::from(key.is_some()) + usize::from(button.is_some()) + usize::from(wheel.is_some());
    if primary_count != 1 {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "binding trigger must contain exactly one primary trigger",
        ));
    }
    if let Some(key) = key {
        return Ok(BindingTrigger::keyboard(HotkeyId::parse(key)?));
    }
    let primary = if let Some(button) = button {
        MouseTrigger::Button(MouseButton::parse(button)?)
    } else {
        MouseTrigger::Wheel(WheelDirection::parse(wheel.expect("wheel counted above"))?)
    };
    Ok(BindingTrigger::Composite(CompositeTrigger::new(
        modifiers, primary,
    )))
}

fn parse_macro_field(source: &str) -> Result<MacroDefinition, DiagnosableError> {
    let Some(macro_start) = source.find("macro") else {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "binding must map to a macro",
        ));
    };
    let macro_source = &source[macro_start..];
    let Some(block_start) = macro_source.find('{') else {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "macro must use table constructor",
        ));
    };
    let Some(block_end) = matching_brace(macro_source, block_start) else {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "macro table is not closed",
        ));
    };
    MacroDefinition::new(parse_actions(&macro_source[block_start + 1..block_end])?)
}

fn parse_actions(source: &str) -> Result<Vec<MacroAction>, DiagnosableError> {
    let mut actions = Vec::new();
    for line in source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Some(rest) = line.strip_prefix("key ") {
            actions.push(MacroAction::key(first_quoted(rest).ok_or_else(|| {
                DiagnosableError::new(ErrorPhase::ScriptValidation, "key action needs a string")
            })?)?);
        } else if let Some(rest) = line.strip_prefix("text ") {
            actions.push(MacroAction::text(first_quoted(rest).ok_or_else(|| {
                DiagnosableError::new(ErrorPhase::ScriptValidation, "text action needs a string")
            })?)?);
        } else if let Some(rest) = line.strip_prefix("mouse_click ") {
            actions.push(MacroAction::mouse_click(MouseButton::parse(
                first_quoted(rest).ok_or_else(|| {
                    DiagnosableError::new(
                        ErrorPhase::ScriptValidation,
                        "mouse_click action needs a string",
                    )
                })?,
            )?));
        } else if let Some(rest) = line.strip_prefix("delay") {
            let number = parse_delay_milliseconds(rest)?;
            actions.push(MacroAction::delay(number)?);
        } else if line.starts_with('}') || line.starts_with('{') {
        } else {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("unsupported macro action '{line}'"),
            ));
        }
    }
    Ok(actions)
}

fn field_u64(source: &str, field: &str) -> Result<Option<u64>, DiagnosableError> {
    let Some(index) = source.find(field) else {
        return Ok(None);
    };
    let after_field = &source[index + field.len()..];
    let Some(equals) = after_field.find('=') else {
        return Ok(None);
    };
    let value = after_field[equals + 1..]
        .trim_start()
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '-')
        .collect::<String>();
    if value.is_empty() {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("{field} needs milliseconds"),
        ));
    }
    let parsed = value.parse::<i64>().map_err(|_| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("{field} needs milliseconds"),
        )
    })?;
    if parsed < 0 {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("{field} cannot be negative"),
        ));
    }
    Ok(Some(parsed as u64))
}

fn parse_delay_milliseconds(source: &str) -> Result<u64, DiagnosableError> {
    let value =
        source.trim_matches(|c: char| c == '(' || c == ')' || c == ',' || c.is_whitespace());
    value.parse::<u64>().map_err(|_| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "delay action needs milliseconds",
        )
    })
}

fn quoted_strings(source: &str) -> Vec<&str> {
    let mut values = Vec::new();
    let mut cursor = source;
    while let Some(value) = first_quoted(cursor) {
        values.push(value);
        let offset = cursor.find(value).unwrap_or(0) + value.len() + 1;
        cursor = &cursor[offset.min(cursor.len())..];
    }
    values
}

fn first_quoted(source: &str) -> Option<&str> {
    let start = source.find('"')? + 1;
    let end = source[start..].find('"')? + start;
    Some(&source[start..end])
}

fn field_string<'a>(source: &'a str, field: &str) -> Option<&'a str> {
    let index = source.find(field)?;
    let after_field = &source[index + field.len()..];
    let equals = after_field.find('=')?;
    first_quoted(&after_field[equals + 1..])
}

fn table_body_after<'a>(source: &'a str, field: &str) -> Result<Option<&'a str>, DiagnosableError> {
    let Some(field_index) = source.find(field) else {
        return Ok(None);
    };
    let after_field = &source[field_index + field.len()..];
    let Some(block_start) = after_field.find('{') else {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("{field} must use table constructor"),
        ));
    };
    let Some(block_end) = matching_brace(after_field, block_start) else {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("{field} table is not closed"),
        ));
    };
    Ok(Some(&after_field[block_start + 1..block_end]))
}

fn top_level_tables(source: &str) -> Vec<&str> {
    let bytes = source.as_bytes();
    let mut tables = Vec::new();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        if bytes[cursor] == b'{' {
            if let Some(end) = matching_brace(source, cursor) {
                tables.push(&source[cursor + 1..end]);
                cursor = end + 1;
                continue;
            }
        }
        cursor += 1;
    }
    tables
}

fn matching_brace(source: &str, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    for (index, byte) in source.as_bytes().iter().enumerate().skip(start) {
        match (*byte, in_string) {
            (b'"', false) => in_string = true,
            (b'"', true) => in_string = false,
            (b'{', false) => depth += 1,
            (b'}', false) => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_v1_sample() {
        let source = r#"
            return {
              scope = { processes = { "poe2.exe" } },
              hotkeys = {
                ["F5"] = macro {
                  key "Enter",
                  text "/hideout",
                  delay(50),
                  key "Enter",
                },
              },
            }
        "#;

        let config = load_lua_source(source).unwrap();
        assert_eq!(config.hotkeys().len(), 1);
        assert!(config.scope.is_some());
    }

    #[test]
    fn parses_legacy_delay_without_parentheses() {
        let source = r#"
            return {
              hotkeys = {
                ["F5"] = macro {
                  delay 50,
                },
              },
            }
        "#;

        let config = load_lua_source(source).unwrap();
        assert_eq!(config.hotkeys().len(), 1);
    }

    #[test]
    fn denies_ambient_api() {
        assert!(load_lua_source("return { hotkeys = {}, x = os.getenv(\"HOME\") }").is_err());
    }

    #[test]
    fn parses_structured_mouse_wheel_binding() {
        let source = r#"
            return {
              bindings = {
                {
                  trigger = {
                    modifiers = { "Ctrl", "Shift" },
                    mouse = { wheel = "up" },
                  },
                  macro = macro { key "Left" },
                },
              },
            }
        "#;

        let config = load_lua_source(source).unwrap();
        let binding = config.bindings().values().next().unwrap();
        assert_eq!(binding.mode, BindingMode::Consume);
        assert_eq!(binding.trigger.describe(), "Ctrl+Shift+wheel_up");
    }

    #[test]
    fn parses_passthrough_mouse_button_binding() {
        let source = r#"
            return {
              bindings = {
                {
                  trigger = {
                    modifiers = { "Ctrl" },
                    mouse = { button = "left" },
                  },
                  mode = "passthrough",
                  macro = macro {
                    key "Alt+Right",
                    text "hello world",
                    key "Enter",
                  },
                },
              },
            }
        "#;

        let config = load_lua_source(source).unwrap();
        let binding = config.bindings().values().next().unwrap();
        assert_eq!(binding.mode, BindingMode::Passthrough);
        assert_eq!(binding.macro_definition.actions().len(), 3);
    }

    #[test]
    fn parses_uniform_keyboard_and_mouse_motions() {
        let source = r#"
            return {
              leader = "F13",
              defaults = {
                inter_action_delay_ms = 0,
              },
              motions = {
                {
                  trigger = { "<Leader>", "f", "f" },
                  mode = "consume",
                  macro = macro {
                    text "/search",
                  },
                },
                {
                  trigger = { "<Leader>", "<LClick>", "<LClick>" },
                  mode = "passthrough",
                  repeat = {
                    while_held = { "<Leader>", "<LClick>" },
                    interval_ms = { min = 50, max = 80 },
                    macro = macro {
                      mouse_click "left",
                    },
                  },
                },
              },
            }
        "#;

        let config = load_lua_source(source).unwrap();

        assert_eq!(config.motions().len(), 2);
        assert!(config
            .motions()
            .contains_key(&MotionTrigger::parse(["<Leader>", "f", "f"]).unwrap()));
        let repeat_motion = config
            .motions()
            .get(&MotionTrigger::parse(["<Leader>", "<LClick>", "<LClick>"]).unwrap())
            .unwrap();
        assert_eq!(repeat_motion.mode, BindingMode::Passthrough);
        assert!(repeat_motion.repeat.is_some());
    }

    #[test]
    fn motion_delay_override_takes_precedence_over_defaults() {
        let source = r#"
            return {
              leader = "F13",
              defaults = { inter_action_delay_ms = 10 },
              motions = {
                {
                  trigger = { "<Leader>", "f", "f" },
                  inter_action_delay_ms = 25,
                  macro = macro {
                    text "/search",
                  },
                },
              },
            }
        "#;

        let config = load_lua_source(source).unwrap();
        let motion = config.motions().values().next().unwrap();

        assert_eq!(config.defaults.inter_action_delay_ms, 10);
        assert_eq!(motion.inter_action_delay_ms, 25);
    }

    #[test]
    fn parses_explicit_evdev_observation_provider() {
        let source = r#"
            return {
              leader = "F13",
              input_provider = {
                backend = "evdev",
                mode = "observe",
                output = "portal",
                devices = {
                  "/dev/input/by-id/test-keyboard",
                  "/dev/input/by-id/test-mouse",
                },
              },
              motions = {
                {
                  trigger = { "<Leader>", "f", "f" },
                  macro = macro { text "/search" },
                },
              },
            }
        "#;

        let config = load_lua_source(source).unwrap();
        let provider = config.input_provider.unwrap();

        assert_eq!(provider.backend, InputProviderBackend::Evdev);
        assert_eq!(provider.mode, InputProviderMode::Observe);
        assert_eq!(provider.output, InputProviderOutput::Portal);
        assert_eq!(provider.devices.len(), 2);
    }

    #[test]
    fn parses_explicit_evdev_grab_and_uinput_provider() {
        let source = r#"
            return {
              leader = "F13",
              input_provider = {
                backend = "evdev",
                mode = "grab",
                output = "uinput",
                devices = { "/dev/input/by-id/test-mouse" },
              },
              motions = {
                {
                  trigger = { "<Leader>", "<LClick>", "<LClick>" },
                  macro = macro { mouse_click "left" },
                },
              },
            }
        "#;

        let config = load_lua_source(source).unwrap();
        let provider = config.input_provider.unwrap();

        assert_eq!(provider.mode, InputProviderMode::Grab);
        assert_eq!(provider.output, InputProviderOutput::Uinput);
    }

    #[test]
    fn parses_explicit_all_device_observation_provider() {
        let source = r#"
            return {
              leader = "F13",
              input_provider = {
                backend = "evdev",
                mode = "observe",
                devices = "all",
              },
              motions = {
                {
                  trigger = { "<Leader>", "f", "f" },
                  macro = macro { text "/search" },
                },
              },
            }
        "#;

        let config = load_lua_source(source).unwrap();
        let provider = config.input_provider.unwrap();

        assert!(provider.all_devices);
        assert!(provider.devices.is_empty());
        assert_eq!(provider.mode, InputProviderMode::Observe);
    }

    #[test]
    fn all_device_grab_requires_explicit_acknowledgement() {
        let denied = r#"
            return {
              leader = "F13",
              input_provider = {
                backend = "evdev",
                mode = "grab",
                devices = "all",
              },
              motions = {
                {
                  trigger = { "<Leader>", "f", "f" },
                  macro = macro { text "/search" },
                },
              },
            }
        "#;

        assert!(load_lua_source(denied).is_err());

        let accepted = r#"
            return {
              leader = "F13",
              input_provider = {
                backend = "evdev",
                mode = "grab",
                devices = "all",
                acknowledge_risk = "GRAB_ALL_INPUTS",
              },
              motions = {
                {
                  trigger = { "<Leader>", "f", "f" },
                  macro = macro { text "/search" },
                },
              },
            }
        "#;

        let provider = load_lua_source(accepted).unwrap().input_provider.unwrap();
        assert!(provider.all_devices);
        assert_eq!(provider.mode, InputProviderMode::Grab);
    }

    #[test]
    fn rejects_invalid_motion_shapes() {
        for source in [
            r#"return { motions = { { trigger = {}, macro = macro { text "x" } } } }"#,
            r#"return { motions = { { trigger = { "<Bad>" }, macro = macro { text "x" } } } }"#,
            r#"return { defaults = { inter_action_delay_ms = -1 }, motions = { { trigger = { "f" }, macro = macro { text "x" } } } }"#,
            r#"return { motions = { { trigger = { "f" }, repeat = { while_held = { "f" }, interval_ms = { min = 80, max = 50 }, macro = macro { text "x" } } } } }"#,
            r#"return { motions = { { trigger = { "f" }, repeat = { while_held = { "f" }, interval_ms = { min = 50, max = 80 } } } } }"#,
        ] {
            assert!(
                load_lua_source(source).is_err(),
                "source should be denied: {source}"
            );
        }
    }

    #[test]
    fn rejects_malformed_structured_binding_triggers() {
        for source in [
            r#"return { bindings = { { trigger = { modifiers = { "Meta" }, mouse = { wheel = "up" } }, macro = macro { key "Left" } } } }"#,
            r#"return { bindings = { { trigger = { mouse = { button = "back" } }, macro = macro { key "Left" } } } }"#,
            r#"return { bindings = { { trigger = { mouse = { wheel = "sideways" } }, macro = macro { key "Left" } } } }"#,
            r#"return { bindings = { { trigger = { modifiers = { "Ctrl" } }, macro = macro { key "Left" } } } }"#,
            r#"return { bindings = { { trigger = { key = "F5", mouse = { wheel = "up" } }, macro = macro { key "Left" } } } }"#,
        ] {
            assert!(
                load_lua_source(source).is_err(),
                "source should be denied: {source}"
            );
        }
    }
}
