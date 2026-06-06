use signal_auras_core::{
    CapabilityKind, CapabilitySet, DetectorDefinition, MacroAction, RadialCooldownPhase,
    RendererProviderId, StateField,
};
use signal_auras_lua::{
    load_lua_controller_program_file, load_lua_controller_program_source,
    load_lua_controller_runtime_source_file, load_lua_file, load_lua_source, ActiveWindowMetadata,
    ImperativeLuaController, LuaCallbackStep, LuaExecutionBudget, LuaHostRequest, LuaHostResponse,
    LuaLogLevel,
};
use std::{fs, path::Path, time::Duration};

#[test]
fn lua_api_accepts_v1_sample() {
    let config = load_lua_source(
        r#"
        return {
          scope = { processes = { "poe2.exe" } },
          hotkeys = {
            ["F5"] = macro {
              key "Enter",
              text "/hideout",
              key "Enter",
            },
          },
        }
        "#,
    )
    .unwrap();

    assert_eq!(config.hotkeys().len(), 1);
}

#[test]
fn lua_api_accepts_poe2_legacy_example() {
    let config = load_lua_file(Path::new("examples/poe2-legacy.lua")).unwrap();

    assert_eq!(config.motions().len(), 2);
    assert_eq!(config.presses().len(), 4);
}

#[test]
fn lua_api_accepts_poe2_controller_example() {
    let program = load_lua_controller_program_file(Path::new("examples/poe2.lua")).unwrap();

    assert_eq!(program.registrations().registrations().len(), 7);
    assert_eq!(program.state_trackers().trackers().len(), 2);
    assert!(program.input_provider.is_some());
    assert!(program.leader.is_some());
    assert!(!program
        .required_capabilities()
        .contains(signal_auras_core::CapabilityKind::GlobalShortcut));
    assert!(program
        .required_capabilities()
        .contains(signal_auras_core::CapabilityKind::CompositePointerObservation));
    assert!(program
        .required_capabilities()
        .contains(signal_auras_core::CapabilityKind::ActiveWindowMetadata));
    assert!(program
        .required_capabilities()
        .contains(signal_auras_core::CapabilityKind::WindowActivation));
    assert!(program
        .required_capabilities()
        .contains(signal_auras_core::CapabilityKind::Timer));
    assert!(program
        .required_capabilities()
        .contains(signal_auras_core::CapabilityKind::ScreenRead));
    assert!(program.callback("go_home").is_some());
    assert!(program
        .callback("reload_filterblade")
        .is_some_and(|callback| callback.actions.is_empty()));
    let loop_motion = program
        .registrations()
        .registrations()
        .iter()
        .find(|registration| registration.trigger == "<LClick> <LClick>")
        .and_then(|registration| registration.loop_policy.as_ref())
        .unwrap();
    assert_eq!(loop_motion.repeat_every_ms, 40);
    assert_eq!(loop_motion.repeat_callback, "click_left");
    assert!(program.callback("ctrl_click").is_some());

    let refutation_visual = program.overlays().overlays()[0]
        .visuals
        .iter()
        .find_map(|visual| match visual {
            signal_auras_core::VisualDefinition::ProgressBar(visual)
                if visual.id == "refutation" =>
            {
                Some(visual)
            }
            _ => None,
        })
        .unwrap();
    assert!(refutation_visual.activated_style.is_some());
    assert!(refutation_visual.active_style.is_some());
}

#[test]
fn state_trackers_accept_poe2_example_without_tracker_callbacks() {
    let program = load_lua_controller_program_file(Path::new("examples/poe2.lua")).unwrap();
    let trackers = program.state_trackers().trackers();

    let refutation = trackers
        .iter()
        .find(|tracker| tracker.id == "refutation_cooldown")
        .unwrap();
    assert_eq!(refutation.poll_ms, 50);
    assert!(matches!(
        &refutation.detector,
        DetectorDefinition::RadialCooldown {
            roi,
            mask: Some(mask),
            phases,
        } if roi.x == 1923
            && roi.y == 1370
            && roi.w == 36
            && roi.h == 36
            && mask.inset == 10
            && phases.order.len() == 4
            && phases.prediction.is_some_and(|prediction| prediction.duration_ms == 7_000
                && prediction.stable_after_ms == 500)
    ));

    let heavy_stun = trackers
        .iter()
        .find(|tracker| tracker.id == "heavy_stun")
        .unwrap();
    assert_eq!(heavy_stun.poll_ms, 50);
    assert!(matches!(
        &heavy_stun.condition,
        Some(condition)
            if condition.tracker_id == "refutation_cooldown"
                && condition.phase == RadialCooldownPhase::Active
    ));
    assert!(matches!(
        heavy_stun.detector,
        DetectorDefinition::HorizontalProgressBar { ref roi, .. }
            if roi.x == 315 && roi.y == 1250 && roi.w == 300 && roi.h == 2
    ));

    assert!(program.callback("refutation_cooldown").is_none());
    assert!(program.callback("heavy_stun").is_none());
    assert!(program
        .state_trackers()
        .required_capabilities()
        .contains(CapabilityKind::ScreenRead));
}

#[test]
fn overlay_api_accepts_poe2_progress_bars_without_macro_reactions() {
    let program = load_lua_controller_program_source(&overlay_source("native")).unwrap();

    assert_eq!(program.state_trackers().trackers().len(), 2);
    assert_eq!(program.overlays().overlays().len(), 1);
    assert_eq!(
        program.overlays().overlays()[0].provider,
        RendererProviderId::Native
    );
    assert_eq!(program.overlays().overlays()[0].visuals.len(), 2);
    assert!(program.callback("poe2_status").is_none());
    assert!(!program
        .required_capabilities()
        .contains(CapabilityKind::SynthesizedInput));
    assert!(program
        .required_capabilities()
        .contains(CapabilityKind::ScreenRead));
    assert!(program
        .required_capabilities()
        .contains(CapabilityKind::ActiveProcessMetadata));
}

#[test]
fn overlay_api_accepts_radial_activated_and_active_styles() {
    let program = load_lua_controller_program_source(&overlay_source("native")).unwrap();
    let refutation = program.overlays().overlays()[0]
        .visuals
        .iter()
        .find_map(|visual| match visual {
            signal_auras_core::VisualDefinition::ProgressBar(visual)
                if visual.id == "refutation" =>
            {
                Some(visual)
            }
            _ => None,
        })
        .unwrap();

    assert_eq!(
        refutation
            .activated_style
            .as_ref()
            .and_then(|style| style.fill.as_deref()),
        Some("#f97316")
    );
    assert_eq!(
        refutation
            .activated_style
            .as_ref()
            .and_then(|style| style.background.as_deref()),
        Some("#7f1d1d")
    );
    assert_eq!(
        refutation
            .active_style
            .as_ref()
            .and_then(|style| style.fill.as_deref()),
        Some("#38bdf8")
    );
    assert_eq!(
        refutation
            .active_style
            .as_ref()
            .and_then(|style| style.background.as_deref()),
        Some("#082f49")
    );
}

#[test]
fn overlay_api_accepts_future_provider_ids_as_declarations() {
    for provider in ["webview", "tauri_window", "tool_window"] {
        let program = load_lua_controller_program_source(&overlay_source(provider)).unwrap();
        assert_eq!(program.overlays().overlays().len(), 1);
    }
}

#[test]
fn overlay_api_rejects_invalid_provider_duplicate_visuals_rects_opacity_and_bindings() {
    let cases = [
        overlay_source("unknown_provider"),
        overlay_source_with_visuals(
            r##"
            {
              id = "dup",
              kind = "progress_bar",
              bind = { tracker = "heavy_stun", field = "progress_percent" },
              rect = { x = 0, y = 0, w = 100, h = 20 },
              opacity = 0.7,
              fill = "#d8b84c",
              background = "#101820",
            },
            {
              id = "dup",
              kind = "progress_bar",
              bind = { tracker = "heavy_stun", field = "progress_percent" },
              rect = { x = 0, y = 25, w = 100, h = 20 },
              opacity = 0.7,
              fill = "#d8b84c",
              background = "#101820",
            },
            "##,
        ),
        overlay_source_with_visuals(
            r##"
            {
              id = "bad_rect",
              kind = "progress_bar",
              bind = { tracker = "heavy_stun", field = "progress_percent" },
              rect = { x = -1, y = 0, w = 100, h = 20 },
              opacity = 0.7,
              fill = "#d8b84c",
              background = "#101820",
            },
            "##,
        ),
        overlay_source_with_visuals(
            r##"
            {
              id = "bad_opacity",
              kind = "progress_bar",
              bind = { tracker = "heavy_stun", field = "progress_percent" },
              rect = { x = 0, y = 0, w = 100, h = 20 },
              opacity = 2.0,
              fill = "#d8b84c",
              background = "#101820",
            },
            "##,
        ),
        overlay_source_with_visuals(
            r##"
            {
              id = "missing_bind",
              kind = "progress_bar",
              rect = { x = 0, y = 0, w = 100, h = 20 },
              opacity = 0.7,
              fill = "#d8b84c",
              background = "#101820",
            },
            "##,
        ),
        overlay_source_with_visuals(
            r##"
            {
              id = "phase_style_wrong_tracker",
              kind = "progress_bar",
              bind = { tracker = "heavy_stun", field = "progress_percent" },
              rect = { x = 0, y = 0, w = 100, h = 20 },
              opacity = 0.7,
              fill = "#d8b84c",
              background = "#101820",
              active = { fill = "#38bdf8" },
            },
            "##,
        ),
        overlay_source_with_visuals(
            r##"
            {
              id = "wrong_field",
              kind = "progress_bar",
              bind = { tracker = "heavy_stun", field = "remaining_ms" },
              rect = { x = 0, y = 0, w = 100, h = 20 },
              opacity = 0.7,
              fill = "#d8b84c",
              background = "#101820",
            },
            "##,
        ),
    ];

    for source in cases {
        assert!(
            load_lua_controller_program_source(&source).is_err(),
            "source should be rejected: {source}"
        );
    }
}

#[test]
fn overlay_api_rejects_authority_fields_and_sandbox_escape_attempts() {
    for field in [
        "callback = \"draw\"",
        "macro = macro { key \"Enter\" }",
        "screen = true",
        "input = true",
        "compositor = true",
        "network = true",
    ] {
        let source = overlay_source_with_extra_overlay_field(field);
        assert!(
            load_lua_controller_program_source(&source).is_err(),
            "field should be rejected: {field}"
        );
    }

    for source in [
        r#"sa.overlay.mount({ id = "bad", provider = "native", visuals = {} }); io.open("/tmp/x")"#,
        r#"sa.overlay.mount({ id = "bad", provider = "native", visuals = {} }); require("socket")"#,
        r#"sa.overlay.mount({ id = "bad", provider = "native", visuals = {} }); portal.remote_desktop()"#,
    ] {
        assert!(load_lua_controller_program_source(source).is_err());
    }
}

#[test]
fn overlay_api_preserves_typed_state_bindings() {
    let program = load_lua_controller_program_source(&overlay_source("native")).unwrap();
    let bindings = program.overlays().overlays()[0]
        .visuals
        .iter()
        .map(|visual| visual.binding().field)
        .collect::<Vec<_>>();

    assert_eq!(
        bindings,
        vec![StateField::ProgressPercent, StateField::RemainingMs]
    );
}

#[test]
fn state_trackers_reject_user_declared_emits_and_fixture_fields() {
    for field in [
        "emits = { \"ready\" }",
        "fixture = \"examples/poe2/refutation_cooldown.webm\"",
    ] {
        let source = format!(
            r#"
            sa.state.track({{
              id = "bad",
              capabilities = {{ "screen_read" }},
              poll_ms = 50,
              {field},
              detector = {{
                kind = "horizontal_progress_bar",
                roi = {{ x = 0, y = 0, w = 10, h = 10 }},
                fill = {{ direction = "left_to_right" }},
              }},
            }})
            "#
        );
        assert!(load_lua_controller_program_source(&source).is_err());
    }
}

#[test]
fn radial_cooldown_lua_phases_are_required_and_validated() {
    let valid = radial_tracker_source(
        r##"
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
        "##,
    );
    assert!(load_lua_controller_program_source(&valid).is_ok());

    for invalid in [
        radial_tracker_source(""),
        valid.replace("fallback = \"unknown\"", "fallback = \"ready\""),
        valid.replace("radius_px = 15", "radius_px = 40"),
        valid.replace(
            "inner_radius_px = 13, outer_radius_px = 17",
            "inner_radius_px = 17, outer_radius_px = 13",
        ),
        valid.replace("min_luminance_percent = 44", "min_luminance_percent = 144"),
        valid.replace(
            "progress_fill = \"full\"",
            "progress_fill = \"nearly_full\"",
        ),
    ] {
        assert!(
            load_lua_controller_program_source(&invalid).is_err(),
            "source should be rejected: {invalid}"
        );
    }
}

#[test]
fn radial_cooldown_lua_phases_reject_visual_style_fields() {
    for field in [
        r##"fill = "#f97316""##,
        r##"background = "#7f1d1d""##,
        "opacity = 0.85",
    ] {
        let source = radial_tracker_source(&format!(
            r#"
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
            "#
        ));
        let err = load_lua_controller_program_source(&source).unwrap_err();
        assert!(
            err.to_string()
                .contains("radial_cooldown phase style field"),
            "unexpected error for {field}: {err}"
        );
    }
}

fn radial_tracker_source(phases: &str) -> String {
    format!(
        r#"
        sa.state.track({{
          id = "refutation_cooldown",
          capabilities = {{ "screen_read" }},
          poll_ms = 50,
          detector = {{
            kind = "radial_cooldown",
            roi = {{ x = 0, y = 0, w = 36, h = 36 }},
            mask = {{ shape = "circle", inset = 10 }},
            {phases}
          }},
        }})
        "#
    )
}

fn overlay_source(provider: &str) -> String {
    overlay_source_with_provider_and_visuals(provider, overlay_visuals())
}

fn overlay_source_with_visuals(visuals: &str) -> String {
    overlay_source_with_provider_and_visuals("native", visuals.to_string())
}

fn overlay_source_with_extra_overlay_field(field: &str) -> String {
    format!(
        r##"
        poe = {{ processes = {{ "PathOfExileSteam.exe" }} }}
        {}
        sa.overlay.mount({{
          id = "poe2_status",
          scope = poe,
          provider = "native",
          surface = "overlay",
          {field},
          visuals = {{
            {}
          }},
        }})
        "##,
        overlay_trackers(),
        overlay_visuals()
    )
}

fn overlay_source_with_provider_and_visuals(provider: &str, visuals: String) -> String {
    format!(
        r##"
        poe = {{ processes = {{ "PathOfExileSteam.exe" }} }}
        {}
        sa.overlay.mount({{
          id = "poe2_status",
          scope = poe,
          provider = "{provider}",
          surface = "overlay",
          visuals = {{
            {visuals}
          }},
        }})
        "##,
        overlay_trackers()
    )
}

fn overlay_trackers() -> &'static str {
    r#"
        sa.state.track({
          id = "heavy_stun",
          scope = poe,
          capabilities = { "screen_read" },
          poll_ms = 50,
          detector = {
            kind = "horizontal_progress_bar",
            roi = { x = 0, y = 0, w = 10, h = 10 },
            fill = { direction = "left_to_right" },
          },
        })
        sa.state.track({
          id = "refutation_cooldown",
          scope = poe,
          capabilities = { "screen_read" },
          poll_ms = 50,
          detector = {
            kind = "radial_cooldown",
            roi = { x = 0, y = 0, w = 36, h = 36 },
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
    "#
}

fn overlay_visuals() -> String {
    r##"
            {
              id = "heavy_stun",
              kind = "progress_bar",
              bind = { tracker = "heavy_stun", field = "progress_percent" },
              rect = { x = 1640, y = 1590, w = 600, h = 22 },
              opacity = 0.72,
              fill = "#d8b84c",
              background = "#101820",
              label = { visible = true },
              inactive = { opacity = 0.25 },
            },
            {
              id = "refutation",
              kind = "progress_bar",
              bind = { tracker = "refutation_cooldown", field = "remaining_ms" },
              rect = { x = 1640, y = 1620, w = 600, h = 22 },
              opacity = 0.72,
              fill = "#5aa7ff",
              background = "#101820",
              label = { visible = true },
              ready = { fill = "#4ade80", opacity = 0.85 },
              activated = { fill = "#f97316", background = "#7f1d1d", opacity = 0.85 },
              active = { fill = "#38bdf8", background = "#082f49", opacity = 0.8 },
              inactive = { opacity = 0.25 },
            },
    "##
    .to_string()
}

#[test]
fn imperative_lua_accepts_poe2_controller_example_keyword_fields() {
    let source = fs::read_to_string("examples/poe2.lua").unwrap();
    let runtime = ImperativeLuaController::load_source(&source).unwrap();

    assert!(runtime
        .registrations()
        .required_capabilities()
        .contains(CapabilityKind::WindowActivation));
    assert!(runtime.start_callback("reload_filterblade").is_ok());
}

#[test]
fn lua_api_accepts_structured_composite_bindings() {
    let config = load_lua_source(
        r#"
        return {
          bindings = {
            {
              trigger = {
                modifiers = { "Ctrl" },
                mouse = { button = "left" },
              },
              macro = macro {
                key "Alt+Right",
                text "hello world",
                key "Enter",
              },
            },
          },
        }
        "#,
    )
    .unwrap();

    assert_eq!(config.bindings().len(), 1);
    assert_eq!(
        config
            .bindings()
            .values()
            .next()
            .unwrap()
            .trigger
            .describe(),
        "Ctrl+mouse_left"
    );
}

#[test]
fn lua_api_denies_ambient_filesystem() {
    assert!(load_lua_source(r#"return { hotkeys = { }, leak = io.open("/etc/passwd") }"#).is_err());
}

#[test]
fn lua_api_denies_process_shell_environment_and_dynamic_loading() {
    for source in [
        r#"return { hotkeys = {}, leak = os.getenv("HOME") }"#,
        r#"return { hotkeys = {}, leak = os.execute("id") }"#,
        r#"return { hotkeys = {}, leak = require("socket") }"#,
        r#"return { hotkeys = {}, leak = load("return 1") }"#,
        r#"return { hotkeys = {}, leak = debug.traceback() }"#,
    ] {
        assert!(
            load_lua_source(source).is_err(),
            "source should be denied: {source}"
        );
    }
}

#[test]
fn lua_api_denies_compositor_metadata_and_raw_input_apis() {
    for source in [
        r#"return { hotkeys = {}, leak = active_process() }"#,
        r#"return { hotkeys = {}, leak = synthesize_input("x") }"#,
        r#"return { hotkeys = {}, leak = wayland.global_shortcut("F5") }"#,
        r#"return { hotkeys = {}, leak = kde.active_window() }"#,
        r#"return { hotkeys = {}, leak = kwin.activeWindow }"#,
        r#"return { hotkeys = {}, leak = portal.remote_desktop() }"#,
    ] {
        assert!(
            load_lua_source(source).is_err(),
            "source should be denied: {source}"
        );
    }
}

#[test]
fn lua_api_accepts_loop_repeat_motion_syntax() {
    let config = load_lua_source(
        r#"
        return {
          leader = "F13",
          motions = {
            {
              trigger = { "<Leader>", "<LClick>", "<LClick>" },
              within_ms = 500,
              mode = "passthrough",
              loop = {
                while_held = { "<Leader>", "<LClick>" },
                before = macro { key_down "Ctrl" },
                repeat = {
                  every_ms = 65,
                  macro = macro { mouse_click "left" },
                },
                after = macro { key_up "Ctrl" },
              },
            },
          },
        }
        "#,
    )
    .unwrap();

    assert_eq!(config.motions().len(), 1);
    assert!(config
        .motions()
        .values()
        .next()
        .unwrap()
        .loop_definition
        .as_ref()
        .and_then(|loop_definition| loop_definition.repeat())
        .is_some());
}

#[test]
fn lua_api_accepts_held_preconditions_and_guarded_presses() {
    let config = load_lua_source(
        r#"
        return {
          leader = "F13",
          motions = {
            {
              requires_held = { "<Leader>" },
              trigger = { "<LClick>", "<LClick>" },
              within_ms = 500,
              mode = "passthrough",
              loop = {
                while_held = { "<LClick>" },
                repeat = {
                  every_ms = 65,
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
        "#,
    )
    .unwrap();

    assert_eq!(config.motions().len(), 1);
    assert_eq!(config.presses().len(), 1);
}

#[test]
fn lua_api_rejects_non_holdable_preconditions() {
    assert!(
        load_lua_source(
            r#"return { leader = "F13", presses = { { requires_held = { "<WheelUp>" }, trigger = "<WheelDown>", macro = macro { key "Right" } } } }"#
        )
        .is_err()
    );
}

#[test]
fn lua_api_rejects_prefix_overlapping_motion_triggers() {
    let error = load_lua_source(
        r#"
        return {
          leader = "F13",
          motions = {
            {
              trigger = { "<Leader>", "x" },
              macro = macro { text "short" },
            },
            {
              trigger = { "<Leader>", "x", "x" },
              macro = macro { text "long" },
            },
          },
        }
        "#,
    )
    .unwrap_err();

    assert!(error.message.contains("prefix-overlapping motion triggers"));
}

#[test]
fn lua_api_accepts_expanded_keyboard_keys_on_trigger_surfaces() {
    let config = load_lua_source(
        r#"
        return {
          leader = "PageUp",
          hotkeys = {
            ["VolumeUp"] = macro { key "Enter" },
          },
          bindings = {
            {
              trigger = { key = "KPEnter" },
              macro = macro { key "PageDown" },
            },
          },
          motions = {
            {
              trigger = { "<Leader>", "Return" },
              loop = {
                while_held = { "<Leader>", "Return" },
                repeat = {
                  every_ms = 50,
                  macro = macro { key "Mute" },
                },
              },
            },
          },
        }
        "#,
    )
    .unwrap();

    assert_eq!(config.leader.as_ref().unwrap().describe(), "PageUp");
    assert_eq!(config.bindings().len(), 2);
    assert_eq!(config.motions().len(), 1);
    assert_eq!(
        config.motions().keys().next().unwrap().describe(),
        "<Leader> Enter"
    );
}

#[test]
fn lua_api_rejects_alias_equivalent_duplicate_keys() {
    let error = load_lua_source(
        r#"
        return {
          hotkeys = {
            ["Return"] = macro { text "one" },
            ["Enter"] = macro { text "two" },
          },
        }
        "#,
    )
    .unwrap_err();

    assert!(error.message.contains("duplicate binding trigger"));
}

#[test]
fn imperative_lua_filterblade_relay_yields_ordered_host_requests() {
    let runtime = ImperativeLuaController::load_source(
        r#"
        sa.press({
          requires_held = { "Ctrl" },
          trigger = "S",
          mode = "passthrough",
          capabilities = {
            "active_window_metadata",
            "window_activation",
            "synthesized_input",
            "timer",
          },
          callback = "reload_filterblade",
        })

        sa.callback("reload_filterblade", function()
          sa.sleep(100)

          local active = sa.window.active({ title = true })
          local filter = active.title and active.title:match("^FilterBlade%s+%-%s+(.-)%s+%-%s+FilterBlade")
          if filter == nil then
            filter = active.title and active.title:match("^(.-)%s+%-%s+FilterBlade")
          end
          if filter == nil or filter == "" then
            return
          end

          local poe = sa.window.find({
            processes = { "steam_app_2694490", "PathOfExileSteam.exe" },
          })
          if poe == nil then
            return
          end

          if not sa.window.activate(poe) then
            return
          end
          if not sa.window.wait_active(poe, 500) then
            return
          end

          sa.input.key("Enter")
          sa.input.text("/reloaditemfilter " .. filter)
          sa.input.key("Enter")
        end)
        "#,
    )
    .unwrap();
    let capabilities = CapabilitySet::new([
        CapabilityKind::ActiveWindowMetadata,
        CapabilityKind::WindowActivation,
        CapabilityKind::SynthesizedInput,
        CapabilityKind::Timer,
    ]);
    let run = runtime.start_callback("reload_filterblade").unwrap();

    assert_eq!(
        runtime
            .resume_callback(&run, LuaHostResponse::Unit, &capabilities)
            .unwrap(),
        LuaCallbackStep::Yielded(LuaHostRequest::Sleep { duration_ms: 100 })
    );
    assert_eq!(
        runtime
            .resume_callback(&run, LuaHostResponse::Unit, &capabilities)
            .unwrap(),
        LuaCallbackStep::Yielded(LuaHostRequest::ActiveWindow {
            include_title: true
        })
    );
    assert_eq!(
        runtime
            .resume_callback(
                &run,
                LuaHostResponse::ActiveWindow(ActiveWindowMetadata {
                    title: Some(
                        "FilterBlade - v0.5_IuseNixOSBtw - FilterBlade - PoE1&2 Filter Customizer"
                            .to_string()
                    )
                }),
                &capabilities
            )
            .unwrap(),
        LuaCallbackStep::Yielded(LuaHostRequest::FindWindow {
            processes: vec![
                "steam_app_2694490".to_string(),
                "PathOfExileSteam.exe".to_string()
            ]
        })
    );
    assert_eq!(
        runtime
            .resume_callback(
                &run,
                LuaHostResponse::WindowHandle(Some("poe-window-1".to_string())),
                &capabilities
            )
            .unwrap(),
        LuaCallbackStep::Yielded(LuaHostRequest::ActivateWindow {
            handle: "poe-window-1".to_string()
        })
    );
    assert_eq!(
        runtime
            .resume_callback(&run, LuaHostResponse::Bool(true), &capabilities)
            .unwrap(),
        LuaCallbackStep::Yielded(LuaHostRequest::WaitActive {
            handle: "poe-window-1".to_string(),
            timeout_ms: 500
        })
    );
    assert_eq!(
        runtime
            .resume_callback(&run, LuaHostResponse::Bool(true), &capabilities)
            .unwrap(),
        LuaCallbackStep::Yielded(LuaHostRequest::Input {
            action: MacroAction::key("Enter").unwrap()
        })
    );
    assert_eq!(
        runtime
            .resume_callback(&run, LuaHostResponse::Unit, &capabilities)
            .unwrap(),
        LuaCallbackStep::Yielded(LuaHostRequest::Input {
            action: MacroAction::text("/reloaditemfilter v0.5_IuseNixOSBtw").unwrap()
        })
    );
    assert_eq!(
        runtime
            .resume_callback(&run, LuaHostResponse::Unit, &capabilities)
            .unwrap(),
        LuaCallbackStep::Yielded(LuaHostRequest::Input {
            action: MacroAction::key("Enter").unwrap()
        })
    );
    assert_eq!(
        runtime
            .resume_callback(&run, LuaHostResponse::Unit, &capabilities)
            .unwrap(),
        LuaCallbackStep::Complete
    );
}

#[test]
fn imperative_lua_filterblade_relay_emits_nothing_without_matching_title() {
    let runtime = ImperativeLuaController::load_source(
        r#"
        sa.press({
          trigger = "S",
          mode = "passthrough",
          capabilities = { "active_window_metadata", "timer" },
          callback = "reload_filterblade",
        })

        sa.callback("reload_filterblade", function()
          sa.sleep(100)
          local active = sa.window.active({ title = true })
          local filter = active.title and active.title:match("^(.-)%s+%- FilterBlade")
          if filter == nil or filter == "" then
            return
          end
          sa.input.text(filter)
        end)
        "#,
    )
    .unwrap();
    let capabilities =
        CapabilitySet::new([CapabilityKind::ActiveWindowMetadata, CapabilityKind::Timer]);
    let run = runtime.start_callback("reload_filterblade").unwrap();

    assert!(matches!(
        runtime
            .resume_callback(&run, LuaHostResponse::Unit, &capabilities)
            .unwrap(),
        LuaCallbackStep::Yielded(LuaHostRequest::Sleep { .. })
    ));
    assert!(matches!(
        runtime
            .resume_callback(&run, LuaHostResponse::Unit, &capabilities)
            .unwrap(),
        LuaCallbackStep::Yielded(LuaHostRequest::ActiveWindow { .. })
    ));
    assert_eq!(
        runtime
            .resume_callback(
                &run,
                LuaHostResponse::ActiveWindow(ActiveWindowMetadata {
                    title: Some("Downloads".to_string())
                }),
                &capabilities
            )
            .unwrap(),
        LuaCallbackStep::Complete
    );
}

#[test]
fn imperative_lua_logs_without_sensitive_capability() {
    let runtime = ImperativeLuaController::load_source(
        r#"
        sa.press({
          trigger = "S",
          mode = "passthrough",
          capabilities = {},
          callback = "probe",
        })

        sa.callback("probe", function()
          sa.log("checking filterblade")
        end)
        "#,
    )
    .unwrap();
    let capabilities = CapabilitySet::new([]);
    let run = runtime.start_callback("probe").unwrap();

    assert_eq!(
        runtime
            .resume_callback(&run, LuaHostResponse::Unit, &capabilities)
            .unwrap(),
        LuaCallbackStep::Yielded(LuaHostRequest::Log {
            level: LuaLogLevel::Info,
            message: "checking filterblade".to_string()
        })
    );
    assert_eq!(
        runtime
            .resume_callback(&run, LuaHostResponse::Unit, &capabilities)
            .unwrap(),
        LuaCallbackStep::Complete
    );
}

#[test]
fn imperative_lua_preempts_non_yielding_callback_before_first_host_request() {
    let runtime = ImperativeLuaController::load_source(
        r#"
        sa.press({
          trigger = "S",
          capabilities = { "synthesized_input" },
          callback = "spin",
        })

        sa.callback("spin", function()
          while true do
          end
          sa.input.text("after")
        end)
        "#,
    )
    .unwrap();
    let run = runtime.start_callback("spin").unwrap();
    let budget = LuaExecutionBudget::new(Duration::from_millis(1), 100).unwrap();

    assert_eq!(
        runtime
            .resume_callback_with_budget(
                &run,
                LuaHostResponse::Unit,
                &CapabilitySet::new([CapabilityKind::SynthesizedInput]),
                budget
            )
            .unwrap(),
        LuaCallbackStep::Preempted
    );
}

#[test]
fn imperative_lua_preempts_non_yielding_callback_after_sleep_resume() {
    let runtime = ImperativeLuaController::load_source(
        r#"
        sa.press({
          trigger = "S",
          capabilities = { "timer", "synthesized_input" },
          callback = "spin",
        })

        sa.callback("spin", function()
          sa.sleep(10)
          while true do
          end
          sa.input.text("after")
        end)
        "#,
    )
    .unwrap();
    let run = runtime.start_callback("spin").unwrap();
    let capabilities =
        CapabilitySet::new([CapabilityKind::Timer, CapabilityKind::SynthesizedInput]);
    let budget = LuaExecutionBudget::new(Duration::from_millis(1), 100).unwrap();

    assert_eq!(
        runtime
            .resume_callback_with_budget(&run, LuaHostResponse::Unit, &capabilities, budget)
            .unwrap(),
        LuaCallbackStep::Yielded(LuaHostRequest::Sleep { duration_ms: 10 })
    );
    assert_eq!(
        runtime
            .resume_callback_with_budget(&run, LuaHostResponse::Unit, &capabilities, budget)
            .unwrap(),
        LuaCallbackStep::Preempted
    );
}

#[test]
fn imperative_lua_preserves_bounded_work_and_sleep_with_budget() {
    let runtime = ImperativeLuaController::load_source(
        r#"
        sa.press({
          trigger = "S",
          capabilities = { "timer", "synthesized_input" },
          callback = "bounded",
        })

        sa.callback("bounded", function()
          local sum = 0
          for i = 1, 100 do
            sum = sum + i
          end
          sa.sleep(5)
          sa.input.text(tostring(sum))
        end)
        "#,
    )
    .unwrap();
    let run = runtime.start_callback("bounded").unwrap();
    let capabilities =
        CapabilitySet::new([CapabilityKind::Timer, CapabilityKind::SynthesizedInput]);
    let budget = LuaExecutionBudget::new(Duration::from_millis(50), 100).unwrap();

    assert_eq!(
        runtime
            .resume_callback_with_budget(&run, LuaHostResponse::Unit, &capabilities, budget)
            .unwrap(),
        LuaCallbackStep::Yielded(LuaHostRequest::Sleep { duration_ms: 5 })
    );
    assert_eq!(
        runtime
            .resume_callback_with_budget(&run, LuaHostResponse::Unit, &capabilities, budget)
            .unwrap(),
        LuaCallbackStep::Yielded(LuaHostRequest::Input {
            action: MacroAction::text("5050").unwrap()
        })
    );
    assert_eq!(
        runtime
            .resume_callback_with_budget(&run, LuaHostResponse::Unit, &capabilities, budget)
            .unwrap(),
        LuaCallbackStep::Complete
    );
}

#[test]
fn imperative_lua_denies_host_request_without_declared_capability() {
    let runtime = ImperativeLuaController::load_source(
        r#"
        sa.press({
          trigger = "S",
          capabilities = { "timer" },
          callback = "probe",
        })

        sa.callback("probe", function()
          sa.window.active({ title = true })
        end)
        "#,
    )
    .unwrap();
    let run = runtime.start_callback("probe").unwrap();
    let error = runtime
        .resume_callback(
            &run,
            LuaHostResponse::Unit,
            &CapabilitySet::new([CapabilityKind::Timer]),
        )
        .unwrap_err();

    assert_eq!(
        error.capability,
        Some(signal_auras_core::Capability::ActiveWindowMetadata)
    );
    assert!(error.message.contains("active_window_metadata"));
}

#[test]
fn imperative_lua_denies_ambient_runtime_apis() {
    for source in [
        r#"os.getenv("HOME")"#,
        r#"io.open("/etc/passwd")"#,
        r#"require("socket")"#,
        r#"load("return 1")"#,
        r#"debug.traceback()"#,
    ] {
        assert!(
            ImperativeLuaController::load_source(source).is_err(),
            "source should be denied: {source}"
        );
    }
}

#[test]
fn imperative_lua_loads_resolved_imported_callbacks_with_runtime_import_noop() {
    let root = temp_lua_dir("runtime-source-tree");
    fs::write(
        root.join("helper.lua"),
        r#"
        sa.callback("imported_sleep", function()
          sa.sleep(25)
        end)
        "#,
    )
    .unwrap();
    let main = root.join("main.lua");
    fs::write(
        &main,
        r#"
        sa.import("helper")
        sa.hotkey({
          trigger = "F5",
          capabilities = { "global_shortcut", "timer" },
          callback = "imported_sleep",
        })
        "#,
    )
    .unwrap();

    let source = load_lua_controller_runtime_source_file(&main).unwrap();
    assert!(source.contains(r#"sa.callback("imported_sleep""#));
    assert!(source.contains(r#"sa.import("helper")"#));

    let runtime = ImperativeLuaController::load_source(&source).unwrap();
    let run = runtime.start_callback("imported_sleep").unwrap();

    assert_eq!(
        runtime
            .resume_callback(
                &run,
                LuaHostResponse::Unit,
                &CapabilitySet::new([CapabilityKind::Timer])
            )
            .unwrap(),
        LuaCallbackStep::Yielded(LuaHostRequest::Sleep { duration_ms: 25 })
    );
}

#[test]
fn imperative_lua_requires_registered_callbacks_to_be_defined() {
    let error = match ImperativeLuaController::load_source(
        r#"
        sa.press({
          trigger = "S",
          capabilities = { "timer" },
          callback = "missing",
        })
        "#,
    ) {
        Ok(_) => panic!("missing callback should fail validation"),
        Err(error) => error,
    };

    assert!(error.message.contains("registered but not defined"));
}

fn temp_lua_dir(label: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_DIR_ID: AtomicU64 = AtomicU64::new(0);
    let mut path = std::env::temp_dir();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = NEXT_DIR_ID.fetch_add(1, Ordering::SeqCst);
    path.push(format!(
        "signal-auras-lua-api-{label}-{}-{unique}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&path).unwrap();
    path
}
