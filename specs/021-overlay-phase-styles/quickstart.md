# Quickstart: Overlay Phase Styles

## Validate Targeted Behavior

```sh
cargo test -p signal-auras-core overlay
cargo test -p signal-auras-lua overlay
cargo test --test lua_api poe2
```

## Optional Broader Checks

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

## Manual Review

1. Open `examples/poe2.lua`.
2. Confirm `detector.phases` contain sampling and threshold language only.
3. Confirm Refutation phase color/opacity language is under `sa.overlay.mount`.
