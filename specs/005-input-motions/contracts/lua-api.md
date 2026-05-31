# Lua API Contract: Unified Input Motions

Existing `hotkeys` and `bindings` remain supported. New scripts may define
`motions` and immediate guarded `presses`:

```lua
return {
  input_provider = {
    backend = "evdev",
    mode = "observe",
    output = "portal",
    devices = { "/dev/input/event3" },
  },

  leader = "F13",

  defaults = {
    inter_action_delay_ms = 0,
  },

  motions = {
    {
      trigger = { "<Leader>", "f", "f" },
      mode = "consume",
      macro = macro {
        text "/search",
      },
    },
    {
      requires_held = { "<Leader>" },
      trigger = { "<LClick>", "<LClick>" },
      mode = "passthrough",
      within_ms = 500,
      loop = {
        while_held = { "<LClick>" },
        before = macro {
          key_down "Ctrl",
        },
        repeat = {
          every_ms = 65,
          macro = macro {
            mouse_click "left",
          },
        },
        after = macro {
          key_up "Ctrl",
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
```

## Motion Fields

- `trigger`: required non-empty token list.
- `requires_held`: optional list of holdable tokens that must already be held.
- `within_ms`: optional positive trigger completion window; missing means `500`.
- `mode`: optional `consume` or `passthrough`; missing means `consume`.
- `macro`: optional one-shot macro.
- `loop`: optional held loop behavior.
- `inter_action_delay_ms`: optional non-negative override for generated actions.

Each motion must define `macro` or `loop`.

For guarded motions, all `requires_held` tokens must be held before the first
trigger press and must remain held until the motion completes. Releasing a
required token discards the active attempt; if the motion already started a
loop, the release cancels the loop and runs `after` cleanup.

## Press Fields

- `trigger`: required single token string.
- `requires_held`: optional list of holdable tokens that must currently be held.
- `mode`: optional `consume` or `passthrough`; missing means `consume`.
- `macro`: required macro emitted immediately when the trigger press arrives.
- `inter_action_delay_ms`: optional non-negative override for generated actions.

Presses do not use `within_ms`, sequence matching, or loop scheduling. A guarded
press whose guard is not currently satisfied emits no macro and records no
trigger stats.

## Input Provider Fields

`input_provider` is optional. When omitted, real motions fail closed unless a
safe provider is available.

- `backend`: required; currently only `evdev`.
- `mode`: optional; `observe`, `grab`, or `consume` aliasing `grab`.
- `output`: optional; `portal` or `uinput`.
- `devices`: required non-empty list of explicit evdev device paths.

`observe` mode does not consume physical input events, so scripts using observe
should use `mode = "passthrough"` for motions. `grab`/`consume` asks evdev for
exclusive delivery and may support consumed motions when device permissions and
kernel policy allow it.

## Loop Fields

- `while_held`: required token list; loop stops when any token is released.
- `before`: optional macro emitted once when the loop starts.
- `once`: optional one-shot loop body macro.
- `repeat.every_ms`: positive fixed repeat interval.
- `repeat.macro`: required macro emitted on repeat ticks.
- `after`: optional macro emitted once when the loop ends or is cancelled.

Exactly one loop body is accepted: `once` or `repeat`. `loop.next = function(...)`
is reserved for a later callback design and is rejected for now. The removed
`motions[].repeat.interval_ms.{min,max}` shape migrates to
`motions[].loop.repeat.every_ms`.

## Token Set

Supported trigger tokens include `<Leader>`, printable keys, `F1` through
`F24`, `<LClick>`, `<RClick>`, `<MClick>`, `<WheelUp>`, and `<WheelDown>`.
`requires_held` accepts only holdable tokens: `<Leader>`, keyboard keys, and
mouse buttons. Wheel tokens are rejected because wheel input has no held state.

## Delay Semantics

- `defaults.inter_action_delay_ms` defaults to `0`.
- `motion.inter_action_delay_ms` overrides the global default.
- Explicit `delay(ms)` actions remain supported in macros.
- Inter-action delays are valid from `0` upward.
- Explicit `delay(ms)` actions are valid from `1` upward.
