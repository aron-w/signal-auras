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

The KDE provider now has current-run KWin/KGlobalAccel callback wiring and a KDE
RemoteDesktop portal emission path. T062 remains open until physical desktop
keypress delivery and the denied-portal zero-emission path are manually verified.
Unsupported sessions fail closed instead of falling back to hidden behavior.

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

The v1 Lua surface is intentionally small:

- `macro { ... }` creates one ordered macro.
- `key "<key-name>"` sends a key action.
- `text "<string>"` sends text input.
- `delay <milliseconds>` waits before the next action.

Lua scripts do not receive ambient filesystem, network, process, shell,
environment, compositor, active-process, global-input, or synthesized-input
access. Unsupported or malformed scripts are rejected before registration.

## Examples

- `examples/poe2-hideout.lua`: process-scoped `F5` macro for `/hideout`.
- `examples/prompt-scope.lua`: scope-free script that exercises the terminal
  consent prompt.

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
