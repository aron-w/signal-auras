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
bind registered callback names to Rust-backed output requests:

```lua
sa.callback("hideout", function()
  sa.input.key("Enter")
  sa.input.text("/hideout")
  sa.input.key("Enter")
end)
```

Supported callback output APIs are `sa.input.key`, `sa.input.key_down`,
`sa.input.key_up`, `sa.input.text`, and `sa.input.mouse_click`. These APIs queue
Rust operation requests; Lua does not receive direct OS handles.

## Compatibility

Existing declarative scripts loaded through `load_lua_source` and `load_lua_file` MUST remain unchanged. The controller loader is additive and separate.

## Denied Ambient APIs

The controller surface MUST deny `io`, `os`, `package`, `require`, `debug`, `dofile`, `loadfile`, dynamic `load`, socket APIs, and unrestricted Lua package loading. Local multi-file controllers MUST use `sa.import`.
