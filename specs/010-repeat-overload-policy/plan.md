# Implementation Plan: Repeat Overload Policy

**Branch**: `010-repeat-overload-policy` | **Date**: 2026-05-30 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/010-repeat-overload-policy/spec.md`

## Summary

Define and implement bounded overload policy for trigger work. The existing slice covers held-repeat overload: when repeat output for a held binding is still pending or active, later due ticks for the same binding are skipped/coalesced rather than queued. The architecture-review follow-up extends the same runtime reliability model to non-repeat trigger collisions so repeated or mashed input for an already-active trigger is skipped, coalesced, or denied deterministically while the always-on runner continues. Verbose diagnostics and final summaries report executed, skipped/coalesced, denied, and cancelled work without exposing private macro payloads.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake.

**Primary Dependencies**: Existing workspace crates, `mio`/`nix`/`udev` runtime event-loop dependencies from the prior runtime performance work, and existing tracing diagnostics. No new dependency is required.

**Storage**: Repository files only. No daemon state, persistent queue, cache, or user data store.

**Testing**: `nix develop -c cargo fmt --check`, `nix develop -c cargo clippy --all-targets -- -D warnings`, `nix develop -c cargo test`, and `nix flake check` when feasible. Automated tests cover slow output overload, long-held repeat stability, non-repeat trigger collisions, active trigger cleanup, cancellation races, independent bindings, shutdown/cancel drain behavior, diagnostics, and unchanged Lua loading.

**Target Platform**: NixOS/Linux/KDE Plasma Wayland with current-run explicit unsafe evdev/uinput opt-in for motion repeats.

**Project Type**: Rust workspace with core automation library, Lua validation crate, Wayland adapter crate, and CLI runner.

**Performance Goals**: Slow-output stress tests keep the runner alive for at least 10,000 due repeat opportunities; per-repeat and per-non-repeat-trigger pending output remains bounded to at most one active/pending macro run for the same trigger; cancellation input and shutdown remain serviceable while overload occurs.

**Constraints**: Preserve existing Lua repeat syntax and macro consent requirements. Do not add hidden global behavior, daemon state, IPC, new unsafe scope, or ambient Lua capabilities. Permission denial/revocation continues to fail closed with diagnostics.

**Scale/Scope**: One terminal-started runner process, a small set of local input devices, multiple simultaneously held repeat bindings, and repeated or mashed attempts for already-active non-repeat triggers in a single runtime.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-checked after Phase 1 design: Passed.*

- Library-First: PASS. Repeat overload accounting and bounded scheduling are Rust behavior with tests before CLI-facing diagnostics.
- Wayland/Compositor Awareness: PASS. The feature changes runtime repeat scheduling only; existing KDE Wayland unsafe evdev/uinput opt-in, permission, and unavailable-protocol behavior remain unchanged.
- Rust Safety Boundaries: PASS. No new unsafe Rust or fd/protocol boundary is introduced.
- Lua Extension Contract: PASS. Lua syntax, capability scope, and macro consent are unchanged.
- NixOS Reproducibility: PASS. No new dependencies; verification uses existing flake commands.
- Security and Consent: PASS. Cancellation and fail-closed permission behavior are safety requirements; no ambient access is added.
- TDD and Testability: PASS. Automated tests are planned for core overload behavior, runtime queue behavior, diagnostics, and Lua loading regression.
- Minimal Composition: PASS. No new service, async runtime, persistent queue, or global registry.
- No Hidden Global Behavior: PASS. Runtime remains current-run only and user configured.
- Incremental Delivery: PASS. P1 stability, P2 cancellation safety, and P3 diagnostics are independently testable.

## Project Structure

### Documentation (this feature)

```text
specs/010-repeat-overload-policy/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── repeat-overload-runtime.md
└── tasks.md
```

### Source Code (repository root)
```text
crates/signal-auras-core/src/
├── macro_plan.rs
├── motion.rs
└── stats.rs

crates/signal-auras-cli/src/
└── runner.rs

tests/contract/
├── cli_runner.rs
└── lua_api.rs

tests/integration/
└── runner_flow.rs
```

**Structure Decision**: Keep overload policy and counters in existing core/runtime data structures where they can be tested without compositor hardware. Keep CLI changes limited to the live runner queue, repeat tick scheduling, non-repeat active-trigger state, cleanup, and verbose/final diagnostics. Keep Lua and Wayland adapter code unchanged unless tests expose an existing integration requirement.

## Complexity Tracking

No constitution violations.
