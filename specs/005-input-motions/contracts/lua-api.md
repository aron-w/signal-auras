# Lua API Contract: Unified Input Motions

Existing `hotkeys` and `bindings` remain supported. New scripts may define `motions`:

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
      trigger = { "<Leader>", "<LClick>", "<LClick>" },
      mode = "passthrough",
      repeat = {
        while_held = { "<Leader>", "<LClick>" },
        interval_ms = { min = 50, max = 80 },
        macro = macro {
          mouse_click "left",
        },
      },
    },
  },
}
```

## Motion Fields

- `trigger`: required non-empty token list.
- `mode`: optional `consume` or `passthrough`; missing means `consume`.
- `macro`: optional one-shot macro.
- `repeat`: optional repeat behavior.
- `inter_action_delay_ms`: optional non-negative override for generated actions.

Each motion must define `macro` or `repeat`.

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

## Repeat Fields

- `while_held`: required token list; repeat stops when any token is released.
- `interval_ms.min`: positive lower bound.
- `interval_ms.max`: positive upper bound greater than or equal to `min`.
- `macro`: required macro emitted on repeat ticks.

## Token Set

Supported tokens include `<Leader>`, printable keys, `F1` through `F24`, `<LClick>`, `<RClick>`, and `<MClick>`.

## Delay Semantics

- `defaults.inter_action_delay_ms` defaults to `0`.
- `motion.inter_action_delay_ms` overrides the global default.
- Explicit `delay(ms)` actions remain supported in macros.
- Inter-action delays are valid from `0` upward.
- Explicit `delay(ms)` actions are valid from `1` upward.
