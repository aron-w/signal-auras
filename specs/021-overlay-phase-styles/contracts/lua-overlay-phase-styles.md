# Contract: Lua Overlay Phase Styles

## State Tracker Rule

Detector phase rules are recognition-only.

```lua
activated = {
  sample = { kind = "clock_probe", angle_deg = 8, radius_px = 15, w = 3, h = 3 },
  max_luminance_percent = 12,
  max_saturation = 20,
  progress_fill = "empty",
}
```

The following fields are not accepted inside radial detector phase rules:

- `fill`
- `background`
- `opacity`

## Overlay Visual Rule

Radial cooldown progress bars may use phase-specific visual styles:

```lua
{
  id = "refutation",
  kind = "progress_bar",
  bind = { tracker = "refutation_cooldown", field = "remaining_ms" },
  rect = { x = 1200, y = 930, w = 150, h = 22 },
  opacity = 0.72,
  fill = "#5aa7ff",
  background = "#101820",
  label = { visible = true },
  ready = { fill = "#4ade80", opacity = 0.85 },
  activated = { fill = "#f97316", background = "#7f1d1d", opacity = 0.85 },
  active = { fill = "#38bdf8", background = "#082f49", opacity = 0.8 },
  inactive = { opacity = 0.25 },
}
```

`ready`, `activated`, `active`, and `inactive` use the same style shape:

- `fill`: optional `#RRGGBB`
- `background`: optional `#RRGGBB`
- `opacity`: optional number from `0.0` through `1.0`
- `label_visible` or `visible`: optional boolean

## Validation

- A detector phase style field fails script validation.
- Invalid overlay phase style values fail with the existing overlay style diagnostics.
- `activated` and `active` phase styles only affect progress-bar visuals bound to a radial cooldown tracker's `remaining_ms` field.
- If a phase style is absent, existing default overlay behavior remains compatible.

## Security Boundary

This contract adds no new Lua capability. Lua still receives no raw screen buffers, input streams, compositor handles, permission handles, filesystem access, network access, or macro authority from tracker or overlay declarations.
