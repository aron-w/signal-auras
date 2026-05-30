# Quickstart: Stale Focus Handling

Run automated verification:

```sh
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test
nix flake check
```

Focused manual verification, when a KDE Wayland session and permissions are available:

```sh
just run-verbose
```

Configure a process-scoped binding, trigger it while the focused process matches, then trigger after metadata is unavailable or delayed during a focus change. Confirm fresh matching metadata allows the macro, stale or unavailable metadata denies it before emitted input, and verbose diagnostics distinguish stale, unavailable, permission denied, and mismatch reasons without logging command-line arguments or window text.
