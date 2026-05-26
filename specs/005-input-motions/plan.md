# Implementation Plan: Unified Input Motions

**Branch**: `005-input-motions` | **Date**: 2026-05-26 | **Spec**: [spec.md](./spec.md)

## Summary

Add Lua `motions` as the next input API: a uniform sequence notation for leader keys, keyboard tokens, mouse click tokens, repeat ownership, and generated-action delay precedence. The implemented increment adds Rust core data types, Lua validation, macro mouse-click actions, capability planning, an explicit unsafe evdev/uinput backend, documentation, LuaLS metadata, and an example script while preserving existing `hotkeys` and `bindings`.

Safe desktop-wide sequence observation remains behind adapter capability boundaries and fails closed until a compositor-specific provider is implemented. The unsafe backend can read configured evdev device files, optionally request evdev grabs for consumed motions, and emit generated input through the KDE portal or `/dev/uinput`.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake; Lua 5.4-compatible script surface.

**Primary Dependencies**: Existing workspace crates plus `libc` for low-level evdev and uinput file descriptor boundaries.

**Storage**: Repository files only. No persistent runtime storage.

**Testing**: `cargo test`, `cargo fmt --check`, `nix develop -c cargo fmt --check`, `nix develop -c cargo test`, and `nix flake check` when the Nix sandbox can evaluate the project.

**Target Platform**: NixOS and KDE Plasma Wayland.

**Constraints**: Current-run only; no daemon, autostart, hidden hooks, or ambient Lua capabilities. Motions touching input observation, consumed trigger events, or synthesized input must declare required capabilities and fail closed when unavailable.

## Constitution Check

GATE: Passed before implementation and re-checked after design.

- Library-First: PASS. Motion tokens, triggers, repeats, delay defaults, and capability planning are Rust library contracts.
- Wayland/Compositor Awareness: PASS. Safe compositor sequence observation remains an explicit provider capability gap; unsafe evdev observe is opt-in.
- Rust Safety Boundaries: PASS. Unsafe blocks are restricted to ioctl/fcntl calls and repr(C) event byte serialization at the evdev/uinput adapter boundary.
- Lua Extension Contract: PASS. `motions` is documented and editor metadata is updated; existing APIs remain compatible.
- NixOS Reproducibility: PASS. Verification commands use the existing flake path.
- Security and Consent: PASS. Motions remain script-declared and current-run only with no hidden global behavior.
- Testable Automation Behavior: PASS. Automated tests cover core validation, Lua parsing, capability planning, and compatibility.
- Minimal Composition: PASS. Existing config, macro, and capability models are extended without new services.
- No Hidden Global Behavior: PASS. No background registration or persistence is added.
- Incremental Delivery: PASS. The implemented slice validates motions, supports unsafe evdev/uinput operation, and defers safe compositor sequence observation.

## Project Structure

```text
crates/signal-auras-core/src/
├── config.rs
├── error.rs
├── hotkey.rs
├── macro_plan.rs
└── motion.rs

crates/signal-auras-lua/src/sandbox.rs
crates/signal-auras-wayland/src/
├── evdev.rs
├── input.rs
└── portal.rs

lua-types/signal-auras.lua
examples/input-motions.lua
README.md
tests/contract/
specs/005-input-motions/contracts/
├── unsafe-evdev-provider.md
├── lua-api.md
└── kde-motion-provider.md
```

## Complexity Tracking

No constitution violations.
