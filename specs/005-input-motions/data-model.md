# Data Model: Unified Input Motions

## AutomationDefaults

- `inter_action_delay_ms`: delay between generated macro actions, default `0`.

## MotionToken

One normalized token in a motion sequence:

- `<Leader>`
- printable key
- function key `F1` through `F24`
- `<LClick>`, `<RClick>`, or `<MClick>`
- `<WheelUp>` or `<WheelDown>`

## MotionTrigger

Ordered non-empty list of `MotionToken` values. Duplicate motion triggers are invalid.

## MotionDefinition

One logical input motion:

- `trigger`: required `MotionTrigger`
- `mode`: `consume` or `passthrough`, default `consume`
- `macro`: optional macro emitted once when the trigger completes
- `repeat`: optional repeat behavior
- `inter_action_delay_ms`: resolved delay from global defaults or a motion override

At least one of `macro` or `repeat` is required.

## RepeatDefinition

Repeat-specific behavior owned by the motion:

- `while_held`: required `MotionTrigger` describing inputs that must remain held
- `interval`: positive min/max interval in milliseconds with `min <= max`
- `macro`: required macro emitted on repeat ticks

## MacroAction

Existing actions remain:

- key press
- text input
- explicit delay

New action:

- mouse click with `left`, `right`, or `middle`
