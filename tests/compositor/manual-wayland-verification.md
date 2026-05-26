# Manual KDE Plasma Wayland Verification

Automated compositor coverage is not available in this repository yet. Use this
procedure in a real NixOS KDE Plasma Wayland session for the v1 runner.

## Selected Adapter Support

The first real provider target is KDE Plasma Wayland. The feature is not
complete until this file records a successful manual run for all three desktop
capabilities:

- desktop-wide global shortcut registration and event delivery
- active-process scoped match and non-match decisions from KDE/KWin metadata
- synthesized key/text input through the KDE/portal path

Non-KDE sessions, X11 sessions, missing KWin services, missing portal support,
permission denial, reserved shortcuts, and provider invalidation must fail
closed with diagnosable output.

## Environment Baseline

Record before the run:

- Date:
- Machine/session:
- KDE Plasma version:
- KWin session type:
- xdg-desktop-portal-kde status:
- Signal Auras command:

## Scoped Script Procedure

1. Enter the development shell with `nix develop`.
2. Start a KDE Plasma Wayland session.
3. Run `cargo run -p signal-auras-cli -- run ./examples/poe2-hideout.lua` or a dedicated KDE test script.
4. Confirm startup output shows the Lua file, validation, effective scope, KDE provider selection, required capability probe, bridge setup if needed, and per-hotkey registration result.
5. Focus an application matching the configured process name.
6. Press the shortcut and confirm one shortcut event is reported.
7. Confirm the active-process decision is logged as a match.
8. Confirm key/text macro actions are emitted in declared order.
9. Focus a non-matching application.
10. Press the shortcut and confirm no input is emitted and the non-match reason is logged.
11. Press Ctrl-C if the process is still running.
12. Confirm hotkey cleanup, portal session cleanup, KDE bridge unload, pending input cancellation, and final stats output.

## Prompt Scope Procedure

1. Run `cargo run -p signal-auras-cli -- run ./examples/prompt-scope.lua`.
2. Confirm the terminal prints the missing-scope prompt before registration.
3. Select `1`, enter one or more comma-separated process names, and confirm the effective scope is logged for the current run.
4. Run the same command again, select `2`, type `GLOBAL`, and confirm explicit global selection is required before registration.
5. Run the same command again, select `3`, and confirm the runner exits without registering hotkeys.

## Capability Failure Verification

1. Run from a non-KDE Wayland session if available and confirm unsupported-provider failure before registration.
2. Run from an X11 session if available and confirm unsupported-session failure before registration.
3. Disable or remove xdg-desktop-portal-kde from the test session if practical and confirm synthesized-input capability failure before macro execution.
4. Use a reserved or already-owned hotkey and confirm any prior registrations are cleaned up.
5. Deny synthesized-input permission if KDE/portal offers a prompt and confirm zero input is emitted.
6. Stop or invalidate KWin/portal during a run if practical and confirm cleanup before exit.

## Global Shortcut Verification

1. Configure a single KDE-supported hotkey that is not already reserved by Plasma.
2. Start the runner and confirm `provider selected=kde-plasma-wayland` appears before registration.
3. Confirm the hotkey registration output includes the configured key and a KDE provider handle.
4. Trigger the hotkey from a focused application outside the terminal and confirm exactly one event is reported.
5. Repeat with a reserved or already-owned hotkey and confirm startup exits after cleaning up any earlier handles.
6. Press Ctrl-C and confirm no shortcut remains active after shutdown.

## Active Process Metadata Verification

1. Configure a process-scoped shortcut for a visible KDE application such as Kate.
2. Focus that application and press the registered shortcut.
3. Confirm the runner logs an active-process match with the visible process or application identity.
4. Focus a different application and press the same shortcut.
5. Confirm the runner logs a non-match and emits no macro input.
6. Repeat on a privileged or compositor-owned surface such as a lock screen or launcher when practical and confirm it is treated as unavailable or ambiguous.
7. Invalidate or deny the metadata path when practical and confirm startup or event handling fails closed with a KWin diagnostic.

## Synthesized Input Verification

1. Configure a macro that emits short ASCII text into a focused KDE text editor.
2. Start the runner in KDE Plasma Wayland and grant any portal permission prompt.
3. Press the registered shortcut and confirm the text appears once in declared order.
4. Repeat with portal permission denied and confirm zero input is emitted.
5. Repeat with text that the key-translation path cannot represent and confirm no partial text is emitted.
6. Press Ctrl-C during or immediately after a macro and confirm portal input is cancelled before exit.

## Results

- KDE provider selected:
- Global shortcut registration:
- Shortcut event delivery:
- Active-process match:
- Active-process non-match:
- Synthesized input success:
- Denied synthesized input emits zero input:
- Ctrl-C cleanup:
- Unsupported-session diagnostics:
- Notes:
