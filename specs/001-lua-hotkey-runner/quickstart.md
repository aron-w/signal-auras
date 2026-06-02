# Quickstart: Lua Hotkey Runner

## Prerequisites

- NixOS development shell from this repository.
- Wayland session for manual compositor verification.
- A compositor/session that exposes the required global shortcut, active-process metadata, and synthesized-input capabilities selected during implementation.

## Build And Test

```bash
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test
```

## Example Script

Create a Lua file like:

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

## Run

```bash
nix develop -c cargo run -p signal-auras-cli -- run ./examples/poe2-legacy.lua
```

Expected startup behavior:

- the runner prints the script path
- the runner validates the Lua script
- the runner prints the effective process scope
- the runner probes required Wayland capabilities
- the runner registers `F5` or exits with a diagnosable error

## Scope-Free Script

If the script omits `scope`, the runner prompts in the terminal. Select process names for scoped behavior or explicitly confirm global scope for the current run. Cancel exits without registering hotkeys.

## Manual Wayland Verification

Until a compositor harness exists, manually verify:

1. Start a NixOS Wayland session.
2. Run the sample scoped script.
3. Confirm startup logs show scope and registration result.
4. Focus a process whose visible executable/process name matches the configured scope.
5. Press the hotkey and confirm actions execute in order.
6. Focus a different process.
7. Press the hotkey and confirm no macro action occurs and a denied trigger is logged.
8. Press Ctrl-C.
9. Confirm hotkeys are unregistered and final stats are printed.

## Failure Verification

Run the CLI with:

- zero arguments
- two arguments
- invalid Lua
- a Lua script with no hotkeys
- a script attempting unavailable ambient APIs
- a scope-free script in a non-interactive terminal

Each case must exit before registration and print a diagnosable error.
