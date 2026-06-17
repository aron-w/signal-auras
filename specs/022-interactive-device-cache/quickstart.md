# Quickstart: Interactive Device Cache

## Automated Verification

```sh
cargo fmt --check
cargo test -p signal-auras-core input_provider
cargo test -p signal-auras-lua interactive
cargo test -p signal-auras-wayland device_identity
cargo test -p signal-auras-cli input_cache
cargo test
cargo clippy --all-targets -- -D warnings
```

When Nix evaluation is available:

```sh
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test
nix flake check
```

## Manual KDE Startup Check

1. Ensure `examples/poe2.lua` uses `devices = "interactive"`.
2. Remove the runtime cache for that script from
   `$XDG_RUNTIME_DIR/signal-auras/input-devices/`.
3. Run `signal-auras run examples/poe2.lua` in an interactive terminal.
4. Select the intended keyboard and pointer devices.
5. Confirm selected-device ACL repair if permissions are missing.
6. Stop the runner and start it again; the second startup should reuse the
   valid runtime cache without prompting.
7. Run `signal-auras run --reset-input-cache examples/poe2.lua`; startup should
   ignore the valid cache and show the checklist again.
8. Replug or change a selected device; the next startup should reject the stale
   cache and prompt again.

## Diagnostic Check

```sh
signal-auras doctor input examples/poe2.lua
```

The report should include the derived runtime cache path, selected device
statuses, identity validation, permission status, and remediation without
changing permissions or cache contents.
