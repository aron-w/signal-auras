# Tasks: Callback Wakeups

**Input**: Design documents from `/specs/009-callback-wakeups/`
**Prerequisites**: plan.md, research.md, data-model.md, contracts/callback-wakeup.md, quickstart.md

## Phase 1: Setup

- [x] T001 Verify callback wakeup scope against AGENTS.md, constitution, and specs/006-009 plans
- [x] T002 [P] Add focused task references for callback wakeups in specs/009-callback-wakeups/quickstart.md

## Phase 2: Foundational

- [x] T003 [P] Add callback latency/drop counters and summary fields in crates/signal-auras-core/src/stats.rs
- [x] T004 [P] Add pollable runtime wake fd tests and implementation in crates/signal-auras-wayland/src/event_loop.rs
- [x] T005 Add bounded KDE callback queue tests and implementation in crates/signal-auras-wayland/src/kde_bridge.rs
- [x] T006 Expose callback wake fd, drain, drop count, and observed shortcut event APIs through crates/signal-auras-wayland/src/adapter.rs

## Phase 3: User Story 1 - Low-Jitter Shortcut Callback Delivery (P1)

**Goal**: Callback shortcut events wake the live runner promptly and dispatch without unrelated input.

**Independent Test**: Simulated callback events can wake through a runtime fd and record callback-to-dispatch latency at or below target buckets.

- [x] T007 [P] [US1] Add callback latency histogram tests in crates/signal-auras-core/src/stats.rs
- [x] T008 [P] [US1] Add KDE callback wake fd readiness tests in crates/signal-auras-wayland/src/kde_bridge.rs
- [x] T009 [US1] Wire callback wake fd into run_live_real_lifecycle in crates/signal-auras-cli/src/runner.rs
- [x] T010 [US1] Record callback receipt, dispatch latency, and accepted dispatch diagnostics in crates/signal-auras-cli/src/runner.rs

## Phase 4: User Story 2 - Efficient Idle Operation (P2)

**Goal**: The live runner stays idle without continuous busy work while callbacks still wake promptly after long idle periods.

**Independent Test**: Idle timeout helpers return long waits when no macro/repeat work is pending, while callback fd readiness still wakes the loop.

- [x] T011 [P] [US2] Add idle timeout regression tests in crates/signal-auras-cli/src/runner.rs
- [x] T012 [US2] Replace fixed short idle polling with deadline-based waits in crates/signal-auras-cli/src/runner.rs
- [x] T013 [US2] Add unavailable callback support diagnostics coverage in tests/contract/cli_runner.rs

## Phase 5: User Story 3 - Coexist With Input, Repeat, and Shutdown Wakeups (P3)

**Goal**: Callback, input, repeat cancellation, and shutdown work share the live runner without starvation or post-shutdown starts.

**Independent Test**: Mixed scripted scenarios cover callback ordering, repeat cancellation, and shutdown no-start semantics.

- [x] T014 [P] [US3] Add mixed callback/input/repeat/shutdown lifecycle tests in tests/integration/runner_flow.rs
- [x] T015 [US3] Process callback, input, cancellation, macro queue, and shutdown wakeups in a bounded fair order in crates/signal-auras-cli/src/runner.rs
- [x] T016 [US3] Ensure callbacks received after shutdown begins follow the documented ignore policy in crates/signal-auras-cli/src/runner.rs

## Phase 6: Polish & Cross-Cutting

- [x] T017 [P] Update tests/compositor/manual-wayland-verification.md with callback wakeup manual checks
- [x] T018 Run cargo fmt, cargo clippy --all-targets -- -D warnings, cargo test, and nix flake check where feasible
- [x] T019 Review FR-001 through FR-012 against implementation and mark all tasks complete

## Dependencies

Setup (T001-T002) before foundational tasks. Foundational tasks (T003-T006) block all user stories. US1 (T007-T010) is MVP and should complete before US2/US3. US2 (T011-T013) and US3 (T014-T016) can proceed after US1 but touch runner behavior and must be integrated carefully. Polish follows all stories.

## Parallel Examples

- T003 and T004 can run in parallel because they touch different crates.
- T007 and T008 can run in parallel for US1.
- T011 and T014 can run in parallel after US1 if runner edits are coordinated.

## Implementation Strategy

Implement the smallest library-backed increment first: stats counters, wake fd,
bounded queue, and live callback dispatch. Then harden idle waiting and mixed
wakeup ordering. Preserve existing Lua behavior and current-run consent gates
throughout.
