# Quickstart: Lua Controller Runtime

## Example Controller

`main.lua`:

```lua
sa.import("motions")

sa.hotkey({
  trigger = "F5",
  scope = { processes = { "poe2.exe" } },
  callback = "hideout",
})
```

`motions.lua`:

```lua
sa.motion({
  trigger = "<Leader> x",
  mode = "passthrough",
  callback = "motion",
})
```

Loading `main.lua` collects both registrations and validates capabilities before any runtime activation.

## Verification

Targeted local checks:

```sh
cargo fmt --check
cargo test -p signal-auras-core controller
cargo test -p signal-auras-lua controller
```

Full local check:

```sh
cargo test
```

Nix checks when feasible:

```sh
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test
nix flake check
```

## Manual KDE Follow-Up

Live KDE/Wayland activation is out of scope for this increment. A later runner integration slice should load a controller, probe required capabilities, activate providers only after validation succeeds, and confirm callback dispatch remains bounded under mixed input.
