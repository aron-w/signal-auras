# Implementation Plan: PoE2 Screen State Tracking

**Branch**: `017-poe2-state-tracking` | **Date**: 2026-06-04 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/017-poe2-state-tracking/spec.md`

## Summary

Add observation-only screen state trackers for PoE2 UI elements. The implementation introduces Rust-owned tracker registration, detector config validation, cooldown/progress state estimation, polling/capability gating with per-tracker active-process scope checks, Lua `sa.state.track(...)` parsing, privacy-bounded diagnostics, and fixture-based detector tests using files under `examples/poe2/`. Runtime media fixture paths are not exposed to Lua configuration.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake; Lua 5.4-compatible script surface remains the user-facing extension layer.

**Primary Dependencies**: Existing workspace crates and `mlua`. No new runtime dependency is required for this increment; fixture tests use committed media bytes as deterministic detector inputs instead of adding a native video decoding stack.

**Storage**: Repository files only. No persistent screenshots, tracker state store, capture cache, IPC state, or daemon state.

**Testing**: Targeted Rust core tests for tracker validation, cooldown estimation, progress detection, permission/focus gating, and batched polling; Lua contract tests for `sa.state.track`; example fixture tests under `tests/contract/rust_library.rs`; verification with `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and Nix commands where feasible.

**Target Platform**: NixOS/Linux/KDE Plasma Wayland. Screen capture is modeled as a current-run screen-read capability and future compositor adapter boundary; unsupported capture fails closed.

**Project Type**: Rust workspace with core automation library, Lua validation crate, Wayland adapter crate, CLI runner, and contract/integration tests.

**Performance Goals**: Simulated two-tracker polling at 50 ms shares one screen sample per poll tick. Detector work is bounded by configured ROI sizes, with no per-tracker capture session.

**Constraints**: Observation-only; no synthesized input, no input capture, no callbacks, no raw screen buffers to Lua, no fixture paths in runtime config, explicit current-run `screen_read` capability, fail closed for denied permission, unsupported capture, or untrusted focus.

**Scale/Scope**: One terminal-started runner process, scoped PoE2 focus, two initial detector kinds (`radial_cooldown`, `horizontal_progress_bar`), explicit ROIs for 3840x2160 PoE2 fullscreen UI, and repository fixture media for automated tests.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-checked after Phase 1 design: Passed.*

- Library-First: PASS. Tracker config, detector kinds, tracker states, cooldown estimation, and polling decisions are Rust library behavior before Lua or runtime wiring.
- Wayland/Compositor Awareness: PASS. Screen capture is a KDE/Wayland current-run adapter boundary and fails closed when unsupported or denied.
- Rust Safety Boundaries: PASS. Lua receives no raw buffers or compositor handles; screen samples remain Rust-owned.
- Lua Extension Contract: PASS. `sa.state.track(...)` is a stable observation-only extension with explicit capabilities and no user-declared emitted fields.
- NixOS Reproducibility: PASS. Verification uses existing flake commands and committed fixtures.
- Security and Consent: PASS. `screen_read` is explicit, current-run scoped, revocable, and denied states produce no screen samples.
- TDD and Testability: PASS. Core detector, Lua parser, and runtime gating tests are planned before implementation.
- Minimal Composition: PASS. No daemon, async runtime, persistent capture service, IPC endpoint, or global registry is introduced.
- No Hidden Global Behavior: PASS. Trackers are inert until explicitly declared and runtime capability/focus checks pass.
- Incremental Delivery: PASS. Refutation cooldown, Heavy Stun progress, and safe Lua registration are independently testable slices.

## Project Structure

### Documentation (this feature)

```text
specs/017-poe2-state-tracking/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── state-tracker-library.md
│   └── lua-state-api.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/signal-auras-core/src/
├── screen_state.rs
├── controller.rs
├── error.rs
└── lib.rs

crates/signal-auras-lua/src/
├── sandbox.rs
└── lib.rs

crates/signal-auras-wayland/src/
├── capability.rs
└── lib.rs

crates/signal-auras-cli/src/
└── runner.rs

tests/contract/
├── lua_api.rs
└── rust_library.rs

examples/
├── poe2.lua
└── poe2/
    ├── refutation_cooldown.webm
    └── progress_heavy_stun.webm
```

**Structure Decision**: Keep detector schemas, state estimation, diagnostics, and polling/capability decisions in `signal-auras-core::screen_state`. Keep Lua parsing as a consumer that only constructs Rust tracker definitions. Keep real screen capture unavailable/fail-closed in the current adapter layer until a compositor capture implementation is specified separately. Keep fixtures under `examples/poe2/` and reference them only from tests.

## Complexity Tracking

No constitution violations.
