# Tasks: True Input Latency Metrics

**Input**: Design documents from `/specs/013-true-input-latency-metrics/`

**Prerequisites**: plan.md, spec.md

**Verification**: TDD is mandatory for timestamp parsing, event-age metric calculation, diagnostic labels, and dispatch metric compatibility.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

## Phase 1: Setup

- [ ] T001 Verify existing evdev raw event parsing, observed input event structs, runtime stats, and runner latency recording in `crates/signal-auras-wayland/src/evdev.rs`, `crates/signal-auras-core/src/stats.rs`, and `crates/signal-auras-cli/src/runner.rs`
- [ ] T002 [P] Confirm no new dependencies are needed in `Cargo.toml` and `flake.nix`

## Phase 2: Foundational

- [ ] T003 Add timestamp availability model tests in `crates/signal-auras-wayland/src/evdev.rs`
- [ ] T004 Implement timestamp availability model for decoded observed events in `crates/signal-auras-wayland/src/evdev.rs`
- [ ] T005 Add true event-age counter and histogram tests in `crates/signal-auras-core/src/stats.rs`
- [ ] T006 Implement true event-age stats fields and summary rendering in `crates/signal-auras-core/src/stats.rs`

## Phase 3: User Story 1 - Preserve Kernel Event Time (P1)

**Goal**: Decoded evdev keyboard, pointer button, and wheel events preserve valid kernel timestamps and userspace observation times.

**Independent Test**: Simulated raw evdev events retain kernel timestamps on observed input events.

- [ ] T007 [P] [US1] Add keyboard timestamp parsing tests in `crates/signal-auras-wayland/src/evdev.rs`
- [ ] T008 [P] [US1] Add pointer button and wheel timestamp parsing tests in `crates/signal-auras-wayland/src/evdev.rs`
- [ ] T009 [US1] Preserve kernel timestamps on `ObservedInputEvent` and `ObservedMotionInputEvent` in `crates/signal-auras-wayland/src/evdev.rs`
- [ ] T010 [US1] Preserve userspace observation/read timestamps for existing dispatch metrics in `crates/signal-auras-wayland/src/evdev.rs`

## Phase 4: User Story 2 - Report True Event Age and Backlog (P2)

**Goal**: Stats report true kernel-event-to-action age where timestamps are comparable and count unavailable samples explicitly.

**Independent Test**: Controlled timestamp tests produce expected average, p95, p99, max, and unavailable-sample values.

- [ ] T011 [P] [US2] Add true event-age metric calculation tests in `crates/signal-auras-core/src/stats.rs`
- [ ] T012 [P] [US2] Add backlog scenario tests in `tests/integration/runner_flow.rs`
- [ ] T013 [US2] Record kernel-event-to-action age when dispatching observed input in `crates/signal-auras-cli/src/runner.rs`
- [ ] T014 [US2] Count unavailable or incomparable kernel timestamps without corrupting summaries in `crates/signal-auras-core/src/stats.rs`

## Phase 5: User Story 3 - Keep Existing Dispatch Metrics Understandable (P3)

**Goal**: Current dispatch-after-read metrics remain available or are clearly renamed and rendered separately from true event age.

**Independent Test**: Final summaries and verbose diagnostics include distinct labels for both metric families.

- [ ] T015 [P] [US3] Add dispatch metric compatibility and label tests in `crates/signal-auras-core/src/stats.rs`
- [ ] T016 [P] [US3] Add verbose diagnostic label tests in `crates/signal-auras-cli/src/runner.rs`
- [ ] T017 [US3] Rename or preserve current dispatch latency fields with explicit dispatch-after-read labels in `crates/signal-auras-core/src/stats.rs`
- [ ] T018 [US3] Update runner verbose diagnostics to distinguish true event age from dispatch-after-read latency in `crates/signal-auras-cli/src/runner.rs`

## Phase 6: Polish and Verification

- [ ] T019 Update README and manual verification notes for true input latency metrics in `README.md` and `tests/compositor/manual-wayland-verification.md`
- [ ] T020 Run `cargo fmt --check`
- [ ] T021 Run `cargo clippy --all-targets -- -D warnings`
- [ ] T022 Run `cargo test`
- [ ] T023 Run `nix flake check` when feasible

## Dependencies and Order

- Setup and foundational timestamp/stat models block user stories.
- US1 timestamp preservation blocks US2 event-age reporting.
- US3 can start after foundational stats labels exist but must be reconciled after US2.
- Verification tasks precede implementation tasks within each story.

## Parallel Opportunities

- T002 can run independently after T001.
- T007 and T008 can be written in parallel.
- T011, T012, T015, and T016 touch separate files and can be drafted independently after foundation.

## Implementation Strategy

Deliver timestamp preservation first, then true event-age stats, then diagnostic labeling compatibility. Keep the feature metrics-only and do not change Lua or matching semantics.
