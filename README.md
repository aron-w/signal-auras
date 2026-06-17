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
nix develop -c cargo run -p signal-auras-cli -- run ./examples/poe2.lua
```

The command shape is:

```text
signal-auras run [--verbose|-v] [--reset-input-cache] <lua-file>
```

The runner accepts exactly one Lua file path. Startup output includes the script
path, validation result, effective scope, capability probe result, and hotkey
registration result. Press Ctrl-C to stop a successful run and print final
runtime stats. Use `--verbose` while debugging provider setup and motion input;
verbose logs use `level=... event=... key=value` fields. Motion diagnostics
include the source evdev path, dispatch-after-read latency, true event age when
evdev kernel timestamps are comparable, repeat trigger/cancel/tick events, and
provider device counts so delayed or missed input can be traced without logging
macro text payloads.
If an already-active non-repeat trigger is pressed again, the later attempt is
skipped/denied deterministically and counted in final stats without stopping the
runner.
Process-scope denials include a privacy-bounded reason such as
`stale_focus`, `focus_unavailable`, `focus_permission_denied`, or
`process_mismatch`; stale denials report the configured rule, metadata age, and
2 second default freshness threshold without logging command-line arguments or
window text.
Evdev motion scopes use a longer 30 second stable-focus threshold so a focused
game does not deny held input solely because no focus-change callback arrived;
the runtime still fails closed if KWin metadata stops refreshing beyond that
threshold.

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
For process-scoped bindings, focused-process metadata must be fresh. Metadata
older than the 2 second default threshold, unavailable metadata, permission
denial, ambiguous focus, or an untrusted timestamp denies the trigger before any
macro action is emitted. Matching resumes automatically on the next trigger
after fresh matching metadata is available.

## Lua API

The Lua surface is intentionally small:

- `macro { ... }` creates one ordered macro.
- `key "<key-name>"` sends a key action.
- `key_down "<key-name>"` sends a key press without the release.
- `key_up "<key-name>"` sends a key release without the press.
- `text "<string>"` sends text input.
- `mouse_click "<left|right|middle>"` sends a mouse button click action.
- `delay <milliseconds>` waits before the next action.
- `hotkeys = { ["F5"] = macro { ... } }` keeps the legacy keyboard binding shape.
- `bindings = { ... }` accepts structured triggers with modifiers, mouse buttons, mouse wheel directions, and an explicit mode.
- `motions = { ... }` accepts uniform sequence notation for leader, keyboard, and mouse tokens.
- `presses = { ... }` accepts immediate single-token actions, optionally guarded by `requires_held`.

Keyboard key names are normalized through one Linux evdev-backed vocabulary
across `leader`, motion triggers, motion `requires_held`, loop `while_held`
tokens, press triggers, press `requires_held`, structured
binding keys, legacy hotkeys, and macro `key`/`key_down`/`key_up` actions. Existing spellings such
as one-character keys, `F1` through `F24`, `Left`, `Right`, `Enter`, `Return`,
`Tab`, `Esc`, `Escape`, `Delete`, `Del`, `Backspace`, and `Space` remain valid.
Expanded names cover standard navigation, editing, keypad, modifier, system,
and media keys when the configured input or output backend supports them.

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
      macro = macro {
        key "Left",
      },
    },
    {
      requires_held = { "<Leader>" },
      trigger = "<WheelDown>",
      mode = "passthrough",
      macro = macro {
        key "Right",
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
all current evdev devices and keep rescanning during the current run, set
`devices = "all"`. Combining `devices = "all"` with grab mode also requires
`acknowledge_risk = "GRAB_ALL_INPUTS"`. Device rescans are current-run only:
added, removed, and skipped unreadable paths are reported in diagnostics, but no
device list is persisted and no background service is installed.

For daily use on NixOS, prefer selected device permissions over repeatedly
running the temporary ACL helper. The flake exports a NixOS module that grants a
dedicated group access only to the devices you match and creates stable symlinks
under `/dev/input/by-signal-auras/`:

```nix
{
  imports = [
    inputs.signal-auras.nixosModules.signal-auras
  ];

  programs.signal-auras.unsafeInput = {
    enable = true;
    users = [ "aron" ];
    selectedDevices = [
      {
        id = "keyboard";
        match = ''ATTRS{name}=="Example Keyboard"'';
      }
      {
        id = "mouse";
        match = ''ATTRS{name}=="Example Mouse"'';
      }
    ];
    uinput.enable = true;
  };
}
```

After rebuilding NixOS, start a new login session so group membership applies,
then point Lua at the selected symlinks instead of `devices = "all"`:

```lua
input_provider = {
  backend = "evdev",
  mode = "grab",
  output = "uinput",
  devices = {
    "/dev/input/by-signal-auras/keyboard",
    "/dev/input/by-signal-auras/mouse",
  },
}
```

Use `signal-auras doctor input <script.lua>` or `just input-doctor file=<script.lua>`
to check the configured evdev paths and `/dev/uinput` access without grabbing
devices or emitting input. `just unsafe-input-acl` remains available for short
local tests; those ACLs can still be reset by reboot, replug, or udev changes.
Scripts can also set `devices = "interactive"` for a terminal startup checklist.
The runner stores the selected devices in a mandatory per-script runtime cache
under `$XDG_RUNTIME_DIR/signal-auras/input-devices/`, keyed by the canonical
main Lua path. On later starts the cache is used only when the cached event
paths still match the recorded device identity and required permissions are
present; otherwise interactive startup prompts again, or non-interactive startup
fails closed. Permission repair from this flow is selected-device scoped and
targets only the chosen evdev paths plus `/dev/uinput` when uinput output is
configured. Use `--reset-input-cache` to discard the runtime cache for that
startup and force the interactive checklist before the runner opens devices.
Use `signal-auras doctor keys <script.lua>` when you need key-name discovery
for the current run. Key diagnostics report current-run device status, raw key
code, canonical token, aliases, triggerability, emittability, and unavailable
reasons without persisting discovered keys. Hardware-only Fn or firmware layer
controls that do not emit Linux input events are reported as unobserved rather
than guessed.

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
      requires_held = { "<Leader>" },
      trigger = { "<LClick>", "<LClick>" },
      mode = "passthrough",
      loop = {
        while_held = { "<LClick>" },
        repeat = {
          every_ms = 65,
          macro = macro {
            mouse_click "left",
          },
        },
      },
    },
  },
}
```

`within_ms` defaults to `500` and controls how quickly the trigger sequence
must complete. `requires_held` accepts holdable tokens (`<Leader>`, keyboard
keys, and mouse buttons) and rejects wheel tokens because wheel input has no
held state. For motions, all required tokens must already be held before the
first trigger press and must remain held until the motion completes; releasing a
required token cancels the active attempt or active loop. Presses fire
immediately when their single trigger press arrives and their guard is
satisfied. `motions[].repeat` has been removed; migrate
`repeat.interval_ms.{min,max}` to `loop.repeat.every_ms` with one fixed interval.
`defaults.inter_action_delay_ms` applies between generated macro actions;
`motion.inter_action_delay_ms` overrides it for one motion. Explicit
`delay(ms)` actions remain part of macros. Delays are valid from zero for
inter-action defaults and one millisecond for explicit `delay` actions.

## Examples

- `examples/poe2.lua`: typed controller-style PoE2 bindings backed by Rust
  output, screen-state, and overlay APIs.

## Input Performance Diagnostics

The unsafe evdev provider waits on input-device readiness instead of relying on
a fixed sleep between polls. When repeat motions are active, the live runner
waits only until the next input readiness or repeat deadline and processes ready
input before due repeat ticks so release events can cancel held repeats first.
If repeat output for a held binding is still pending or active when another
tick becomes due, the later tick is skipped/coalesced instead of queued; skipped
ticks are not replayed after overload clears or after release. Final runtime
stats include motion input count, executed repeat tick count, skipped/coalesced
repeat count, non-repeat skipped/denied collision count, repeat cancel count,
cancelled queued macro runs, maximum observed motion dispatch latency for the
run, true motion event-age samples, unavailable event-age samples, and average,
p95, p99, and max true event age where evdev kernel timestamps are comparable.
Verbose motion logs use `dispatch_after_read_latency_ms` for the existing
userspace-read-to-action metric and `event_age_ms` for kernel-event-to-action
age, or `event_age_ms=unavailable` when a usable kernel timestamp is absent.

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
just run-verbose
just input-doctor
just failures
```

Manual KDE Plasma Wayland verification is documented in
`tests/compositor/manual-wayland-verification.md`. Completion requires a real
KDE session proving desktop-wide shortcut registration/event delivery,
active-process scoped match and non-match behavior, synthesized input success,
denial behavior, and shutdown cleanup.
