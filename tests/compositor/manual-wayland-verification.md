# Manual Wayland Verification

Automated compositor coverage is not available in this repository yet. Use this
procedure in a real NixOS Wayland session for the v1 runner.

## Selected Adapter Support

The implementation currently selects a mock-friendly Wayland adapter boundary,
not a real compositor protocol implementation. The current adapter behavior is:

- Global shortcut registration: mock registration IDs are returned in-process.
- Active process metadata: supplied by the adapter test/mock state.
- Synthesized input: explicitly unavailable with a `SynthesizedInput`
  capability error.
- Real compositor protocols or portals: not selected yet.

This means the procedure below verifies the documented startup, validation,
consent, cleanup, and unsupported-capability paths. It does not prove real
desktop-wide hotkey capture or synthesized input behavior until a compositor
adapter is implemented.

## Scoped Script Procedure

1. Enter the development shell with `nix develop`.
2. Run `cargo run -p signal-auras-cli -- run ./examples/poe2-hideout.lua`.
3. Confirm startup output shows the Lua file, validation, effective scope,
   mock capability probe, and `F5` registration result.
4. If the run attempts macro execution, confirm synthesized input fails with a
   diagnosable `SynthesizedInput` capability error rather than silently doing
   nothing.
5. Press Ctrl-C if the process is still running and confirm hotkey cleanup plus
   final stats output.

## Prompt Scope Procedure

1. Run `cargo run -p signal-auras-cli -- run ./examples/prompt-scope.lua`.
2. Confirm the terminal prints the missing-scope prompt before registration.
3. Select `1`, enter one or more comma-separated process names, and confirm the
   effective scope is logged for the current run.
4. Run the same command again, select `2`, type `GLOBAL`, and confirm explicit
   global selection is required before registration.
5. Run the same command again, select `3`, and confirm the runner exits without
   registering hotkeys.

## Future Real-Compositor Procedure

After real compositor support is selected, extend this file with the compositor
name, required protocols or portals, permission prompts, and exact commands
used to verify:

1. Real global shortcut registration.
2. Active-process matching against a focused application.
3. Denied trigger behavior for a non-matching focused application.
4. Ordered key, text, and delay action execution.
5. Ctrl-C unregister and final stats output.

Known gap: real compositor protocol support must replace the current skeleton
before this procedure can prove desktop-wide global shortcut or synthesized
input behavior.
