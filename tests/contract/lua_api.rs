use signal_auras_lua::{load_lua_controller_program_file, load_lua_file, load_lua_source};
use std::path::Path;

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

    assert_eq!(program.registrations().registrations().len(), 6);
    assert!(program.input_provider.is_some());
    assert!(program.leader.is_some());
    assert!(!program
        .required_capabilities()
        .contains(signal_auras_core::CapabilityKind::GlobalShortcut));
    assert!(program
        .required_capabilities()
        .contains(signal_auras_core::CapabilityKind::CompositePointerObservation));
    assert!(program.callback("go_home").is_some());
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
