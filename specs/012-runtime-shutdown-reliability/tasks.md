# Tasks: Runtime Shutdown Reliability

**Input**: Design documents from `/specs/012-runtime-shutdown-reliability/`

**Prerequisites**: plan.md, spec.md

**Verification**: TDD is mandatory for signal routing, helper-thread mask ordering, shutdown wakeups, and cleanup behavior. Manual KDE verification is supplemental for live compositor cleanup.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

## Phase 1: Setup

- [x] T001 Verify existing signal, wake-fd, runtime lifecycle, KDE bridge, evdev, and uinput cleanup paths in `crates/signal-auras-wayland/src/event_loop.rs`, `kde_bridge.rs`, `evdev.rs`, `uinput.rs`, and `crates/signal-auras-cli/src/runner.rs`
- [x] T002 [P] Confirm existing Nix verification commands and no new dependencies in `flake.nix`

## Phase 2: Foundational

- [x] T003 Add runtime signal guard tests for SIGINT/SIGTERM mask setup in `crates/signal-auras-wayland/src/event_loop.rs`
- [x] T004 Implement a reusable runtime signal guard that blocks SIGINT/SIGTERM before signal fd creation in `crates/signal-auras-wayland/src/event_loop.rs`
- [x] T005 Add helper-thread signal-mask inheritance test hooks in `crates/signal-auras-wayland/src/event_loop.rs`

## Phase 3: User Story 1 - Route Terminal Signals Through Cleanup (P1)

**Goal**: SIGINT and SIGTERM enter the runtime shutdown path and run cleanup exactly once.

**Independent Test**: Simulated SIGINT/SIGTERM each produce a shutdown reason and cleanup report without abrupt termination.

- [x] T006 [P] [US1] Add SIGINT/SIGTERM routing tests in `crates/signal-auras-wayland/src/event_loop.rs`
- [x] T007 [US1] Wire SIGINT/SIGTERM runtime signal fd handling into `crates/signal-auras-cli/src/runner.rs`
- [x] T008 [US1] Ensure duplicate shutdown signals keep cleanup idempotent in `crates/signal-auras-wayland/src/event_loop.rs`

## Phase 4: User Story 2 - Keep Helper Threads From Receiving Default Terminating Signals (P2)

**Goal**: Listener/helper threads inherit blocked SIGINT/SIGTERM masks before they start.

**Independent Test**: Thread startup tests verify helper threads cannot observe unblocked default SIGINT/SIGTERM before the runtime signal fd is ready.

- [x] T009 [P] [US2] Add helper-thread mask ordering tests in `crates/signal-auras-wayland/src/event_loop.rs`
- [x] T010 [US2] Move listener/helper thread startup after runtime signal guard setup in `crates/signal-auras-cli/src/runner.rs`
- [x] T011 [US2] Document startup failure unwind behavior for signal masks in `crates/signal-auras-wayland/src/event_loop.rs`

## Phase 5: User Story 3 - Wake and Release Promptly on Shutdown (P3)

**Goal**: Shutdown wakes idle waits promptly and releases current-run virtual input and grab resources.

**Independent Test**: Idle wait tests wake on shutdown, no-new-work tests pass, and cleanup reports release resources.

- [x] T012 [P] [US3] Add idle shutdown wake tests in `crates/signal-auras-wayland/src/event_loop.rs`
- [x] T013 [P] [US3] Add current-run input session cleanup tests in `crates/signal-auras-wayland/src/adapter.rs`
- [x] T014 [US3] Ensure shutdown signal fd wakes the runtime loop in `crates/signal-auras-wayland/src/event_loop.rs` and `crates/signal-auras-cli/src/runner.rs`
- [x] T015 [US3] Ensure evdev grabs, uinput/portal devices, KDE bridge scripts, callbacks, and registrations are released or reported during cleanup in `crates/signal-auras-wayland/src/` and `crates/signal-auras-cli/src/runner.rs`
- [x] T016 [US3] Prevent new macro scheduling after shutdown starts in `crates/signal-auras-cli/src/runner.rs`

## Phase 6: Polish and Verification

- [x] T017 Update shutdown manual verification notes in `tests/compositor/manual-wayland-verification.md`
- [x] T018 Run `cargo fmt --check`
- [x] T019 Run `cargo clippy --all-targets -- -D warnings`
- [x] T020 Run `cargo test`
- [x] T021 Run `nix flake check` when feasible

## Dependencies and Order

- Setup and foundational signal guard work block all user stories.
- Implement US1 before US2 and US3 because signal routing establishes the shutdown source.
- US2 and US3 can proceed independently after the signal guard is in place.
- Verification tasks must precede implementation tasks in each story.

## Parallel Opportunities

- T002 can run independently after T001.
- T006, T009, T012, and T013 touch different files and can be drafted independently.
- Polish documentation can run after behavior is implemented.

## Implementation Strategy

Deliver SIGINT/SIGTERM routing first, then thread mask ordering, then prompt wakeup and resource cleanup. Keep scope limited to the current runner and current-run resources.
