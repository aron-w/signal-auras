# Tasks: Runner Architecture Decomposition

**Input**: Design documents from `/specs/019-runner-decomposition/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/runner-boundaries.md

**Verification**: TDD is mandatory. Lifecycle cleanup guarantees, callback responsiveness, and focus policy unification tests must pass before structural movement begins.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

## Phase 1: Setup

- [X] T001 Verify lifecycle cleanup, callback responsiveness, and focus policy unification behavior specs are implemented or explicitly out of scope before modifying `crates/signal-auras-cli/src/runner.rs`
- [X] T002 [P] Capture current `runner.rs` public behavior and clippy findings in `specs/019-runner-decomposition/research.md`

## Phase 2: Foundational

- [X] T003 Add or confirm regression tests for startup failure cleanup, normal shutdown cleanup, callback wakeups, Lua callback budgets, and focus policy decisions in `tests/contract/` and integration tests
- [X] T004 Define module skeletons for lifecycle, runtime loop, controller execution, and diagnostics in `crates/signal-auras-cli/src/runner/`
- [X] T005 Keep `crates/signal-auras-cli/src/runner.rs` as the compatibility entry point while extracted modules are introduced

## Phase 3: User Story 1 - Reuse Lifecycle Inputs Without Long Argument Lists (P1)

**Goal**: Lifecycle startup and cleanup use named configuration/session ownership.

**Independent Test**: Startup failure cleanup and normal shutdown cleanup tests pass before and after lifecycle extraction.

- [X] T006 [P] [US1] Add lifecycle config/session tests in `crates/signal-auras-cli/src/runner/lifecycle.rs`
- [X] T007 [US1] Extract lifecycle configuration inputs into `crates/signal-auras-cli/src/runner/lifecycle.rs`
- [X] T008 [US1] Extract current-run resource ownership and idempotent cleanup session behavior into `crates/signal-auras-cli/src/runner/lifecycle.rs`
- [X] T009 [US1] Replace clippy-reported live lifecycle function argument lists in `crates/signal-auras-cli/src/runner.rs` with named argument structs

## Phase 4: User Story 2 - Isolate Runtime Loop Coordination (P2)

**Goal**: Wake source ordering and no-new-work-after-shutdown behavior live in a focused coordinator.

**Independent Test**: Callback, input, repeat, focus pass-through, and shutdown wake tests pass before and after extraction.

- [X] T010 [P] [US2] Add runtime loop coordinator tests in `crates/signal-auras-cli/src/runner/runtime_loop.rs`
- [ ] T011 [US2] Extract input/callback/timer/hotplug/repeat/shutdown coordination into `crates/signal-auras-cli/src/runner/runtime_loop.rs`
- [ ] T012 [US2] Preserve runtime diagnostics fields while moving loop coordination out of `crates/signal-auras-cli/src/runner.rs`

## Phase 5: User Story 3 - Separate Controller Execution From CLI Orchestration (P3)

**Goal**: Lua controller execution is reusable and independently testable.

**Independent Test**: Lua controller runtime tests and CLI runner tests cover the same controller execution dispositions.

- [ ] T013 [P] [US3] Add controller execution boundary tests in `crates/signal-auras-cli/src/runner/controller.rs`
- [ ] T014 [US3] Extract Lua controller execution wiring into `crates/signal-auras-cli/src/runner/controller.rs`
- [ ] T015 [US3] Keep capability denial, sleep/yield, budgets, and diagnostics behavior unchanged through extracted boundary

## Phase 6: Polish and Verification

- [X] T016 [P] Update `AGENTS.md` and related spec references only if the runner decomposition plan remains active
- [X] T017 Run `cargo fmt --check`
- [X] T018 Run `cargo test`
- [X] T019 Run `cargo clippy --all-targets -- -D warnings`
- [X] T020 Run Nix verification commands when feasible

## Dependencies and Order

- Lifecycle cleanup, callback responsiveness, and focus policy behavior work blocks this feature.
- US1 should complete before US2 and US3 because session ownership is shared by loop coordination and controller execution.
- US2 and US3 can proceed independently after US1.

## Parallel Opportunities

- T002 can run in parallel with T001.
- T006, T010, and T013 are independent tests once foundational module skeletons exist.
- Diagnostics documentation can be updated in parallel with module extraction after behavior tests pass.

## Implementation Strategy

Deliver lifecycle extraction first, then runtime-loop coordination, then controller execution. Stop after each boundary and run focused tests plus clippy before continuing.
