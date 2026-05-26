# Implementation Plan: Composite Input Bindings

**Branch**: `004-composite-bindings` | **Date**: 2026-05-26 | **Spec**: [spec.md](./spec.md)

## Summary

Add structured `bindings` beside legacy `hotkeys` so Lua scripts can express modifier plus pointer triggers such as `Ctrl` plus wheel up/down and `Ctrl` plus left click. The Rust core owns normalized trigger validation, duplicate detection, binding mode semantics, capability requirements, and runner lifecycle behavior. The current KDE Wayland provider fails closed for consumed composite pointer bindings until a real event-observation and consumption provider exists.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake; Lua 5.4-compatible script surface.

**Primary Dependencies**: Existing workspace crates only.

**Storage**: Repository files only. No persistent runtime storage.

**Testing**: `cargo test`, `nix develop -c cargo fmt --check`, `nix develop -c cargo test`, `nix develop -c cargo clippy --all-targets -- -D warnings`, and `nix flake check`.

**Target Platform**: NixOS and KDE Plasma Wayland.

**Constraints**: Current-run only, no daemon, no autostart, no hidden global behavior, no polling input observation provider, explicit capability failure for unsupported consumed pointer bindings.

## Constitution Check

GATE: Passed before implementation and re-checked after design.

- Library-First: PASS. Trigger parsing, normalization, capability requirements, and runner behavior are Rust library contracts.
- Wayland/Compositor Awareness: PASS. Composite pointer provider gaps are explicit capability failures.
- Rust Safety Boundaries: PASS. No new unsafe code or privileged helper.
- Lua Extension Contract: PASS. New structured API is documented; legacy `hotkeys` remains compatible.
- NixOS Reproducibility: PASS. Verification uses existing flake commands.
- Security and Consent: PASS. No persistence or hidden registration; consumed events fail closed when unsupported.
- Testable Automation Behavior: PASS. Automated tests cover parser, core model, capability gating, runner lifecycle, and cleanup behavior.
- Minimal Composition: PASS. Existing runner and adapter traits are extended instead of adding services.
- No Hidden Global Behavior: PASS. All bindings are current-run only.
- Incremental Delivery: PASS. This feature adds validated API and fails closed where real compositor support is not yet available.

## Project Structure

```text
crates/signal-auras-core/src/
├── config.rs
├── error.rs
├── hotkey.rs
└── stats.rs

crates/signal-auras-lua/src/sandbox.rs
crates/signal-auras-cli/src/runner.rs
crates/signal-auras-wayland/src/
├── adapter.rs
├── capability.rs
├── kde.rs
└── kde_bridge.rs

lua-types/signal-auras.lua
README.md
tests/
├── contract/
└── integration/
```

## Complexity Tracking

No constitution violations.
