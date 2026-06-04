# Implementation Plan: Overlay Render Providers

**Branch**: `018-overlay-render-providers` | **Date**: 2026-06-04 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/018-overlay-render-providers/spec.md`

## Summary

Add provider-based scoped overlay rendering for PoE2 screen tracker state. The implementation introduces a Rust-owned provider-neutral overlay model, Lua `sa.overlay.mount(...)` parsing, typed state-to-progress-bar mapping, fail-closed lifecycle diagnostics, and a v1 native pass-through renderer adapter seam that can be exercised without real compositor overlay hardware. Future WebView/TypeScript, Tauri-style windows, and normal tool windows are represented as provider ids/adapters but do not own hotkeys, macros, screen capture, focus, or capability decisions.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake; Lua 5.4-compatible controller/declaration surface remains the user-facing extension layer.

**Primary Dependencies**: Existing workspace crates and existing Lua/parser dependencies. No new runtime dependency is required for the first increment; the v1 native renderer contract is modeled as a Rust adapter trait and an in-memory test provider before any real compositor surface dependency is added.

**Storage**: Repository files only. No persistent overlay layout store, daemon state, IPC state, screenshot cache, provider cache, or cross-run UI state.

**Testing**: `cargo fmt --check`, targeted `cargo test -p signal-auras-core overlay`, targeted `cargo test -p signal-auras-lua overlay`, full `cargo test`, `cargo clippy --all-targets -- -D warnings`, and Nix verification commands `nix develop -c cargo fmt --check`, `nix develop -c cargo clippy --all-targets -- -D warnings`, `nix develop -c cargo test`, and `nix flake check` where feasible.

**Target Platform**: NixOS/Linux/KDE Plasma Wayland. Real overlay presentation is scoped to current-run trusted focus and compositor/provider availability; unsupported providers fail closed.

**Project Type**: Rust workspace with core automation library, Lua validation crate, Wayland adapter crate, CLI runner, and contract/integration tests.

**Performance Goals**: Overlay state-to-visual mapping is an in-memory transformation over typed tracker snapshots. Simulated updates for the two PoE2 bars should avoid macro scheduling and input interception, and should stay bounded to one render snapshot per overlay update tick. Future real rendering should target normal desktop overlay responsiveness without changing existing input event-loop p95 <= 20 ms and p99 <= 50 ms goals.

**Constraints**: Lua and future UI providers receive no raw screen buffers, input streams, compositor handles, permission handles, ambient filesystem access, or ambient network access. Overlay providers do not own hotkeys, macros, screen capture, focus decisions, capability decisions, or automation scheduling. Scoped overlays are current-run only, fail closed for unavailable provider/permission/focus/state, and must not consume input.

**Scale/Scope**: One terminal-started runner process, one PoE2-scoped overlay definition with two progress-bar visuals, existing state trackers (`refutation_cooldown`, `heavy_stun`), an in-memory test renderer, and a documented manual KDE/PoE2 pass-through verification path.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-checked after Phase 1 design: Passed.*

- Library-First: PASS. Overlay definitions, validation, provider selection, lifecycle decisions, and state-to-visual snapshots are Rust library behavior before CLI, Lua, or real renderer wiring.
- Wayland/Compositor Awareness: PASS. KDE Plasma Wayland remains the manual target; unsupported providers, unavailable pass-through surfaces, denied permissions, and inactive focus fail closed with diagnostics.
- Rust Safety Boundaries: PASS. Real compositor surface creation is isolated behind a provider adapter; Lua and future UI providers receive sanitized snapshots only.
- Lua Extension Contract: PASS. `sa.overlay.mount(...)` is a startup registration API with explicit validation and no ambient Lua capability expansion.
- NixOS Reproducibility: PASS. No new dependency is required for the core increment; verification uses existing flake commands.
- Security and Consent: PASS. Overlays require explicit script declarations, scoped focus, current-run capability checks, pass-through behavior, and cleanup. No screen/input/process authority is delegated to renderer UI code.
- TDD and Testability: PASS. Tasks require parser/validation tests, state-to-visual mapping tests, provider fallback tests, input non-interference tests, and manual compositor verification documentation before real desktop claims.
- Minimal Composition: PASS. No daemon, async runtime, IPC endpoint, persistent state store, or global provider registry is introduced.
- No Hidden Global Behavior: PASS. Overlay surfaces are created only from explicit configuration during the current run and are cleaned up on shutdown/failure.
- Incremental Delivery: PASS. P1 PoE2 bars, P2 provider-neutral declarations, and P3 isolation/security behavior are independently testable.

## Project Structure

### Documentation (this feature)

```text
specs/018-overlay-render-providers/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── lua-overlay-api.md
│   └── overlay-renderer-library.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/signal-auras-core/src/
├── overlay.rs
├── screen_state.rs
├── controller.rs
├── error.rs
└── lib.rs

crates/signal-auras-lua/src/
├── sandbox.rs
└── lib.rs

crates/signal-auras-wayland/src/
├── overlay.rs
└── lib.rs

crates/signal-auras-cli/src/
└── runner.rs

tests/contract/
├── lua_api.rs
└── rust_library.rs

tests/compositor/
└── manual-wayland-verification.md

examples/
└── poe2.lua
```

**Structure Decision**: Keep overlay definitions, typed visual state, diagnostics, and provider-selection lifecycle in `signal-auras-core::overlay`. Keep Lua parsing as a consumer that constructs core overlay definitions. Keep the v1 renderer adapter behind `signal-auras-wayland::overlay` so real pass-through compositor work remains isolated, while tests can use an in-memory renderer. Keep CLI changes limited to composing state tracker snapshots into overlay updates and provider diagnostics.

## Complexity Tracking

No constitution violations.
