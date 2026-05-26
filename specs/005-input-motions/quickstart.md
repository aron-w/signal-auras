# Quickstart: Unified Input Motions

## Example

```bash
cargo run -p signal-auras-cli -- run ./examples/input-motions.lua
```

The current KDE provider does not yet implement safe compositor-side motion
observation. For high-trust local testing, scripts may opt into
`input_provider = { backend = "evdev", mode = "grab", output = "uinput" }`
with explicit `/dev/input/event*` paths, or `devices = "all"` to scan every
current event device. Evdev observe mode can drive passthrough motions and live
repeat ticks; evdev grab mode can satisfy consumed motions if the kernel grants
exclusive access to those devices. `devices = "all"` plus grab mode requires
`acknowledge_risk = "GRAB_ALL_INPUTS"`.

## Verification

```bash
cargo fmt --check
cargo test
nix develop -c cargo fmt --check
nix develop -c cargo test
nix flake check
```

Manual compositor verification for the safe KDE provider remains blocked until a
KDE input observation provider can report real motion events and held-state
release. Unsafe evdev verification requires local device paths and appropriate
read permissions.
