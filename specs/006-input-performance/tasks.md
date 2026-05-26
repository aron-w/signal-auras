# Tasks: Input Motion Performance and Consistency

**Input**: Design documents from `/specs/006-input-performance/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Verification**: TDD is mandatory. Every user story includes automated tests before implementation for Rust behavior.

## Phase 1: Setup

- [x] T001 Update `.specify/feature.json` to point at `specs/006-input-performance`
- [x] T002 Update `AGENTS.md` Spec Kit context to `specs/006-input-performance/plan.md`

## Phase 2: Foundational

- [x] T003 [P] Add latency and repeat diagnostic counters in `crates/signal-auras-core/src/stats.rs`
- [x] T004 [P] Add provider event metadata types in `crates/signal-auras-wayland/src/evdev.rs`

## Phase 3: User Story 1 - Low-Latency Mixed Input Observation (Priority: P1)

**Goal**: Supported events from mixed keyboard and pointer devices dispatch fairly and without fixed polling delay.

**Independent Test**: Simulated evdev tests dispatch mixed events within target latency and without starvation.

### Verification for User Story 1

- [x] T005 [P] [US1] Add evdev fair-read and readiness tests in `crates/signal-auras-wayland/src/evdev.rs`
- [x] T006 [P] [US1] Add live-loop latency accounting tests in `tests/integration/runner_flow.rs`

### Implementation for User Story 1

- [x] T007 [US1] Implement readiness wait and bounded fair draining in `crates/signal-auras-wayland/src/evdev.rs`
- [x] T008 [US1] Use provider wait deadlines instead of fixed input sleeps in `crates/signal-auras-cli/src/runner.rs`
- [x] T009 [US1] Emit verbose input dispatch latency diagnostics in `crates/signal-auras-cli/src/runner.rs`

## Phase 4: User Story 2 - Reliable Repeat Cancellation (Priority: P2)

**Goal**: Repeats stop before another repeat macro once a cancellation release has been processed.

**Independent Test**: Simulated repeat release/tick race produces no post-cancel repeat macro.

### Verification for User Story 2

- [x] T010 [P] [US2] Add repeat cancellation race tests in `tests/integration/runner_flow.rs`
- [x] T011 [P] [US2] Add motion runtime idempotent cancellation tests in `crates/signal-auras-core/src/motion.rs`

### Implementation for User Story 2

- [x] T012 [US2] Prioritize input release processing before repeat ticks in `crates/signal-auras-cli/src/runner.rs`
- [x] T013 [US2] Add repeat cancel/tick counters and verbose lifecycle logging in `crates/signal-auras-core/src/stats.rs` and `crates/signal-auras-cli/src/runner.rs`

## Phase 5: User Story 3 - Device Hotplug and Diagnosable Operation (Priority: P3)

**Goal**: `devices = "all"` rescans current-run event devices and logs additions/removals/skips.

**Independent Test**: Simulated discovery changes add and remove devices without restarting the provider.

### Verification for User Story 3

- [x] T014 [P] [US3] Add all-device rescan tests in `crates/signal-auras-wayland/src/evdev.rs`
- [x] T015 [P] [US3] Add unsafe consent regression tests in `tests/contract/lua_api.rs`

### Implementation for User Story 3

- [x] T016 [US3] Implement `devices = "all"` runtime rescan in `crates/signal-auras-wayland/src/evdev.rs`
- [x] T017 [US3] Surface provider rescan summaries through `crates/signal-auras-wayland/src/adapter.rs`
- [x] T018 [US3] Document diagnostics and manual smoke steps in `README.md`

## Phase 6: Polish

- [x] T019 Run `nix develop -c cargo fmt --check`
- [x] T020 Run `nix develop -c cargo clippy --all-targets -- -D warnings`
- [x] T021 Run `nix develop -c cargo test`
- [x] T022 Run `nix flake check` when available
- [x] T023 Add latency percentile metrics and summary assertions in `crates/signal-auras-core/src/stats.rs`
- [x] T024 Add multi-device input scalability stress test in `crates/signal-auras-wayland/src/evdev.rs`

## Dependencies & Execution Order

- Setup tasks must complete before implementation.
- Foundational tasks unblock all stories.
- US1 should land before US2 and US3 because it establishes provider wait behavior.
- US2 and US3 can be implemented after US1.
- Polish follows all stories.

## Implementation Strategy

Implement the smallest reliability slice first: provider readiness/fairness and runner wait deadlines. Then harden repeat cancellation ordering. Finish with `devices = "all"` rescan diagnostics and full verification.
