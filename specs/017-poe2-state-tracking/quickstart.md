# Quickstart: PoE2 Screen State Tracking

## Example

`examples/poe2.lua` declares:
- `refutation_cooldown` with `radial_cooldown`
- `heavy_stun` with `horizontal_progress_bar`

Both trackers are observation-only and request `screen_read`.

## Verification

Run targeted tests:

```sh
cargo test -p signal-auras-core screen_state
cargo test --test lua_api state_trackers
cargo test --test rust_library poe2_screen_state
```

Run broader checks:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test
nix flake check
```

## Runtime Behavior

Registration validates tracker definitions. Runtime polling only samples the screen when:
- `screen_read` is granted for the current run
- compositor capture support is available
- the tracker scope is trusted and active

Denied or inactive states produce diagnostics and no screen sample.
