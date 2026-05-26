# Signal Auras

Signal Auras is a Wayland-first automation runner for NixOS. The v1 runner
loads one Lua file from a terminal command, validates scoped hotkeys, registers
them through the current Wayland adapter boundary, and prints runtime stats when
the run stops.

## Current Status

The repository currently contains the first Lua hotkey runner implementation and
a KDE Plasma Wayland adapter boundary. The runner detects KDE Wayland session
requirements, maps missing KWin/KGlobalAccel/portal support to diagnosable
capability failures by probing the live user D-Bus, tracks current-run shortcut
handles, converts KDE/KWin active-window snapshots into conservative
process-matching contexts, and gates synthesized input through a portal-oriented
validation/session boundary.

The KDE provider now has current-run KWin/KGlobalAccel callback wiring and KDE
RemoteDesktop portal emission. Motions can opt into an unsafe evdev backend for
explicitly listed `/dev/input/event*` devices; this is for high-trust local use,
requires device permissions, and supports observe or grab mode. Generated input
can use the KDE portal or `/dev/uinput`. T062 remains open until physical
desktop keypress delivery and the denied-portal zero-emission path are manually
verified. Unsupported sessions fail closed instead of falling back to hidden
behavior.

## Usage

Run from the project development shell:

```bash
nix develop -c cargo run -p signal-auras-cli -- run ./examples/poe2-hideout.lua
```

The command shape is:

```text
signal-auras run <lua-file>
```

The runner accepts exactly one Lua file path. Startup output includes the script
path, validation result, effective scope, capability probe result, and hotkey
registration result. Press Ctrl-C to stop a successful run and print final
runtime stats.

## Consent Model

Signal Auras does not install background services, autostart entries, persistent
state, or hidden global hotkeys for v1. Every run is terminal-started and
current-run only.

Lua scripts may declare a process scope:

```lua
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
```

If a script omits `scope`, the runner prompts before registration:

```text
No scope declared by script.
Select scope for this run:
1. Process names
2. Global hotkeys for this run
3. Cancel
```

Process selection applies only to the current process. Global selection requires
an explicit `GLOBAL` confirmation and is also current-run only. Cancel exits
without registering hotkeys.

## Lua API

The Lua surface is intentionally small:

- `macro { ... }` creates one ordered macro.
- `key "<key-name>"` sends a key action.
- `text "<string>"` sends text input.
- `mouse_click "<left|right|middle>"` sends a mouse button click action.
- `delay <milliseconds>` waits before the next action.
- `hotkeys = { ["F5"] = macro { ... } }` keeps the legacy keyboard binding shape.
- `bindings = { ... }` accepts structured triggers with modifiers, mouse buttons, mouse wheel directions, and an explicit mode.
- `motions = { ... }` accepts uniform sequence notation for leader, keyboard, and mouse tokens.

Structured composite bindings use one primary trigger and optional modifiers:

```lua
return {
  bindings = {
    {
      trigger = {
        modifiers = { "Ctrl" },
        mouse = { wheel = "up" },
      },
      macro = macro {
        key "Left",
      },
    },
    {
      trigger = {
        modifiers = { "Ctrl" },
        mouse = { button = "left" },
      },
      mode = "passthrough",
      macro = macro {
        key "Alt+Right",
        text "hello world",
        key "Enter",
      },
    },
  },
}
```

Supported modifiers are `Ctrl`, `Alt`, `Shift`, and `Super`. Supported mouse
buttons are `left`, `right`, and `middle`; supported wheel directions are `up`
and `down`. Missing `mode` defaults to `consume`; `passthrough` leaves the
original input event available to the target application. Consumed pointer
bindings fail before activation when the Wayland provider cannot guarantee
suppression of the original click or wheel event.

Lua scripts do not receive ambient filesystem, network, process, shell,
environment, compositor, active-process, global-input, or synthesized-input
access. Unsupported or malformed scripts are rejected before registration.
Synthesized keyboard and pointer output is requested through the KDE
RemoteDesktop portal and requires user-granted portal permission for the
current run. Global input observation for motions requires either the explicit
unsafe evdev backend below or a future KDE/KWin-side provider; normal Wayland
clients are not granted ambient event capture.

Motions model multi-input sequences as one logical unit:

```lua
return {
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

For local high-trust testing on KDE Wayland, a script may explicitly name evdev
devices. `mode = "observe"` reads events without suppressing the original input;
use `mode = "grab"` when this process should ask evdev for exclusive delivery.
`output = "portal"` keeps generated input behind KDE portal permission;
`output = "uinput"` writes generated input through `/dev/uinput`. To listen to
all current evdev devices, set `devices = "all"`. Combining `devices = "all"`
with grab mode also requires `acknowledge_risk = "GRAB_ALL_INPUTS"`.

```lua
return {
  input_provider = {
    backend = "evdev",
    mode = "grab",
    output = "uinput",
    devices = "all",
    acknowledge_risk = "GRAB_ALL_INPUTS",
  },

  leader = "F13",
  motions = {
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

`defaults.inter_action_delay_ms` applies between generated macro actions;
`motion.inter_action_delay_ms` overrides it for one motion. Explicit
`delay(ms)` actions remain part of macros. Delays are valid from zero for
inter-action defaults and one millisecond for explicit `delay` actions.

## Examples

- `examples/poe2-hideout.lua`: process-scoped `F5` macro for `/hideout`.
- `examples/prompt-scope.lua`: scope-free script that exercises the terminal
  consent prompt.
- `examples/composite-bindings.lua`: structured `Ctrl` plus wheel and left-click
  bindings.
- `examples/input-motions.lua`: uniform leader, keyboard, mouse, and repeat
  motion notation.

## Verification

Automated checks:

```bash
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test
```

The repository also includes a `justfile` with a shell-rendered command guide:

```bash
just
just check
just run
just failures
```

Manual KDE Plasma Wayland verification is documented in
`tests/compositor/manual-wayland-verification.md`. Completion requires a real
KDE session proving desktop-wide shortcut registration/event delivery,
active-process scoped match and non-match behavior, synthesized input success,
denial behavior, and shutdown cleanup.
