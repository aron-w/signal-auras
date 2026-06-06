use crate::{
    runtime::lua_compatible_controller_source,
    sandbox_policy::{
        contains_denied_token, declarative_denied_tokens, denied_global_tokens, first_denied_token,
        install_denied_globals,
    },
};
use mlua::{Function, Lua, Table};
use signal_auras_core::{
    AutomationDefaults, BindingDefinition, BindingMode, BindingTrigger, CapabilityKind,
    CapabilitySet, CircularMask, CompositeTrigger, ControllerCallback, ControllerLoopPolicy,
    ControllerProgram, ControllerRegistration, ControllerRegistrationKind,
    ControllerRegistrationSet, DetectorDefinition, DiagnosableError, ErrorPhase, HeldCondition,
    HotkeyId, InputProviderBackend, InputProviderConfig, InputProviderMode, InputProviderOutput,
    LoopBody, LoopDefinition, LoopInterval, LoopRepeat, LuaAutomationConfiguration, MacroAction,
    MacroDefinition, ModifierSet, MotionDefinition, MotionToken, MotionTrigger, MouseButton,
    MouseTrigger, OverlayDefinition, OverlayDefinitionSet, OverlayRect, OverlayStyle,
    OverlaySurfaceKind, OverlayToggleHotkey, PressDefinition, ProcessName,
    ProgressBarVisualDefinition, ProgressFillDirection, RadialCooldownPhase, RadialCooldownPhases,
    RadialCooldownPrediction, RadialPhaseRule, RadialProgressFill, RadialRuleMetric,
    RadialSampleRegion, RendererProviderId, Roi, ScopeSelection, ScriptScope, StateBinding,
    StateField, StateTrackerCondition, StateTrackerDefinition, StateTrackerDefinitionSet,
    VisualDefinition, WheelDirection,
};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

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
    for token in declarative_denied_tokens() {
        if contains_denied_token(source, token) {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("Lua sandbox denies ambient API token '{token}'"),
            ));
        }
    }
    if !source.contains("return")
        || !(source.contains("hotkeys")
            || source.contains("bindings")
            || source.contains("motions")
            || source.contains("presses"))
    {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "Lua script must return a table with hotkeys, bindings, motions, or presses",
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
    let presses = parse_presses(source, &defaults, leader.is_some())?;
    LuaAutomationConfiguration::with_bindings_and_motions(
        scope,
        leader,
        defaults,
        input_provider,
        bindings,
        motions,
        presses,
    )
}

pub fn load_lua_controller_file(
    path: &Path,
) -> Result<ControllerRegistrationSet, DiagnosableError> {
    let source = load_controller_source_tree_file(path)?;
    load_lua_controller_source(&source)
}

pub fn load_lua_controller_program_file(
    path: &Path,
) -> Result<ControllerProgram, DiagnosableError> {
    let source = load_controller_source_tree_file(path)?;
    load_lua_controller_program_source(&source)
}

pub fn load_lua_controller_runtime_source_file(path: &Path) -> Result<String, DiagnosableError> {
    let source = load_controller_source_tree_file(path)?;
    load_lua_controller_program_source(&source)?;
    Ok(source)
}

pub fn load_lua_controller_source(
    source: &str,
) -> Result<ControllerRegistrationSet, DiagnosableError> {
    validate_controller_sandbox(source)?;
    parse_controller_registration_set(source)
}

pub fn load_lua_controller_program_source(
    source: &str,
) -> Result<ControllerProgram, DiagnosableError> {
    validate_controller_sandbox(source)?;
    let registrations = parse_controller_registration_set(source)?;
    let state_trackers = parse_state_tracker_definition_set(source)?;
    let overlays = parse_overlay_definition_set(source, &state_trackers)?;
    Ok(
        ControllerProgram::new(registrations, parse_controller_callbacks(source)?)?
            .with_state_trackers(state_trackers)
            .with_overlay_definitions(overlays)
            .with_runtime_options(parse_input_provider(source)?, parse_leader(source)?),
    )
}

fn validate_controller_sandbox(source: &str) -> Result<(), DiagnosableError> {
    let lua = Lua::new();
    install_denied_globals(&lua)?;
    install_controller_validation_api(&lua)?;
    let source = lua_compatible_controller_source(source);
    lua.load(&source)
        .set_name("signal-auras-controller-validation")
        .exec()
        .map_err(controller_lua_error)?;
    Ok(())
}

fn install_controller_validation_api(lua: &Lua) -> Result<(), DiagnosableError> {
    let sa = lua.create_table().map_err(controller_lua_error)?;
    for name in ["hotkey", "motion", "press", "timer", "shutdown"] {
        sa.set(
            name,
            lua.create_function(|_, _: Table| Ok(()))
                .map_err(controller_lua_error)?,
        )
        .map_err(controller_lua_error)?;
    }
    sa.set(
        "callback",
        lua.create_function(|_, (_name, _function): (String, Function)| Ok(()))
            .map_err(controller_lua_error)?,
    )
    .map_err(controller_lua_error)?;
    sa.set(
        "import",
        lua.create_function(|_, _module: String| Ok(()))
            .map_err(controller_lua_error)?,
    )
    .map_err(controller_lua_error)?;

    sa.set("input", lua.create_table().map_err(controller_lua_error)?)
        .map_err(controller_lua_error)?;
    sa.set("window", lua.create_table().map_err(controller_lua_error)?)
        .map_err(controller_lua_error)?;

    let state = lua.create_table().map_err(controller_lua_error)?;
    state
        .set(
            "track",
            lua.create_function(|_, _: Table| Ok(()))
                .map_err(controller_lua_error)?,
        )
        .map_err(controller_lua_error)?;
    sa.set("state", state).map_err(controller_lua_error)?;

    let overlay = lua.create_table().map_err(controller_lua_error)?;
    overlay
        .set(
            "mount",
            lua.create_function(|_, _: Table| Ok(()))
                .map_err(controller_lua_error)?,
        )
        .map_err(controller_lua_error)?;
    sa.set("overlay", overlay).map_err(controller_lua_error)?;

    lua.globals().set("sa", sa).map_err(controller_lua_error)?;
    Ok(())
}

fn controller_lua_error(error: mlua::Error) -> DiagnosableError {
    DiagnosableError::new(
        ErrorPhase::ScriptValidation,
        format!("Lua controller sandbox rejected source: {error}"),
    )
}

fn parse_controller_registration_set(
    source: &str,
) -> Result<ControllerRegistrationSet, DiagnosableError> {
    let mut registrations = Vec::new();
    registrations.extend(parse_controller_entries(
        source,
        "sa.hotkey",
        ControllerRegistrationKind::Hotkey,
    )?);
    registrations.extend(parse_controller_entries(
        source,
        "sa.motion",
        ControllerRegistrationKind::Motion,
    )?);
    registrations.extend(parse_controller_entries(
        source,
        "sa.press",
        ControllerRegistrationKind::Press,
    )?);
    registrations.extend(parse_controller_entries(
        source,
        "sa.timer",
        ControllerRegistrationKind::Timer,
    )?);
    registrations.extend(parse_controller_entries(
        source,
        "sa.shutdown",
        ControllerRegistrationKind::Shutdown,
    )?);
    ControllerRegistrationSet::new(registrations)
}

fn parse_state_tracker_definition_set(
    source: &str,
) -> Result<StateTrackerDefinitionSet, DiagnosableError> {
    let mut trackers = Vec::new();
    let mut cursor = source;
    while let Some(start) = cursor.find("sa.state.track") {
        cursor = &cursor[start + "sa.state.track".len()..];
        let Some(paren) = cursor.find('(') else {
            break;
        };
        cursor = &cursor[paren + 1..];
        let Some(block_start) = cursor.find('{') else {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "sa.state.track requires a table argument",
            ));
        };
        let block = &cursor[block_start..];
        let Some(block_end) = matching_table_end(block) else {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "sa.state.track table is not closed",
            ));
        };
        trackers.push(parse_state_tracker_definition(
            &block[1..block_end],
            source,
        )?);
        cursor = &block[block_end + 1..];
    }
    StateTrackerDefinitionSet::new(trackers)
}

fn parse_state_tracker_definition(
    source: &str,
    full_source: &str,
) -> Result<StateTrackerDefinition, DiagnosableError> {
    for forbidden in ["emits", "fixture", "source", "callback", "macro"] {
        if top_level_field_index(source, forbidden).is_some() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("state tracker field '{forbidden}' is not accepted"),
            ));
        }
    }
    let id = field_string(source, "id").ok_or_else(|| {
        DiagnosableError::new(ErrorPhase::ScriptValidation, "state tracker requires id")
    })?;
    let capabilities = parse_controller_capabilities(source)?
        .unwrap_or_else(|| CapabilitySet::new([signal_auras_core::CapabilityKind::ScreenRead]));
    let poll_ms = field_u64(source, "poll_ms")?.ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "state tracker requires poll_ms",
        )
    })?;
    let scope = parse_state_tracker_scope(source, full_source)?;
    let detector_body = table_body_field_after(source, "detector")?.ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "state tracker requires detector",
        )
    })?;
    let detector = parse_state_detector(detector_body)?;
    let tracker = StateTrackerDefinition::new(id, scope, capabilities, poll_ms, detector)?;
    if let Some(condition_body) = table_body_field_after(source, "when")? {
        Ok(tracker.only_when(parse_state_tracker_condition(condition_body)?))
    } else {
        Ok(tracker)
    }
}

fn parse_state_tracker_condition(source: &str) -> Result<StateTrackerCondition, DiagnosableError> {
    let tracker = field_string(source, "tracker").ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "state tracker condition requires tracker",
        )
    })?;
    let phase = parse_radial_phase_name(field_string(source, "phase").ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "state tracker condition requires phase",
        )
    })?)?;
    StateTrackerCondition::radial_phase(tracker, phase)
}

fn parse_overlay_definition_set(
    source: &str,
    trackers: &StateTrackerDefinitionSet,
) -> Result<OverlayDefinitionSet, DiagnosableError> {
    let mut overlays = Vec::new();
    let mut cursor = source;
    while let Some(start) = cursor.find("sa.overlay.mount") {
        cursor = &cursor[start + "sa.overlay.mount".len()..];
        let Some(paren) = cursor.find('(') else {
            break;
        };
        cursor = &cursor[paren + 1..];
        let Some(block_start) = cursor.find('{') else {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "sa.overlay.mount requires a table argument",
            ));
        };
        let block = &cursor[block_start..];
        let Some(block_end) = matching_table_end(block) else {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "sa.overlay.mount table is not closed",
            ));
        };
        overlays.push(parse_overlay_definition(&block[1..block_end], source)?);
        cursor = &block[block_end + 1..];
    }
    OverlayDefinitionSet::new(overlays, trackers)
}

fn parse_overlay_definition(
    source: &str,
    full_source: &str,
) -> Result<OverlayDefinition, DiagnosableError> {
    for forbidden in [
        "callback",
        "macro",
        "screen",
        "screen_buffer",
        "input",
        "device",
        "compositor",
        "portal",
        "permission",
        "filesystem",
        "network",
    ] {
        if top_level_field_index(source, forbidden).is_some() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("overlay field '{forbidden}' is not accepted"),
            ));
        }
    }
    let id = field_string(source, "id").ok_or_else(|| {
        DiagnosableError::new(ErrorPhase::ScriptValidation, "overlay requires id")
    })?;
    let provider =
        RendererProviderId::parse(field_string(source, "provider").ok_or_else(|| {
            DiagnosableError::new(ErrorPhase::ScriptValidation, "overlay requires provider")
        })?)?;
    let surface_kind = OverlaySurfaceKind::parse(field_string(source, "surface"))?;
    let toggle_hotkey = parse_overlay_toggle_hotkey(source)?;
    let scope = parse_overlay_scope(source, full_source)?;
    let visuals_body = table_body_field_after(source, "visuals")?.ok_or_else(|| {
        DiagnosableError::new(ErrorPhase::ScriptValidation, "overlay requires visuals")
    })?;
    let visuals = top_level_tables(visuals_body)
        .into_iter()
        .map(parse_overlay_visual)
        .collect::<Result<Vec<_>, _>>()?;
    OverlayDefinition::new(id, scope, surface_kind, provider, toggle_hotkey, visuals)
}

fn parse_overlay_toggle_hotkey(
    source: &str,
) -> Result<Option<OverlayToggleHotkey>, DiagnosableError> {
    if top_level_field_index(source, "hotkey").is_none() {
        return Ok(None);
    }
    let hotkey_body = table_body_field_after(source, "hotkey")?.ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "overlay hotkey must use table constructor",
        )
    })?;
    let trigger = field_string(hotkey_body, "trigger").ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "overlay hotkey requires trigger",
        )
    })?;
    Ok(Some(OverlayToggleHotkey::new(
        HotkeyId::parse(trigger)?,
        BindingMode::parse(field_string(hotkey_body, "mode"))?,
    )))
}

fn parse_overlay_scope(
    source: &str,
    full_source: &str,
) -> Result<ScopeSelection, DiagnosableError> {
    if top_level_field_uses_table(source, "scope") {
        return parse_controller_scope(source);
    }
    if let Some(identifier) = field_identifier(source, "scope") {
        let scope_body = top_level_table_body(full_source, identifier)?.ok_or_else(|| {
            DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("overlay scope variable '{identifier}' is not defined"),
            )
        })?;
        return parse_process_scope_body(scope_body);
    }
    Ok(ScopeSelection::ExplicitGlobal)
}

fn parse_overlay_visual(source: &str) -> Result<VisualDefinition, DiagnosableError> {
    for forbidden in [
        "callback",
        "macro",
        "screen",
        "input",
        "compositor",
        "network",
    ] {
        if top_level_field_index(source, forbidden).is_some() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("overlay visual field '{forbidden}' is not accepted"),
            ));
        }
    }
    let kind = field_string(source, "kind").ok_or_else(|| {
        DiagnosableError::new(ErrorPhase::ScriptValidation, "overlay visual requires kind")
    })?;
    match kind {
        "progress_bar" => {
            let bind_body = table_body_field_after(source, "bind")?.ok_or_else(|| {
                DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    "overlay progress_bar requires bind",
                )
            })?;
            let tracker = field_string(bind_body, "tracker").ok_or_else(|| {
                DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    "overlay binding requires tracker",
                )
            })?;
            let field = StateField::parse(field_string(bind_body, "field").ok_or_else(|| {
                DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    "overlay binding requires field",
                )
            })?)?;
            let rect_body = table_body_field_after(source, "rect")?.ok_or_else(|| {
                DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    "overlay progress_bar requires rect",
                )
            })?;
            Ok(VisualDefinition::ProgressBar(
                ProgressBarVisualDefinition::new(
                    field_string(source, "id").ok_or_else(|| {
                        DiagnosableError::new(
                            ErrorPhase::ScriptValidation,
                            "overlay visual requires id",
                        )
                    })?,
                    StateBinding::new(tracker, field)?,
                    parse_overlay_rect(rect_body)?,
                    field_f32(source, "opacity")?.unwrap_or(1.0),
                    field_string(source, "fill").ok_or_else(|| {
                        DiagnosableError::new(
                            ErrorPhase::ScriptValidation,
                            "overlay progress_bar requires fill",
                        )
                    })?,
                    field_string(source, "background").ok_or_else(|| {
                        DiagnosableError::new(
                            ErrorPhase::ScriptValidation,
                            "overlay progress_bar requires background",
                        )
                    })?,
                    table_body_field_after(source, "label")?
                        .and_then(|body| field_bool(body, "visible"))
                        .unwrap_or(false),
                    table_body_field_after(source, "ready")?
                        .map(parse_overlay_style)
                        .transpose()?,
                    table_body_field_after(source, "activated")?
                        .map(parse_overlay_style)
                        .transpose()?,
                    table_body_field_after(source, "active")?
                        .map(parse_overlay_style)
                        .transpose()?,
                    table_body_field_after(source, "inactive")?
                        .map(parse_overlay_style)
                        .transpose()?,
                )?,
            ))
        }
        other => Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("unsupported overlay visual kind '{other}'"),
        )),
    }
}

fn parse_overlay_rect(source: &str) -> Result<OverlayRect, DiagnosableError> {
    OverlayRect::new(
        field_i64(source, "x")?.unwrap_or(0),
        field_i64(source, "y")?.unwrap_or(0),
        field_i64(source, "w")?.ok_or_else(|| {
            DiagnosableError::new(ErrorPhase::ScriptValidation, "overlay rect requires w")
        })?,
        field_i64(source, "h")?.ok_or_else(|| {
            DiagnosableError::new(ErrorPhase::ScriptValidation, "overlay rect requires h")
        })?,
    )
}

fn parse_overlay_style(source: &str) -> Result<OverlayStyle, DiagnosableError> {
    OverlayStyle::new(
        field_string(source, "fill"),
        field_string(source, "background"),
        field_f32(source, "opacity")?,
        field_bool(source, "label_visible").or_else(|| field_bool(source, "visible")),
    )
}

fn parse_state_tracker_scope(
    source: &str,
    full_source: &str,
) -> Result<ScopeSelection, DiagnosableError> {
    if top_level_field_uses_table(source, "scope") {
        return parse_controller_scope(source);
    }
    if let Some(identifier) = field_identifier(source, "scope") {
        let scope_body = top_level_table_body(full_source, identifier)?.ok_or_else(|| {
            DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("state tracker scope variable '{identifier}' is not defined"),
            )
        })?;
        return parse_process_scope_body(scope_body);
    }
    Ok(ScopeSelection::ExplicitGlobal)
}

fn parse_state_detector(source: &str) -> Result<DetectorDefinition, DiagnosableError> {
    let kind = field_string(source, "kind").ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "state tracker detector requires kind",
        )
    })?;
    let roi = parse_roi(table_body_field_after(source, "roi")?.ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "state tracker detector requires roi",
        )
    })?)?;
    match kind {
        "radial_cooldown" => {
            let mask = table_body_field_after(source, "mask")?
                .map(parse_circular_mask)
                .transpose()?;
            let phases_body = table_body_field_after(source, "phases")?.ok_or_else(|| {
                DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    "radial_cooldown detector requires phases",
                )
            })?;
            let mut phases = parse_radial_cooldown_phases(phases_body)?;
            if let Some(prediction_body) = table_body_field_after(source, "prediction")? {
                phases =
                    phases.with_prediction(parse_radial_cooldown_prediction(prediction_body)?)?;
            }
            phases.validate_for_roi(&roi)?;
            Ok(DetectorDefinition::RadialCooldown { roi, mask, phases })
        }
        "horizontal_progress_bar" => {
            let fill_body = table_body_field_after(source, "fill")?.ok_or_else(|| {
                DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    "horizontal_progress_bar detector requires fill",
                )
            })?;
            let direction = match field_string(fill_body, "direction").unwrap_or("left_to_right") {
                "left_to_right" => ProgressFillDirection::LeftToRight,
                other => {
                    return Err(DiagnosableError::new(
                        ErrorPhase::ScriptValidation,
                        format!("unsupported progress fill direction '{other}'"),
                    ));
                }
            };
            Ok(DetectorDefinition::HorizontalProgressBar {
                roi,
                fill_direction: direction,
            })
        }
        other => Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("unknown state tracker detector kind '{other}'"),
        )),
    }
}

fn parse_radial_cooldown_prediction(
    source: &str,
) -> Result<RadialCooldownPrediction, DiagnosableError> {
    let duration_ms = field_u64(source, "duration_ms")?.ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "radial_cooldown prediction requires duration_ms",
        )
    })?;
    let stable_after_ms = field_u64(source, "stable_after_ms")?.unwrap_or(0);
    RadialCooldownPrediction::new(duration_ms, stable_after_ms)
}

fn parse_radial_cooldown_phases(source: &str) -> Result<RadialCooldownPhases, DiagnosableError> {
    let order_body = table_body_field_after(source, "order")?.ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "radial_cooldown phases requires order",
        )
    })?;
    let fallback = match field_string(source, "fallback").unwrap_or("unknown") {
        "unknown" => RadialCooldownPhase::Unknown,
        other => {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("unsupported radial_cooldown fallback '{other}'"),
            ));
        }
    };

    let mut rules = Vec::new();
    for phase_name in quoted_strings(order_body) {
        let phase = parse_radial_phase_name(phase_name)?;
        let body = table_body_field_after(source, phase_name)?.ok_or_else(|| {
            DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("radial_cooldown phase '{phase_name}' is not defined"),
            )
        })?;
        rules.push(parse_radial_phase_rule(phase, body)?);
    }
    RadialCooldownPhases::new(rules, fallback)
}

fn parse_radial_phase_name(value: &str) -> Result<RadialCooldownPhase, DiagnosableError> {
    match value {
        "ready" => Ok(RadialCooldownPhase::Ready),
        "activated" => Ok(RadialCooldownPhase::Activated),
        "active" => Ok(RadialCooldownPhase::Active),
        "recovering" => Ok(RadialCooldownPhase::Recovering),
        "unknown" => Ok(RadialCooldownPhase::Unknown),
        other => Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("unknown radial_cooldown phase '{other}'"),
        )),
    }
}

fn parse_radial_phase_rule(
    phase: RadialCooldownPhase,
    source: &str,
) -> Result<RadialPhaseRule, DiagnosableError> {
    for forbidden in ["fill", "background", "opacity"] {
        if top_level_field_index(source, forbidden).is_some() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("radial_cooldown phase style field '{forbidden}' is not accepted"),
            ));
        }
    }
    let sample_body = table_body_field_after(source, "sample")?.ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "radial_cooldown phase requires sample",
        )
    })?;
    Ok(RadialPhaseRule {
        phase,
        sample: parse_radial_sample_region(sample_body)?,
        min_luminance_percent: parse_optional_u8(source, "min_luminance_percent", 100)?,
        max_luminance_percent: parse_optional_u8(source, "max_luminance_percent", 100)?,
        min_saturation: parse_optional_u8(source, "min_saturation", 255)?,
        max_saturation: parse_optional_u8(source, "max_saturation", 255)?,
        metric: match field_string(source, "metric").unwrap_or("average") {
            "average" => RadialRuleMetric::Average,
            "bright_ratio" => RadialRuleMetric::BrightRatio,
            other => {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!("unsupported radial_cooldown metric '{other}'"),
                ));
            }
        },
        metric_scale: field_f32(source, "metric_scale")?,
        progress_fill: match field_string(source, "progress_fill").ok_or_else(|| {
            DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "radial_cooldown phase requires progress_fill",
            )
        })? {
            "empty" => RadialProgressFill::Empty,
            "fraction" => RadialProgressFill::Fraction,
            "full" => RadialProgressFill::Full,
            other => {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!("unsupported radial_cooldown progress_fill '{other}'"),
                ));
            }
        },
        max_fill_until_ready: field_f32(source, "max_fill_until_ready")?,
    })
}

fn parse_radial_sample_region(source: &str) -> Result<RadialSampleRegion, DiagnosableError> {
    match field_string(source, "kind").ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "radial_cooldown sample requires kind",
        )
    })? {
        "clock_probe" => Ok(RadialSampleRegion::ClockProbe {
            angle_deg: field_f32(source, "angle_deg")?.ok_or_else(|| {
                DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    "clock_probe requires angle_deg",
                )
            })?,
            radius_px: parse_required_u32(source, "radius_px")?,
            w: parse_required_u32(source, "w")?,
            h: parse_required_u32(source, "h")?,
        }),
        "annulus_arc" => Ok(RadialSampleRegion::AnnulusArc {
            inner_radius_px: parse_required_u32(source, "inner_radius_px")?,
            outer_radius_px: parse_required_u32(source, "outer_radius_px")?,
            start_deg: field_f32(source, "start_deg")?.ok_or_else(|| {
                DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    "annulus_arc requires start_deg",
                )
            })?,
            end_deg: field_f32(source, "end_deg")?.ok_or_else(|| {
                DiagnosableError::new(ErrorPhase::ScriptValidation, "annulus_arc requires end_deg")
            })?,
        }),
        "aggregate_mask" => Ok(RadialSampleRegion::AggregateMask),
        other => Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("unknown radial_cooldown sample kind '{other}'"),
        )),
    }
}

fn parse_required_u32(source: &str, field: &str) -> Result<u32, DiagnosableError> {
    let value = field_u64(source, field)?.ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("radial_cooldown sample requires {field}"),
        )
    })?;
    u32::try_from(value).map_err(|_| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("{field} must fit in u32"),
        )
    })
}

fn parse_optional_u8(
    source: &str,
    field: &str,
    maximum: u64,
) -> Result<Option<u8>, DiagnosableError> {
    let Some(value) = field_u64(source, field)? else {
        return Ok(None);
    };
    if value > maximum {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("{field} must be between 0 and {maximum}"),
        ));
    }
    Ok(Some(value as u8))
}

fn parse_roi(source: &str) -> Result<Roi, DiagnosableError> {
    Roi::new(
        field_u64(source, "x")?.unwrap_or(0) as u32,
        field_u64(source, "y")?.unwrap_or(0) as u32,
        field_u64(source, "w")?
            .ok_or_else(|| DiagnosableError::new(ErrorPhase::ScriptValidation, "ROI requires w"))?
            as u32,
        field_u64(source, "h")?
            .ok_or_else(|| DiagnosableError::new(ErrorPhase::ScriptValidation, "ROI requires h"))?
            as u32,
    )
}

fn parse_circular_mask(source: &str) -> Result<CircularMask, DiagnosableError> {
    if field_string(source, "shape").unwrap_or("circle") != "circle" {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "radial_cooldown mask shape must be circle",
        ));
    }
    Ok(CircularMask::new(
        field_u64(source, "inset")?.unwrap_or(0) as u32
    ))
}

fn parse_controller_callbacks(source: &str) -> Result<Vec<ControllerCallback>, DiagnosableError> {
    let mut callbacks = Vec::new();
    let mut cursor = source;
    while let Some(start) = cursor.find("sa.callback") {
        cursor = &cursor[start + "sa.callback".len()..];
        let Some(paren) = cursor.find('(') else {
            break;
        };
        cursor = &cursor[paren + 1..];
        let name = first_quoted(cursor).ok_or_else(|| {
            DiagnosableError::new(ErrorPhase::ScriptValidation, "sa.callback requires a name")
        })?;
        let Some(function_start) = cursor.find("function") else {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "sa.callback requires a function body",
            ));
        };
        let after_function = &cursor[function_start + "function".len()..];
        let Some(args_end) = after_function.find(')') else {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "sa.callback function arguments are not closed",
            ));
        };
        let body = &after_function[args_end + 1..];
        let Some(body_end) = body.find("\nend").or_else(|| body.find(" end")) else {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "sa.callback function body is not closed",
            ));
        };
        callbacks.push(ControllerCallback::new(
            name,
            parse_controller_output_actions(&body[..body_end])?,
        )?);
        cursor = &body[body_end + "end".len()..];
    }
    Ok(callbacks)
}

fn parse_controller_output_actions(source: &str) -> Result<Vec<MacroAction>, DiagnosableError> {
    if let Some(token) = first_denied_token(source, denied_global_tokens()) {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("Lua controller callback sandbox denies ambient API token '{token}'"),
        ));
    }
    if source.contains("sa.sleep") || source.contains("sa.window.") {
        return Ok(Vec::new());
    }

    let mut actions = Vec::new();
    for line in source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("--"))
    {
        if let Some(argument) = controller_call_argument(line, "sa.input.key") {
            actions.push(MacroAction::key(argument)?);
        } else if let Some(argument) = controller_call_argument(line, "sa.input.key_down") {
            actions.push(MacroAction::key_down(argument)?);
        } else if let Some(argument) = controller_call_argument(line, "sa.input.key_up") {
            actions.push(MacroAction::key_up(argument)?);
        } else if let Some(argument) = controller_call_argument(line, "sa.input.text") {
            actions.push(MacroAction::text(argument)?);
        } else if let Some(argument) = controller_call_argument(line, "sa.input.mouse_click") {
            actions.push(MacroAction::mouse_click(MouseButton::parse(argument)?));
        } else if supported_imperative_callback_api(line) {
            continue;
        } else if line.starts_with("sa.") {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("unsupported Lua controller callback API '{line}'"),
            ));
        }
    }
    Ok(actions)
}

fn supported_imperative_callback_api(line: &str) -> bool {
    [
        "sa.sleep",
        "sa.window.active",
        "sa.window.find",
        "sa.window.activate",
        "sa.window.wait_active",
        "sa.input.key",
        "sa.input.key_down",
        "sa.input.key_up",
        "sa.input.text",
        "sa.input.mouse_click",
    ]
    .iter()
    .any(|api| line.starts_with(api))
}

fn controller_call_argument<'a>(line: &'a str, api_name: &str) -> Option<&'a str> {
    line.strip_prefix(api_name)
        .and_then(|rest| {
            let rest = rest.trim_start();
            rest.strip_prefix('(')
                .unwrap_or(rest)
                .trim_start()
                .strip_prefix('"')
        })
        .and_then(|rest| {
            let end = rest.find('"')?;
            if rest[end + 1..].trim_start().starts_with("..") {
                None
            } else {
                Some(&rest[..end])
            }
        })
}

fn load_controller_source_tree(
    path: &Path,
    root: &Path,
    visited: &mut BTreeSet<PathBuf>,
) -> Result<String, DiagnosableError> {
    let canonical_root = root.canonicalize().map_err(|error| {
        DiagnosableError::new(
            ErrorPhase::ScriptLoad,
            format!(
                "cannot resolve Lua controller root '{}': {error}",
                root.display()
            ),
        )
    })?;
    let canonical_path = path.canonicalize().map_err(|error| {
        DiagnosableError::new(
            ErrorPhase::ScriptLoad,
            format!(
                "cannot resolve Lua controller file '{}': {error}",
                path.display()
            ),
        )
    })?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!(
                "Lua controller import '{}' is outside script root '{}'",
                canonical_path.display(),
                canonical_root.display()
            ),
        ));
    }
    if !visited.insert(canonical_path.clone()) {
        return Ok(String::new());
    }
    let source = fs::read_to_string(&canonical_path).map_err(|error| {
        DiagnosableError::new(
            ErrorPhase::ScriptLoad,
            format!(
                "cannot read Lua controller file '{}': {error}",
                canonical_path.display()
            ),
        )
    })?;
    let mut combined = String::new();
    for import in controller_imports(&source) {
        let import_path = resolve_controller_import(&canonical_root, import)?;
        combined.push_str(&load_controller_source_tree(
            &import_path,
            &canonical_root,
            visited,
        )?);
        combined.push('\n');
    }
    combined.push_str(&source);
    Ok(combined)
}

fn load_controller_source_tree_file(path: &Path) -> Result<String, DiagnosableError> {
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let mut visited = BTreeSet::new();
    load_controller_source_tree(path, root, &mut visited)
}

fn controller_imports(source: &str) -> Vec<&str> {
    let mut imports = Vec::new();
    let mut cursor = source;
    while let Some(start) = cursor.find("sa.import") {
        cursor = &cursor[start + "sa.import".len()..];
        let Some(paren) = cursor.find('(') else {
            break;
        };
        cursor = &cursor[paren + 1..];
        let Some(quote) = cursor.find('"') else {
            break;
        };
        cursor = &cursor[quote + 1..];
        let Some(end_quote) = cursor.find('"') else {
            break;
        };
        imports.push(&cursor[..end_quote]);
        cursor = &cursor[end_quote + 1..];
    }
    imports
}

fn resolve_controller_import(root: &Path, import: &str) -> Result<PathBuf, DiagnosableError> {
    if import.contains("..") || import.starts_with('/') || import.chars().any(char::is_control) {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "Lua controller import must be a local module path inside the script root",
        ));
    }
    let mut path = root.join(import.replace('.', "/"));
    if path.extension().is_none() {
        path.set_extension("lua");
    }
    Ok(path)
}

fn parse_controller_entries(
    source: &str,
    api_name: &str,
    kind: ControllerRegistrationKind,
) -> Result<Vec<ControllerRegistration>, DiagnosableError> {
    let mut entries = Vec::new();
    let mut cursor = source;
    while let Some(start) = cursor.find(api_name) {
        cursor = &cursor[start + api_name.len()..];
        let Some(paren) = cursor.find('(') else {
            break;
        };
        cursor = &cursor[paren + 1..];
        let Some(block_start) = cursor.find('{') else {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("{api_name} requires a table argument"),
            ));
        };
        let block = &cursor[block_start..];
        let Some(block_end) = matching_table_end(block) else {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("{api_name} registration table is not closed"),
            ));
        };
        let entry = &block[1..block_end];
        entries.push(parse_controller_entry(entry, kind)?);
        cursor = &block[block_end + 1..];
    }
    Ok(entries)
}

fn matching_table_end(source: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (index, ch) in source.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
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

fn parse_controller_entry(
    source: &str,
    kind: ControllerRegistrationKind,
) -> Result<ControllerRegistration, DiagnosableError> {
    let trigger = field_string(source, "trigger")
        .or_else(|| {
            if kind == ControllerRegistrationKind::Shutdown {
                Some("shutdown")
            } else {
                None
            }
        })
        .ok_or_else(|| {
            DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "controller trigger is required",
            )
        })?;
    let callback = field_string(source, "callback")
        .or_else(|| field_string(source, "handler"))
        .ok_or_else(|| {
            DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "controller callback is required",
            )
        })?;
    let mode = BindingMode::parse(field_string(source, "mode"))?;
    let scope = parse_controller_scope(source)?;
    let required_capabilities = parse_controller_capabilities(source)?
        .unwrap_or_else(|| controller_required_capabilities(kind, mode, &scope));
    Ok(ControllerRegistration::new(
        kind,
        trigger,
        scope.clone(),
        mode,
        callback,
        required_capabilities,
    )?
    .with_requires_held(parse_controller_requires_held(source)?)
    .with_loop_policy(parse_controller_loop_policy(source)?))
}

fn parse_controller_capabilities(source: &str) -> Result<Option<CapabilitySet>, DiagnosableError> {
    let Some(body) = table_body_field_after(source, "capabilities")? else {
        return Ok(None);
    };
    let capabilities = quoted_strings(body)
        .into_iter()
        .map(parse_controller_capability)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(CapabilitySet::new(capabilities)))
}

fn parse_controller_capability(name: &str) -> Result<CapabilityKind, DiagnosableError> {
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

fn parse_controller_requires_held(source: &str) -> Result<HeldCondition, DiagnosableError> {
    let Some(body) = table_body_field_after(source, "requires_held")? else {
        return HeldCondition::new(Vec::new());
    };
    HeldCondition::parse(quoted_strings(body))
}

fn parse_controller_loop_policy(
    source: &str,
) -> Result<Option<ControllerLoopPolicy>, DiagnosableError> {
    let Some(loop_body) = table_body_field_after(source, "loop")? else {
        return Ok(None);
    };
    let while_held_body = table_body_field_after(loop_body, "while_held")?.ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "controller loop requires while_held",
        )
    })?;
    let repeat_body = table_body_field_after(loop_body, "repeat")?.ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "controller loop requires repeat",
        )
    })?;
    let every_ms = field_u64(repeat_body, "every_ms")?.ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "controller loop repeat requires every_ms",
        )
    })?;
    let repeat_callback = field_string(repeat_body, "callback").ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "controller loop repeat requires callback",
        )
    })?;
    ControllerLoopPolicy::new(
        MotionTrigger::parse(quoted_strings(while_held_body))?,
        field_string(loop_body, "before"),
        every_ms,
        repeat_callback,
        field_string(loop_body, "after"),
    )
    .map(Some)
}

fn parse_controller_scope(source: &str) -> Result<ScopeSelection, DiagnosableError> {
    let Some(scope_body) = table_body_after(source, "scope")? else {
        return Ok(ScopeSelection::ExplicitGlobal);
    };
    parse_process_scope_body(scope_body)
}

fn parse_process_scope_body(scope_body: &str) -> Result<ScopeSelection, DiagnosableError> {
    if scope_body.contains("global") {
        return Ok(ScopeSelection::ExplicitGlobal);
    }
    let processes = quoted_strings(scope_body)
        .into_iter()
        .map(ProcessName::parse)
        .collect::<Result<Vec<_>, _>>()?;
    ScopeSelection::process_list(processes)
}

fn controller_required_capabilities(
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
        ControllerRegistrationKind::Timer | ControllerRegistrationKind::Shutdown => {}
    }
    if matches!(scope, ScopeSelection::ProcessList { .. }) {
        required.push(CapabilityKind::ActiveProcessMetadata);
    }
    CapabilitySet::new(required)
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
        source.find("presses"),
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
    let requires_held = parse_requires_held(source, leader_defined)?;
    let inter_action_delay_ms =
        field_u64(source, "inter_action_delay_ms")?.unwrap_or(defaults.inter_action_delay_ms);
    let within_ms = field_u64(source, "within_ms")?
        .unwrap_or(signal_auras_core::DEFAULT_MOTION_DURATION.as_millis() as u64);

    if top_level_field_index(source, "repeat").is_some() {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "motions[].repeat was removed; use motions[].loop.repeat.every_ms",
        ));
    }

    let before_loop = top_level_field_index(source, "loop")
        .map(|index| &source[..index])
        .unwrap_or(source);
    let macro_definition = if top_level_field_index(before_loop, "macro").is_some() {
        Some(parse_macro_field(before_loop)?)
    } else {
        None
    };
    let loop_definition = table_body_field_after(source, "loop")?
        .map(parse_loop_definition)
        .transpose()?;
    MotionDefinition::with_requires_held(
        requires_held,
        trigger,
        mode,
        macro_definition,
        loop_definition,
        within_ms,
        inter_action_delay_ms,
    )
}

fn parse_presses(
    source: &str,
    defaults: &AutomationDefaults,
    leader_defined: bool,
) -> Result<Vec<PressDefinition>, DiagnosableError> {
    let Some(presses_body) = table_body_after(source, "presses")? else {
        return Ok(Vec::new());
    };
    let mut result = Vec::new();
    for entry in top_level_tables(presses_body) {
        result.push(parse_press_entry(entry, defaults, leader_defined)?);
    }
    Ok(result)
}

fn parse_press_entry(
    source: &str,
    defaults: &AutomationDefaults,
    leader_defined: bool,
) -> Result<PressDefinition, DiagnosableError> {
    let mode = BindingMode::parse(field_string(source, "mode"))?;
    if top_level_field_uses_table(source, "trigger") {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "press trigger must be a single token string",
        ));
    }
    let trigger = MotionToken::parse(field_string(source, "trigger").ok_or_else(|| {
        DiagnosableError::new(ErrorPhase::ScriptValidation, "press requires trigger")
    })?)?;
    if matches!(trigger, MotionToken::Leader) && !leader_defined {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "press trigger uses <Leader> but leader is not configured",
        ));
    }
    let requires_held = parse_requires_held(source, leader_defined)?;
    let inter_action_delay_ms =
        field_u64(source, "inter_action_delay_ms")?.unwrap_or(defaults.inter_action_delay_ms);
    let macro_definition = parse_macro_field(source)?;
    Ok(PressDefinition::new(
        requires_held,
        trigger,
        mode,
        macro_definition,
        inter_action_delay_ms,
    ))
}

fn parse_requires_held(
    source: &str,
    leader_defined: bool,
) -> Result<HeldCondition, DiagnosableError> {
    let Some(body) = table_body_field_after(source, "requires_held")? else {
        return HeldCondition::new(Vec::new());
    };
    let condition = HeldCondition::parse(quoted_strings(body))?;
    if condition.contains_leader() && !leader_defined {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "requires_held uses <Leader> but leader is not configured",
        ));
    }
    Ok(condition)
}

fn parse_loop_definition(source: &str) -> Result<LoopDefinition, DiagnosableError> {
    if source.contains("function") {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "callback loop bodies are reserved for a later design",
        ));
    }
    let while_held_body = table_body_field_after(source, "while_held")?.ok_or_else(|| {
        DiagnosableError::new(ErrorPhase::ScriptValidation, "loop requires while_held")
    })?;
    let before = parse_optional_macro_named_field(source, "before")?;
    let after = parse_optional_macro_named_field(source, "after")?;
    let has_once = top_level_field_index(source, "once").is_some();
    let has_repeat = top_level_field_index(source, "repeat").is_some();
    let has_next = top_level_field_index(source, "next").is_some();
    if has_next {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "callback loop bodies are reserved for a later design",
        ));
    }
    if usize::from(has_once) + usize::from(has_repeat) != 1 {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "loop must define exactly one body: once or repeat",
        ));
    }
    let body = if has_once {
        LoopBody::Once(parse_macro_named_field(source, "once")?)
    } else {
        let repeat_body = table_body_field_after(source, "repeat")?.ok_or_else(|| {
            DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "loop repeat must use table constructor",
            )
        })?;
        let every_ms = field_u64(repeat_body, "every_ms")?.ok_or_else(|| {
            DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "loop repeat requires every_ms",
            )
        })?;
        LoopBody::Repeat(LoopRepeat::new(
            LoopInterval::new(every_ms)?,
            parse_macro_field(repeat_body)?,
        ))
    };
    Ok(LoopDefinition::new(
        MotionTrigger::parse(quoted_strings(while_held_body))?,
        before,
        body,
        after,
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

fn parse_optional_macro_named_field(
    source: &str,
    field: &str,
) -> Result<Option<MacroDefinition>, DiagnosableError> {
    if top_level_field_index(source, field).is_some() {
        parse_macro_named_field(source, field).map(Some)
    } else {
        Ok(None)
    }
}

fn parse_macro_named_field(source: &str, field: &str) -> Result<MacroDefinition, DiagnosableError> {
    let Some(field_index) = top_level_field_index(source, field) else {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("{field} must map to a macro"),
        ));
    };
    let after_field = &source[field_index + field.len()..];
    let Some(macro_start) = after_field.find("macro") else {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("{field} must map to a macro"),
        ));
    };
    parse_macro_field(&after_field[macro_start..])
}

fn parse_actions(source: &str) -> Result<Vec<MacroAction>, DiagnosableError> {
    let mut actions = Vec::new();
    for line in source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("--"))
    {
        if let Some(rest) = line.strip_prefix("key ") {
            actions.push(MacroAction::key(first_quoted(rest).ok_or_else(|| {
                DiagnosableError::new(ErrorPhase::ScriptValidation, "key action needs a string")
            })?)?);
        } else if let Some(rest) = line.strip_prefix("key_down ") {
            actions.push(MacroAction::key_down(first_quoted(rest).ok_or_else(
                || {
                    DiagnosableError::new(
                        ErrorPhase::ScriptValidation,
                        "key_down action needs a string",
                    )
                },
            )?)?);
        } else if let Some(rest) = line.strip_prefix("key_up ") {
            actions.push(MacroAction::key_up(first_quoted(rest).ok_or_else(
                || {
                    DiagnosableError::new(
                        ErrorPhase::ScriptValidation,
                        "key_up action needs a string",
                    )
                },
            )?)?);
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

fn field_i64(source: &str, field: &str) -> Result<Option<i64>, DiagnosableError> {
    let Some(index) = top_level_field_index(source, field) else {
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
            format!("{field} needs an integer"),
        ));
    }
    value.parse::<i64>().map(Some).map_err(|_| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("{field} needs an integer"),
        )
    })
}

fn field_f32(source: &str, field: &str) -> Result<Option<f32>, DiagnosableError> {
    let Some(index) = top_level_field_index(source, field) else {
        return Ok(None);
    };
    let after_field = &source[index + field.len()..];
    let Some(equals) = after_field.find('=') else {
        return Ok(None);
    };
    let value = after_field[equals + 1..]
        .trim_start()
        .chars()
        .take_while(|character| {
            character.is_ascii_digit() || *character == '-' || *character == '.'
        })
        .collect::<String>();
    if value.is_empty() {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("{field} needs a number"),
        ));
    }
    value.parse::<f32>().map(Some).map_err(|_| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("{field} needs a number"),
        )
    })
}

fn field_bool(source: &str, field: &str) -> Option<bool> {
    let index = top_level_field_index(source, field)?;
    let after_field = &source[index + field.len()..];
    let equals = after_field.find('=')?;
    let value = after_field[equals + 1..].trim_start();
    if value.starts_with("true") {
        Some(true)
    } else if value.starts_with("false") {
        Some(false)
    } else {
        None
    }
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

fn field_identifier<'a>(source: &'a str, field: &str) -> Option<&'a str> {
    let index = top_level_field_index(source, field)?;
    let after_field = &source[index + field.len()..];
    let equals = after_field.find('=')?;
    let value = after_field[equals + 1..].trim_start();
    if value.starts_with('{') || value.starts_with('"') {
        return None;
    }
    let end = value
        .find(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '_' || character == '-')
        })
        .unwrap_or(value.len());
    let identifier = &value[..end];
    if identifier.is_empty() {
        None
    } else {
        Some(identifier)
    }
}

fn top_level_table_body<'a>(
    source: &'a str,
    field: &str,
) -> Result<Option<&'a str>, DiagnosableError> {
    let Some(field_index) = top_level_field_index(source, field) else {
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

fn table_body_field_after<'a>(
    source: &'a str,
    field: &str,
) -> Result<Option<&'a str>, DiagnosableError> {
    let Some(field_index) = top_level_field_index(source, field) else {
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

fn top_level_field_index(source: &str, field: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    let field_bytes = field.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut index = 0usize;
    while index < bytes.len() {
        match (bytes[index], in_string) {
            (b'"', false) => in_string = true,
            (b'"', true) => in_string = false,
            (b'{', false) => depth += 1,
            (b'}', false) => depth = depth.saturating_sub(1),
            _ => {}
        }
        if !in_string
            && depth == 0
            && bytes[index..].starts_with(field_bytes)
            && is_identifier_boundary(bytes, index, field.len())
        {
            let after = index + field.len();
            if bytes[after..]
                .iter()
                .copied()
                .find(|byte| !byte.is_ascii_whitespace())
                == Some(b'=')
            {
                return Some(index);
            }
        }
        index += 1;
    }
    None
}

fn top_level_field_uses_table(source: &str, field: &str) -> bool {
    let Some(index) = top_level_field_index(source, field) else {
        return false;
    };
    let after_field = &source[index + field.len()..];
    let Some(equals) = after_field.find('=') else {
        return false;
    };
    after_field[equals + 1..].trim_start().starts_with('{')
}

fn is_identifier_boundary(bytes: &[u8], start: usize, len: usize) -> bool {
    let before = start.checked_sub(1).and_then(|index| bytes.get(index));
    let after = bytes.get(start + len);
    !before.is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        && !after.is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
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
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_controller_registrations_without_runtime_activation() {
        let controller = load_lua_controller_source(
            r#"
            sa.hotkey({
              trigger = "F5",
              scope = { processes = { "poe2.exe" } },
              callback = "hideout",
            })
            sa.motion({
              trigger = "<Leader> x",
              mode = "passthrough",
              callback = "motion",
            })
            "#,
        )
        .unwrap();

        assert_eq!(controller.registrations().len(), 2);
        assert!(controller
            .required_capabilities()
            .contains(CapabilityKind::GlobalShortcut));
        assert!(controller
            .required_capabilities()
            .contains(CapabilityKind::ActiveProcessMetadata));
        assert!(controller
            .required_capabilities()
            .contains(CapabilityKind::CompositePointerObservation));
        assert!(!controller
            .required_capabilities()
            .contains(CapabilityKind::CompositePointerConsumption));
    }

    #[test]
    fn parses_state_trackers_without_callbacks_or_input_capabilities() {
        let program = load_lua_controller_program_source(
            r#"
            poe = { processes = { "steam_app_2694490", "PathOfExileSteam.exe" } }

            sa.state.track({
              id = "refutation_cooldown",
              scope = poe,
              capabilities = { "screen_read" },
              poll_ms = 50,
              detector = {
                kind = "radial_cooldown",
                roi = { x = 2850, y = 2030, w = 36, h = 36 },
                mask = { shape = "circle", inset = 10 },
                phases = {
                  order = { "ready", "activated", "active", "recovering" },
                  fallback = "unknown",
                  ready = {
                    sample = { kind = "clock_probe", angle_deg = 352, radius_px = 15, w = 3, h = 3 },
                    min_luminance_percent = 44,
                    min_saturation = 85,
                    progress_fill = "full",
                  },
                  activated = {
                    sample = { kind = "clock_probe", angle_deg = 8, radius_px = 15, w = 3, h = 3 },
                    max_luminance_percent = 12,
                    max_saturation = 20,
                    progress_fill = "empty",
                  },
                  active = {
                    sample = { kind = "clock_probe", angle_deg = 8, radius_px = 15, w = 3, h = 3 },
                    max_luminance_percent = 34,
                    max_saturation = 75,
                    progress_fill = "empty",
                  },
                  recovering = {
                    sample = { kind = "annulus_arc", inner_radius_px = 13, outer_radius_px = 17, start_deg = 20, end_deg = 340 },
                    min_luminance_percent = 40,
                    min_saturation = 80,
                    metric = "bright_ratio",
                    metric_scale = 1.5,
                    progress_fill = "fraction",
                    max_fill_until_ready = 0.95,
                  },
                },
              },
            })
            "#,
        )
        .unwrap();

        assert_eq!(program.registrations().registrations().len(), 0);
        assert_eq!(program.state_trackers().trackers().len(), 1);
        assert!(program
            .required_capabilities()
            .contains(CapabilityKind::ScreenRead));
        assert!(!program
            .required_capabilities()
            .contains(CapabilityKind::SynthesizedInput));
    }

    #[test]
    fn rejects_radial_detector_phase_style_fields() {
        for field in [
            r##"fill = "#f97316""##,
            r##"background = "#7f1d1d""##,
            "opacity = 0.85",
        ] {
            let source = format!(
                r#"
                sa.state.track({{
                  id = "refutation_cooldown",
                  capabilities = {{ "screen_read" }},
                  poll_ms = 50,
                  detector = {{
                    kind = "radial_cooldown",
                    roi = {{ x = 0, y = 0, w = 36, h = 36 }},
                    phases = {{
                      order = {{ "ready" }},
                      fallback = "unknown",
                      ready = {{
                        sample = {{ kind = "clock_probe", angle_deg = 352, radius_px = 15, w = 3, h = 3 }},
                        min_luminance_percent = 44,
                        min_saturation = 85,
                        progress_fill = "full",
                        {field},
                      }},
                    }},
                  }},
                }})
                "#
            );
            let error = load_lua_controller_program_source(&source).unwrap_err();
            assert!(
                error.message.contains("radial_cooldown phase style field"),
                "unexpected error for {field}: {error}"
            );
        }
    }

    #[test]
    fn parses_overlay_activated_and_active_visual_styles() {
        let program = load_lua_controller_program_source(
            r##"
            poe = { processes = { "PathOfExileSteam.exe" } }

            sa.state.track({
              id = "refutation_cooldown",
              scope = poe,
              capabilities = { "screen_read" },
              poll_ms = 50,
              detector = {
                kind = "radial_cooldown",
                roi = { x = 0, y = 0, w = 36, h = 36 },
                phases = {
                  order = { "ready" },
                  fallback = "unknown",
                  ready = {
                    sample = { kind = "clock_probe", angle_deg = 352, radius_px = 15, w = 3, h = 3 },
                    min_luminance_percent = 44,
                    min_saturation = 85,
                    progress_fill = "full",
                  },
                },
              },
            })

            sa.overlay.mount({
              id = "poe2_status",
              scope = poe,
              provider = "native",
              surface = "overlay",
              hotkey = { trigger = "Shift+F1", mode = "consume" },
              visuals = {
                {
                  id = "refutation",
                  kind = "progress_bar",
                  bind = { tracker = "refutation_cooldown", field = "remaining_ms" },
                  rect = { x = 1200, y = 930, w = 150, h = 22 },
                  opacity = 0.72,
                  fill = "#5aa7ff",
                  background = "#101820",
                  activated = { fill = "#f97316", background = "#7f1d1d", opacity = 0.85 },
                  active = { fill = "#38bdf8", background = "#082f49", opacity = 0.8 },
                },
              },
            })
            "##,
        )
        .unwrap();

        let visual = match &program.overlays().overlays()[0].visuals[0] {
            VisualDefinition::ProgressBar(visual) => visual,
        };
        let toggle = program.overlays().overlays()[0]
            .toggle_hotkey
            .as_ref()
            .unwrap();
        assert_eq!(toggle.trigger.as_str(), "Shift+F1");
        assert_eq!(toggle.mode, BindingMode::Consume);
        assert_eq!(
            visual
                .activated_style
                .as_ref()
                .and_then(|style| style.fill.as_deref()),
            Some("#f97316")
        );
        assert_eq!(
            visual
                .active_style
                .as_ref()
                .and_then(|style| style.background.as_deref()),
            Some("#082f49")
        );
    }

    #[test]
    fn rejects_duplicate_controller_triggers() {
        let error = load_lua_controller_source(
            r#"
            sa.hotkey({ trigger = "Return", callback = "first" })
            sa.hotkey({ trigger = "Return", callback = "second" })
            "#,
        )
        .unwrap_err();

        assert!(error.message.contains("duplicate controller trigger"));
    }

    #[test]
    fn controller_loader_denies_ambient_apis() {
        for source in [
            r#"sa.hotkey({ trigger = "F5", callback = "ok" }); os.execute("id")"#,
            r#"sa.hotkey({ trigger = "F5", callback = "ok" }); require("socket")"#,
            r#"sa.hotkey({ trigger = "F5", callback = "ok" }); debug.traceback()"#,
        ] {
            assert!(
                load_lua_controller_source(source).is_err(),
                "source should be denied: {source}"
            );
        }
    }

    #[test]
    fn controller_loader_allows_denied_words_in_safe_lua_values() {
        let controller = load_lua_controller_source(
            r#"
            local require = "documentation only"
            local note = "io.open and debug.traceback are not executed"
            sa.hotkey({ trigger = "F5", callback = "ok" })
            "#,
        )
        .unwrap();

        assert_eq!(controller.registrations().len(), 1);
    }

    #[test]
    fn controller_loader_denies_structured_ambient_globals() {
        for source in [
            r#"sa.hotkey({ trigger = "F5", callback = "ok" }); io.open("/etc/passwd")"#,
            r#"sa.hotkey({ trigger = "F5", callback = "ok" }); load("return 1")"#,
            r#"sa.hotkey({ trigger = "F5", callback = "ok" }); dofile("other.lua")"#,
        ] {
            assert!(
                load_lua_controller_source(source).is_err(),
                "source should be denied: {source}"
            );
        }
    }

    #[test]
    fn controller_program_denies_ambient_apis_inside_callbacks() {
        for body in [
            r#"io.open("/etc/passwd")"#,
            r#"load("return 1")"#,
            r#"debug.traceback()"#,
        ] {
            let source = format!(
                r#"
                sa.hotkey({{ trigger = "F5", callback = "ok" }})
                sa.callback("ok", function()
                  {body}
                end)
                "#
            );
            assert!(
                load_lua_controller_program_source(&source).is_err(),
                "callback body should be denied: {body}"
            );
        }
    }

    #[test]
    fn controller_loader_imports_local_modules_from_script_root() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("signal-auras-controller-{unique}"));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(
            root.join("helper.lua"),
            r#"sa.motion({ trigger = "<Leader> x", callback = "helper_motion" })"#,
        )
        .unwrap();
        let main = root.join("main.lua");
        std::fs::write(
            &main,
            r#"
            sa.import("helper")
            sa.hotkey({ trigger = "F5", callback = "main_hotkey" })
            "#,
        )
        .unwrap();

        let controller = load_lua_controller_file(&main).unwrap();

        assert_eq!(controller.registrations().len(), 2);
        std::fs::remove_file(root.join("helper.lua")).unwrap();
        std::fs::remove_file(main).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn controller_program_parses_callback_output_apis() {
        let program = load_lua_controller_program_source(
            r#"
            sa.hotkey({ trigger = "F5", callback = "hideout" })
            sa.callback("hideout", function()
              sa.input.key("Enter")
              sa.input.text("/hideout")
              sa.input.key("Enter")
            end)
            "#,
        )
        .unwrap();

        let callback = program.callback("hideout").unwrap();
        assert_eq!(callback.actions.len(), 3);
        assert!(program
            .required_capabilities()
            .contains(CapabilityKind::SynthesizedInput));
    }

    #[test]
    fn controller_program_requires_callback_definition() {
        let error = load_lua_controller_program_source(
            r#"sa.hotkey({ trigger = "F5", callback = "missing" })"#,
        )
        .unwrap_err();

        assert!(error.message.contains("registered but not defined"));
    }

    #[test]
    fn controller_program_rejects_unsupported_callback_api() {
        let error = load_lua_controller_program_source(
            r#"
            sa.hotkey({ trigger = "F5", callback = "hideout" })
            sa.callback("hideout", function()
              sa.shell("nope")
            end)
            "#,
        )
        .unwrap_err();

        assert!(error
            .message
            .contains("unsupported Lua controller callback API"));
    }

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
                  loop = {
                    while_held = { "<Leader>", "<LClick>" },
                    repeat = {
                      every_ms = 50,
                      macro = macro {
                        mouse_click "left",
                      },
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
        assert!(repeat_motion
            .loop_definition
            .as_ref()
            .and_then(|loop_definition| loop_definition.repeat())
            .is_some());
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
    fn parses_guarded_motions_and_presses() {
        let source = r#"
            return {
              leader = "F13",
              motions = {
                {
                  requires_held = { "<Leader>" },
                  trigger = { "<LClick>", "<LClick>" },
                  mode = "passthrough",
                  loop = {
                    while_held = { "<LClick>" },
                    repeat = {
                      every_ms = 50,
                      macro = macro { mouse_click "left" },
                    },
                  },
                },
              },
              presses = {
                {
                  requires_held = { "<Leader>" },
                  trigger = "<WheelUp>",
                  mode = "passthrough",
                  macro = macro { key "Left" },
                },
              },
            }
        "#;

        let config = load_lua_source(source).unwrap();
        let motion = config.motions().values().next().unwrap();
        let press = config.presses().values().next().unwrap();

        assert_eq!(motion.requires_held.tokens(), &[MotionToken::Leader]);
        assert_eq!(press.trigger, MotionToken::Wheel(WheelDirection::Up));
        assert_eq!(press.mode, BindingMode::Passthrough);
    }

    #[test]
    fn rejects_invalid_guarded_press_shapes() {
        for source in [
            r#"return { leader = "F13", presses = { { requires_held = { "<WheelUp>" }, trigger = "<WheelDown>", macro = macro { key "Right" } } } }"#,
            r#"return { leader = "F13", presses = { { requires_held = { "<Leader>" }, macro = macro { key "Right" } } } }"#,
            r#"return { leader = "F13", presses = { { requires_held = { "<Leader>" }, trigger = { "<WheelDown>", "<WheelDown>" }, macro = macro { key "Right" } } } }"#,
            r#"return { leader = "F13", presses = { { requires_held = { "<Leader>" }, trigger = "<WheelDown>" } } }"#,
            r#"return { presses = { { requires_held = { "<Leader>" }, trigger = "<WheelDown>", macro = macro { key "Right" } } } }"#,
        ] {
            assert!(
                load_lua_source(source).is_err(),
                "source should be denied: {source}"
            );
        }
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
            r#"return { motions = { { trigger = { "f" }, loop = { while_held = { "f" }, repeat = { every_ms = 0, macro = macro { text "x" } } } } } }"#,
            r#"return { motions = { { trigger = { "f" }, loop = { while_held = { "f" }, repeat = { every_ms = 50 } } } } }"#,
            r#"return { motions = { { trigger = { "f" }, loop = { repeat = { every_ms = 50, macro = macro { text "x" } } } } } }"#,
            r#"return { motions = { { trigger = { "f" }, loop = { while_held = { "f" }, once = macro { text "x" }, repeat = { every_ms = 50, macro = macro { text "x" } } } } } }"#,
            r#"return { motions = { { trigger = { "f" }, loop = { while_held = { "f" }, next = function() end } } } }"#,
            r#"return { motions = { { trigger = { "f" }, repeat = { while_held = { "f" }, interval_ms = { min = 50, max = 80 }, macro = macro { text "x" } } } } }"#,
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
