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

1. Prefer the NixOS selected-device module from `README.md` for persistent
   evdev/uinput access, then start a new login session.
2. Run `just input-doctor` and confirm the selected evdev paths and
   `/dev/uinput` report `status=ok`.
3. For short local tests only, grant temporary local ACLs with
   `just unsafe-input-acl`.
4. Run `just run-verbose`.
5. In Path of Exile 2, verify:
   - `F5` emits `/hideout`.
   - `F3` plus wheel up/down emits Left/Right promptly.
   - `F3` plus double left-click hold starts repeat clicking.
   - Releasing the held click stops repeat clicking immediately.
6. Switch keyboard/input device modes and confirm verbose logs show device removal/addition or skipped devices instead of silent loss.
