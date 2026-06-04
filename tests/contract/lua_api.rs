use signal_auras_core::{CapabilityKind, CapabilitySet, DetectorDefinition, MacroAction};
use signal_auras_lua::{
    load_lua_controller_program_file, load_lua_controller_program_source, load_lua_file,
    load_lua_source, ActiveWindowMetadata, ImperativeLuaController, LuaCallbackStep,
    LuaHostRequest, LuaHostResponse, LuaLogLevel,
};
use std::{fs, path::Path};

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
    assert_eq!(loop_motion.repeat_every_ms, 65);
    assert_eq!(loop_motion.repeat_callback, "click_left");
    assert!(program.callback("ctrl_click").is_some());
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
        refutation.detector,
        DetectorDefinition::RadialCooldown { .. }
    ));

    let heavy_stun = trackers
        .iter()
        .find(|tracker| tracker.id == "heavy_stun")
        .unwrap();
    assert_eq!(heavy_stun.poll_ms, 50);
    assert!(matches!(
        heavy_stun.detector,
        DetectorDefinition::HorizontalProgressBar { .. }
    ));

    assert!(program.callback("refutation_cooldown").is_none());
    assert!(program.callback("heavy_stun").is_none());
    assert!(program
        .state_trackers()
        .required_capabilities()
        .contains(CapabilityKind::ScreenRead));
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
