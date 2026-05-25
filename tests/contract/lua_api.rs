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
