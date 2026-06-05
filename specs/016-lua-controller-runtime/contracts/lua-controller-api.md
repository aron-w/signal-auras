# Contract: Lua Controller API

## Loader

`load_lua_controller_file(path)` MUST:

- Treat `path.parent()` as the script root.
- Read the main script and any `sa.import("module")` local modules.
- Resolve modules under the script root and add `.lua` when no extension is present.
- Reject absolute imports, parent traversal, denied ambient APIs, and imports outside the root.
- Return validated controller registrations without activating input capture, output emission, screen read, compositor calls, timers, or cleanup hooks.

`load_lua_controller_source(source)` MUST parse a single in-memory controller source and apply the same ambient API denial rules, except filesystem import resolution is unavailable.

## Registration Surface

The controller loader recognizes startup registrations:

```lua
sa.hotkey({
  trigger = "F5",
  scope = { processes = { "poe2.exe" } },
  mode = "consume",
  callback = "hideout",
})

sa.motion({
  trigger = "<Leader> x",
  mode = "passthrough",
  callback = "motion",
})

sa.press({ trigger = "<WheelUp>", callback = "wheel" })
sa.timer({ trigger = "heartbeat", callback = "tick" })
sa.shutdown({ callback = "cleanup" })
```

Runtime activation uses `sa.callback(name, function() ... end)` definitions to
bind registered callback names to Rust-backed host requests:

```lua
sa.callback("hideout", function()
  sa.input.key("Enter")
  sa.input.text("/hideout")
  sa.input.key("Enter")
end)
```

Supported callback APIs are:

- `sa.sleep(ms)`: Yield to a Rust timer continuation.
- `sa.log(message)`, `sa.log_debug(message)`, and `sa.log_warn(message)`: Emit explicit script diagnostics.
- `sa.window.active({ title = true })`: Return fresh active-window metadata when the callback has active-window metadata capability.
- `sa.window.find({ processes = {...} })`: Return an opaque window handle for a matching process when the callback has window activation capability.
- `sa.window.activate(handle)`: Request compositor activation of an opaque window handle.
- `sa.window.wait_active(handle, timeout_ms)`: Verify fresh focus before output.
- `sa.input.key`, `sa.input.key_down`, `sa.input.key_up`, `sa.input.text`, and `sa.input.mouse_click`: Queue ordered Rust synthesized-input requests.

These APIs yield to Rust and do not receive direct OS handles. Sensitive host
requests are capability-gated per callback registration. Diagnostics must avoid
ambient title/process disclosure unless the script explicitly logs data it has
already requested through a declared capability.

## Compatibility

Existing declarative scripts loaded through `load_lua_source` and `load_lua_file` MUST remain unchanged. The controller loader is additive and separate.

## Denied Ambient APIs

The controller surface MUST deny `io`, `os`, `package`, `require`, `debug`, `dofile`, `loadfile`, dynamic `load`, socket APIs, and unrestricted Lua package loading. Local multi-file controllers MUST use `sa.import`.

Controller startup validation MUST execute registration/import source in a restricted
`mlua` environment that installs the same denied globals as the imperative runtime.
Denied API names inside harmless local variables or strings MUST NOT fail controller
startup validation. Callback bodies MUST NOT be executed during load; loader-side
callback validation therefore keeps a bounded denied-token fallback for ambient API
references, while runtime execution still denies the same globals through the
structured Lua environment.
