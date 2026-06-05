# Contract: Lua State API

## Registration Shape

```lua
sa.state.track({
  id = "refutation_cooldown",
  scope = poe,
  capabilities = { "screen_read" },
  poll_ms = 50,
  detector = {
    kind = "radial_cooldown",
    roi = { x = 2850, y = 2030, w = 96, h = 92 },
    mask = { shape = "circle", inset = 10 },
  },
})
```

```lua
sa.state.track({
  id = "heavy_stun",
  scope = poe,
  capabilities = { "screen_read" },
  poll_ms = 50,
  detector = {
    kind = "horizontal_progress_bar",
    roi = { x = 1828, y = 702, w = 190, h = 58 },
    fill = { direction = "left_to_right" },
  },
})
```

## Rules

- `emits`, fixture paths, callbacks, macros, and input actions are not accepted in tracker definitions.
- Radial cooldown `phases` are recognition rules only: they accept sample geometry, luminance/saturation thresholds, metrics, and progress-estimation fields.
- Presentation fields such as `fill`, `background`, and `opacity` are not accepted inside radial cooldown phase rules; phase-specific colors belong in `sa.overlay.mount(...)`.
- Registration validates definitions only; it does not start screen capture.
- Lua does not receive screenshots, raw pixel buffers, portal handles, compositor handles, or filesystem access.
- `screen_read` is the only required capability for these trackers; process-scoped focus metadata may be required by runtime scope evaluation.
