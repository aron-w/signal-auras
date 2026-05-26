# Quickstart: KDE Plasma Wayland Adapter

## Prerequisites

- NixOS development shell from this repository.
- KDE Plasma Wayland session.
- KWin running as the active compositor.
- xdg-desktop-portal and xdg-desktop-portal-kde available in the session.
- Terminal access to run Signal Auras and observe diagnostics.

## Build And Test

```bash
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test
nix flake check
```

If `cargo` is not available outside the development shell, use the `nix develop`
forms above. `nix flake check` verifies the development shell and native adapter
dependencies are reproducible.

## Run A Scoped Shortcut

Use the existing example or create a Lua file:

```lua
return {
  scope = { processes = { "kate" } },
  hotkeys = {
    ["F5"] = macro {
      text "signal-auras-kde-test",
    },
  },
}
```

Run:

```bash
nix develop -c cargo run -p signal-auras-cli -- run ./examples/poe2-hideout.lua
```

Expected startup behavior:

- script validates successfully
- effective process scope is printed
- KDE Plasma Wayland provider is selected
- required KDE/portal capabilities are probed
- current-run KDE bridge setup is printed if required
- each registered shortcut prints a registration result
- unsupported or denied capabilities fail before activation

## Manual KDE Plasma Wayland Verification

Until an automated compositor harness exists:

1. Start a NixOS KDE Plasma Wayland session.
2. Confirm the session is Wayland and KDE Plasma.
3. Run the scoped example script.
4. Confirm capability, KDE bridge, and registration diagnostics appear in the terminal.
5. Focus an application matching the configured process name.
6. Press the shortcut and confirm the active-process decision is a match.
7. Confirm the macro emits text or key input in declared order.
8. Focus a different application.
9. Press the shortcut and confirm no input is emitted and a non-match reason is logged.
10. Deny synthesized-input permission if KDE/portal offers a prompt, then confirm zero input is emitted.
11. Press Ctrl-C.
12. Confirm shortcuts are unregistered, portal sessions are closed, KDE bridge state is unloaded, pending input is cancelled, and final stats are printed.

## Failure Verification

Verify diagnosable failures for:

- non-Wayland session
- non-KDE Wayland session
- missing KWin service
- missing xdg-desktop-portal-kde synthesized-input path
- unsupported global shortcut capability
- unsupported active-process metadata with process-scoped shortcuts
- unsupported synthesized input with key/text macro actions
- permission denial for each sensitive capability
- reserved or already-owned hotkey
- KDE bridge setup failure
- KWin or portal invalidation during runtime
- Ctrl-C during macro execution

Each failure must either occur before activation or perform cleanup before exit.

## Completion Criteria

This feature is complete only after a real KDE Plasma Wayland manual run records:

- desktop-wide global shortcut registration and event delivery
- active-process scoped match and non-match decisions
- synthesized key/text input success through the KDE/portal path
- denied or unavailable permission emits zero input
- shutdown cleanup unregisters shortcuts, closes portal sessions, unloads bridge state, and cancels pending input

## Verification Results

Record results here after implementation:

- `nix develop -c cargo fmt --check`: PENDING
- `nix develop -c cargo clippy --all-targets -- -D warnings`: PENDING
- `nix develop -c cargo test`: PENDING
- `nix flake check`: PENDING
- KDE Plasma Wayland manual verification: PENDING

