# Quickstart: Callback Wakeups

## Automated Verification

Run from the repository root:

```sh
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test
nix flake check
```

## Focused Checks

```sh
cargo test -p signal-auras-wayland callback
cargo test -p signal-auras-core callback
cargo test --test runner_flow callback
cargo test --test cli_runner callback
```

## Manual KDE Smoke Check

1. Start a KDE Plasma Wayland session.
2. Run `signal-auras run --verbose <lua-file>` with a configured keyboard
   shortcut macro.
3. Press the registered shortcut while no physical input stream is active.
4. Confirm verbose diagnostics include callback receipt and dispatch latency.
5. Press Ctrl-C and confirm shutdown completes, shortcuts are cleaned up, and no
   callback-started macro begins after shutdown starts.
