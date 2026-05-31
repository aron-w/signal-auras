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

- `requires_held`: optional holdable-token precondition
- `trigger`: required `MotionTrigger`
- `within_ms`: positive trigger completion window, default `500`
- `mode`: `consume` or `passthrough`, default `consume`
- `macro`: optional macro emitted once when the trigger completes
- `loop`: optional held loop behavior
- `inter_action_delay_ms`: resolved delay from global defaults or a motion override

At least one of `macro` or `loop` is required.

`requires_held` tokens are not part of the sequence. They must be held before
the first trigger press and remain held until the attempt completes. Wheel
tokens are invalid here because they do not have release events.

## PressDefinition

One immediate guarded action:

- `requires_held`: optional holdable-token precondition
- `trigger`: required single `MotionToken`
- `mode`: `consume` or `passthrough`, default `consume`
- `macro`: required macro emitted on the trigger press
- `inter_action_delay_ms`: resolved delay from global defaults or a press override

Presses do not create active motion attempts, do not use `within_ms`, and do
not own loops.

## LoopDefinition

Held-loop behavior owned by the motion:

- `while_held`: required `MotionTrigger` describing inputs that must remain held
- `before`: optional macro emitted once when the loop starts
- `body`: exactly one of `once` macro or fixed `repeat` body
- `repeat.every_ms`: positive interval in milliseconds
- `repeat.macro`: required macro emitted on repeat ticks
- `after`: optional macro emitted once when the loop ends or is cancelled

## MacroAction

Existing actions remain:

- key press
- key press-only
- key release-only
- text input
- explicit delay

New action:

- mouse click with `left`, `right`, or `middle`
