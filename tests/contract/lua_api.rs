use signal_auras_lua::load_lua_source;

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
fn lua_api_accepts_existing_repeat_motion_syntax_without_policy_migration() {
    let config = load_lua_source(
        r#"
        return {
          leader = "F13",
          motions = {
            {
              trigger = { "<Leader>", "<LClick>", "<LClick>" },
              mode = "passthrough",
              repeat = {
                while_held = { "<Leader>", "<LClick>" },
                interval_ms = { min = 50, max = 80 },
                macro = macro { mouse_click "left" },
              },
            },
          },
        }
        "#,
    )
    .unwrap();

    assert_eq!(config.motions().len(), 1);
    assert!(config.motions().values().next().unwrap().repeat.is_some());
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
              repeat = {
                while_held = { "<Leader>", "Return" },
                interval_ms = { min = 50, max = 80 },
                macro = macro { key "Mute" },
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
