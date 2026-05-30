# Implementation Plan: True Input Latency Metrics

**Branch**: `013-true-input-latency-metrics` | **Date**: 2026-05-30 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/013-true-input-latency-metrics/spec.md`

## Summary

Extend evdev observation and runtime stats so raw kernel event timestamps are parsed and preserved where available, true kernel-event-to-action age/backlog metrics are reported separately from dispatch-after-userspace-read latency, and diagnostics clearly distinguish the two metric families.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake.

**Primary Dependencies**: Existing `libc` evdev event structures, runtime stats, CLI runner diagnostics, and current `mio`/`nix` event-loop dependencies. No new dependency is expected.

**Storage**: Repository files only. No persistent metric store.

**Testing**: `nix develop -c cargo fmt --check`, `nix develop -c cargo clippy --all-targets -- -D warnings`, `nix develop -c cargo test`, and `nix flake check` when feasible. Automated tests cover raw timestamp parsing, unavailable/invalid timestamp handling, true event-age calculations, existing dispatch metric compatibility, and diagnostic labels.

**Target Platform**: NixOS/Linux/KDE Plasma Wayland with explicit unsafe evdev/uinput opt-in for input observation.

**Project Type**: Rust workspace with evdev adapter boundary, core stats, CLI runner diagnostics, and integration tests.

**Performance Goals**: Timestamp preservation and metric recording remain constant-time per observed event and do not add blocking calls to the hot path.

**Constraints**: Preserve Lua APIs, current input consent boundaries, privacy-bounded diagnostics, no hidden global behavior, and no persistent metrics. True event age must be reported unavailable when kernel timestamps cannot be compared safely.

**Scale/Scope**: One terminal-started runner process, local evdev events, existing motion/hotkey dispatch metrics, and final/verbose runtime diagnostics.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-check after Phase 1 design: Passed for planned scope.*

- Library-First: PASS. Timestamp parsing and metric calculations are Rust behavior with tests before CLI diagnostics.
- Wayland/Compositor Awareness: PASS. Scope is explicit evdev on NixOS/KDE Wayland and unsupported timestamp cases are diagnosable.
- Rust Safety Boundaries: PASS. Raw evdev timestamp parsing stays inside the existing evdev adapter boundary.
- Lua Extension Contract: PASS. No Lua API or capability changes.
- NixOS Reproducibility: PASS. Verification uses the project flake commands and adds no dependency.
- Security and Consent: PASS. Metrics reuse existing explicit input observation consent and add no new data source.
- TDD and Testability: PASS. Parsing and stats calculations require automated tests before implementation.
- Minimal Composition: PASS. No daemon, async runtime, persistent store, or global registry is introduced.
- No Hidden Global Behavior: PASS. Metrics are runtime diagnostics only and do not enable input observation.
- Incremental Delivery: PASS. Timestamp preservation, event-age stats, and diagnostic rename/compatibility are separable.

## Project Structure

### Documentation (this feature)

```text
specs/013-true-input-latency-metrics/
├── spec.md
├── plan.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/signal-auras-wayland/src/
├── evdev.rs
└── adapter.rs

crates/signal-auras-core/src/
└── stats.rs

crates/signal-auras-cli/src/
└── runner.rs

tests/
├── contract/
└── integration/
```

**Structure Decision**: Keep raw timestamp parsing and availability state in the evdev adapter boundary, keep aggregate metric calculations in `signal-auras-core::stats`, and keep CLI changes limited to passing event timestamps into stats and rendering distinct diagnostic labels.

## Complexity Tracking

No constitution violations.
