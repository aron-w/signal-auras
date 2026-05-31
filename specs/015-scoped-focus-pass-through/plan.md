# Implementation Plan: Scoped Focus Pass-Through

**Branch**: `015-scoped-focus-pass-through` | **Date**: 2026-05-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/015-scoped-focus-pass-through/spec.md`

## Summary

Make process-scoped automation inactive outside trusted focused-process scope. While inactive, scoped callbacks, composite triggers, motion sequences, repeat ticks, queued output, and input-grab prevention must not affect the focused application. The implementation extends existing Rust scope/freshness decisions into an explicit scoped-focus state, uses that state before scheduling or consuming scoped work, cancels scoped pending work on deactivation, and emits privacy-bounded info logs on active/inactive transitions.

## Technical Context

**Language/Version**: Rust stable workspace from the project flake; Lua 5.4-compatible script surface remains unchanged.

**Primary Dependencies**: Existing workspace crates and existing runtime dependencies (`mio`, `nix`, `udev`, `tracing`, `tracing-subscriber`, `libc`, `zbus`). No new dependency is required.

**Storage**: Repository files only. No persistent focus state, daemon state, cache, IPC state, or observed-input log.

**Testing**: `nix develop -c cargo fmt --check`, `nix develop -c cargo clippy --all-targets -- -D warnings`, `nix develop -c cargo test`, and `nix flake check` when feasible. Automated tests cover scoped-focus state decisions, inactive pass-through/no-processing, active resumption, repeat and queue cancellation, stale/unavailable/denied/ambiguous/untrusted metadata, transition logs, and Lua compatibility.

**Target Platform**: NixOS/Linux/KDE Plasma Wayland with existing explicit current-run consent for process metadata, evdev observation/grab, macro execution, and synthesized input.

**Project Type**: Rust workspace with core automation library, Wayland/KDE adapters, Lua validation crate, CLI runner, and integration/contract tests.

**Performance Goals**: Scoped-focus checks are in-memory decisions over the current focus snapshot. No blocking compositor query is added to the hot path beyond existing active-process provider calls.

**Constraints**: Preserve existing Lua scope, hotkey, and motion syntax. Explicit global scope remains unaffected. Process-scoped automation fails inactive for stale, unavailable, denied, ambiguous, missing, untrusted, or non-matching focus. Inactive paths must not consume, prevent, or schedule scoped automation, and logs must avoid command-line arguments, window titles, text payloads, and unrelated process data.

**Scale/Scope**: One terminal-started runner process, one current focus snapshot, process-scoped hotkeys/motions/callbacks/repeats, and current-run evdev/uinput/portal runtime paths.

## Constitution Check

*GATE: Passed before Phase 0 research. Re-checked after Phase 1 design: Passed.*

- Library-First: PASS. Scoped focus state and transition classification are Rust library behavior before runner integration.
- Wayland/Compositor Awareness: PASS. KDE Wayland remains the active metadata target; unsupported or untrusted metadata fails inactive/closed.
- Rust Safety Boundaries: PASS. Input grabbing, pass-through, active-process metadata, and synthesized output remain behind existing Wayland adapter boundaries.
- Lua Extension Contract: PASS. No Lua API or capability syntax changes are introduced.
- NixOS Reproducibility: PASS. Verification uses the existing project flake commands and no new dependency is added.
- Security and Consent: PASS. Existing current-run process inspection, input observation/prevention, macro execution, and synthesized input consent boundaries are preserved.
- TDD and Testability: PASS. Core scoped-focus tests and runner integration tests are required before implementation.
- Minimal Composition: PASS. No daemon, IPC, async runtime, persistent queue, global registry, or state store is introduced.
- No Hidden Global Behavior: PASS. Automation remains explicitly configured and current-run only.
- Incremental Delivery: PASS. P1 inactive pass-through, P2 active resumption, and P3 transition logs are independently testable.

## Project Structure

### Documentation (this feature)

```text
specs/015-scoped-focus-pass-through/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── scoped-focus-runtime.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/signal-auras-core/src/
├── scope.rs
├── stats.rs
└── lib.rs

crates/signal-auras-cli/src/
└── runner.rs

crates/signal-auras-wayland/src/
├── adapter.rs
├── evdev.rs
└── uinput.rs

tests/contract/
├── rust_library.rs
├── cli_runner.rs
└── lua_api.rs

tests/integration/
└── runner_flow.rs

tests/compositor/
└── manual-wayland-verification.md
```

**Structure Decision**: Keep scoped-focus state and privacy-bounded transition fields in `signal-auras-core::scope`, keep event scheduling/pass-through/cancellation in `signal-auras-cli::runner`, and keep raw input pass-through plus grab release behavior inside the existing `signal-auras-wayland` adapter boundary.

## Complexity Tracking

No constitution violations.
