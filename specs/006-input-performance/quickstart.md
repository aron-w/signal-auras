# Quickstart: Input Motion Performance and Consistency

## Automated Verification

```bash
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test
nix flake check
```

## Focused Test Runs

```bash
nix develop -c cargo test -p signal-auras-wayland evdev
nix develop -c cargo test --test runner_flow repeat
nix develop -c cargo test --test lua_api input_provider
```

## Manual KDE Wayland Smoke

1. Grant temporary local ACLs with `just unsafe-input-acl`.
2. Run `just run-verbose`.
3. In Path of Exile 2, verify:
   - `F5` emits `/hideout`.
   - `F3` plus wheel up/down emits Left/Right promptly.
   - `F3` plus double left-click hold starts repeat clicking.
   - Releasing the held click stops repeat clicking immediately.
4. Switch keyboard/input device modes and confirm verbose logs show device removal/addition or skipped devices instead of silent loss.
