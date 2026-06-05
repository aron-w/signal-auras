# Implementation Plan: Overlay Phase Styles

**Branch**: `021-overlay-phase-styles` | **Date**: 2026-06-06 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/021-overlay-phase-styles/spec.md`

## Summary

Clarify the Lua API boundary between observation and presentation. Radial detector phase rules become recognition-only by rejecting style fields, while overlay progress-bar visuals gain optional `activated` and `active` style overrides for radial cooldown phases. The PoE2 example moves phase colors from the tracker declaration to the overlay declaration.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake; Lua 5.4-compatible script surface remains the user-facing configuration layer.

**Primary Dependencies**: Existing workspace crates and existing `mlua` parser/runtime dependency. No new dependency is required.

**Storage**: Repository files only. No persistent overlay state, tracker state, cache, IPC state, daemon state, or fixture store changes.

**Testing**: Targeted `cargo test -p signal-auras-core overlay`, targeted `cargo test -p signal-auras-lua overlay`, targeted `cargo test --test lua_api poe2`, full `cargo test` if targeted checks pass, and Nix verification commands where feasible.

**Target Platform**: NixOS/Linux/KDE Plasma Wayland remains the runtime target. This feature changes parser/core snapshot semantics and examples only; real compositor presentation behavior remains behind existing provider gates.

**Project Type**: Rust workspace with core automation library, Lua validation crate, Wayland adapter crate, CLI runner, and contract tests.

**Performance Goals**: Overlay state-to-visual mapping remains an in-memory transformation over typed tracker states. No new screen capture, input, compositor, or scheduling work is added.

**Constraints**: Preserve current-run consent, Lua sandbox boundaries, provider fail-closed behavior, stale/focus gating, and existing overlay defaults when phase style overrides are absent. Do not introduce a daemon, IPC endpoint, persistent state, new provider, or ambient Lua capability.

**Scale/Scope**: One PoE2 example overlay, radial cooldown progress-bar phase styles, parser validation for detector phase style leakage, and test coverage for the existing native provider-neutral snapshot model.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-checked after Phase 1 design: Passed.*

- Library-First: PASS. Phase style application and validation live in core/Lua parser contracts before examples.
- Wayland/Compositor Awareness: PASS. No new compositor support is claimed; provider availability and pass-through behavior remain unchanged.
- Rust Safety Boundaries: PASS. No OS handles or raw buffers are exposed to Lua or overlay providers.
- Lua Extension Contract: PASS. The script API contract is explicit: trackers observe, overlays present.
- NixOS Reproducibility: PASS. No dependency changes; verification uses existing cargo and flake commands.
- Security and Consent: PASS. The change adds no new capabilities and preserves fail-closed behavior.
- TDD and Testability: PASS. Core and Lua parser tests must be added before implementation.
- Minimal Composition: PASS. The change extends existing overlay style structures; no new service or registry is introduced.
- No Hidden Global Behavior: PASS. Startup registration remains inert and current-run only.
- Incremental Delivery: PASS. Validation cleanup, overlay phase styles, and example cleanup are independently testable.

## Project Structure

### Documentation (this feature)

```text
specs/021-overlay-phase-styles/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── lua-overlay-phase-styles.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/signal-auras-core/src/
└── overlay.rs

crates/signal-auras-lua/src/
└── sandbox.rs

tests/contract/
└── lua_api.rs

examples/
└── poe2.lua

specs/017-poe2-state-tracking/contracts/
└── lua-state-api.md

specs/018-overlay-render-providers/contracts/
└── lua-overlay-api.md
```

**Structure Decision**: Keep render mapping in `signal-auras-core::overlay`, Lua validation in `signal-auras-lua::sandbox`, and the public teaching example in `examples/poe2.lua`. Documentation updates are limited to existing Lua state/overlay contracts and this feature's contract.

## Complexity Tracking

No constitution violations.
