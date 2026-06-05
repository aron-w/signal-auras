# Implementation Plan: Runner Architecture Decomposition

**Branch**: `019-runner-decomposition` | **Date**: 2026-06-05 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/019-runner-decomposition/spec.md`

## Summary

Decompose `crates/signal-auras-cli/src/runner.rs` only after behavior is protected by lifecycle cleanup, callback responsiveness, and focus policy tests. The implementation will introduce explicit lifecycle configuration/session ownership, runtime-loop coordination, controller execution, and diagnostics boundaries while preserving public Lua APIs, consent semantics, and current-run resource ownership.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake; Lua 5.4-compatible script surface remains unchanged.

**Primary Dependencies**: Existing workspace crates and existing runtime dependencies. No new dependency is planned.

**Storage**: Repository files only. No persistent runner state, daemon state, IPC state, registry, or cache.

**Testing**: `cargo fmt --check`, targeted runner/core/lua tests for each extracted boundary, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and Nix verification commands `nix develop -c cargo fmt --check`, `nix develop -c cargo clippy --all-targets -- -D warnings`, `nix develop -c cargo test`, and `nix flake check` where feasible.

**Target Platform**: NixOS/Linux/KDE Plasma Wayland with existing current-run input, output, process metadata, callback, and Lua controller consent.

**Project Type**: Rust workspace with core automation library, Lua validation/runtime crate, Wayland adapter crate, and CLI runner.

**Performance Goals**: Refactoring must preserve existing input/callback p95 <= 20 ms and p99 <= 50 ms targets and must not add blocking work to the hot path.

**Constraints**: Preserve Lua APIs, current-run consent, fail-closed permission behavior, focus freshness semantics, callback budgets, shutdown cleanup guarantees, and no hidden global behavior.

**Scale/Scope**: One terminal-started runner process with current-run lifecycle resources, wake sources, Lua controller work, and diagnostics.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-check after Phase 1 design: Passed for planned scope.*

- Library-First: PASS. Behavior remains in Rust library/adapter contracts; CLI decomposition only composes tested boundaries.
- Wayland/Compositor Awareness: PASS. No compositor support claim changes; Wayland/KDE assumptions remain in existing adapter specs.
- Rust Safety Boundaries: PASS. Input, output, signal fd, wake fd, and compositor resources stay behind narrow owned session or adapter types.
- Lua Extension Contract: PASS. No Lua API change; controller execution remains capability-bounded.
- NixOS Reproducibility: PASS. Verification uses existing flake commands and adds no dependency.
- Security and Consent: PASS. Refactor preserves current-run explicit consent and fail-closed resource behavior.
- TDD and Testability: PASS. Existing behavior tests gate refactoring; new boundary tests precede extraction.
- Minimal Composition: PASS. No daemon, async runtime, global registry, IPC endpoint, or persistent store is introduced.
- No Hidden Global Behavior: PASS. No new hooks, autostart, persistence, or ambient capabilities.
- Incremental Delivery: PASS. Lifecycle, loop coordination, and controller execution can be delivered as separable increments.

## Project Structure

### Documentation (this feature)

```text
specs/019-runner-decomposition/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── runner-boundaries.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/signal-auras-cli/src/
├── runner.rs
├── runner/
│   ├── lifecycle.rs
│   ├── runtime_loop.rs
│   ├── controller.rs
│   └── diagnostics.rs
└── main.rs

crates/signal-auras-core/src/
├── controller.rs
├── scope.rs
└── stats.rs

crates/signal-auras-wayland/src/
├── adapter.rs
├── event_loop.rs
├── kde_bridge.rs
└── uinput.rs

tests/contract/
├── cli_runner.rs
├── lua_api.rs
└── rust_library.rs
```

**Structure Decision**: Keep runner decomposition inside the CLI crate unless a boundary proves reusable by core or adapter tests. Move code by responsibility, not by broad style cleanup, and keep behavior-changing fixes in their existing specs.

## Complexity Tracking

No constitution violations.
