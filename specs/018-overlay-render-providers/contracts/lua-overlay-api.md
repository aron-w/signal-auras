# Contract: Lua Overlay API

## Registration

Overlay declarations are startup registrations. Loading a script may validate overlay definitions, but it must not create surfaces, read screen pixels, consume input, synthesize input, or execute macros.

```lua
sa.overlay.mount({
  id = "poe2_status",
  scope = poe2_scope,
  provider = "native",
  surface = "overlay",
  visuals = {
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
      inactive = { opacity = 0.25 },
    },
  },
})
```

## Accepted Fields

### Overlay

- `id`: required non-empty string.
- `scope`: optional scope table or variable matching existing controller scope behavior.
- `provider`: required string. Accepted values: `native`, `webview`, `tauri_window`, `tool_window`.
- `surface`: optional string. Accepted v1 value: `overlay`.
- `visuals`: required array-like table with at least one visual.

### Progress Bar Visual

- `id`: required non-empty string, unique within the overlay.
- `kind`: required string, `progress_bar`.
- `bind`: required table with `tracker` and `field` strings.
- `rect`: required table with `x`, `y`, `w`, and `h`.
- `opacity`: optional number from 0.0 through 1.0; defaults to 1.0.
- `fill`: required color string.
- `background`: required color string.
- `label`: optional table with `visible` boolean.
- `ready`: optional override style.
- `inactive`: optional override style.

## Validation Errors

Script validation fails for:
- Missing overlay id.
- Duplicate overlay id.
- Unknown provider id.
- Missing or empty visuals.
- Duplicate visual id within an overlay.
- Unsupported visual kind.
- Missing binding table, tracker id, or field name.
- Binding to a missing tracker id.
- Binding to a field not emitted by the tracker kind.
- Invalid rectangle coordinates or size.
- Invalid opacity.
- Malformed style/color fields.

## Security Contract

Lua overlay declarations cannot access:
- Raw screen buffers.
- Input devices or input streams.
- Compositor handles or portal handles.
- Permission handles.
- Ambient filesystem, shell, debug, package loading, or network APIs.
- Hotkey, macro, screen capture, focus, or capability decisions beyond declarative registration.
