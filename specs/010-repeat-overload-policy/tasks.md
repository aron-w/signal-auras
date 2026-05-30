# Tasks: Repeat Overload Policy

**Input**: Design documents from `/specs/010-repeat-overload-policy/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Verification**: TDD is mandatory. Add or update failing automated tests before implementation for core repeat scheduling, cancellation, diagnostics, and Lua compatibility behavior.

**Organization**: Tasks are grouped by user story so each story can be implemented and tested independently in priority order.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Confirm the existing Rust workspace and ignore files already cover this feature.

- [x] T001 Verify repository ignore files cover Rust/Nix generated artifacts in .gitignore
- [x] T002 Verify existing runtime modules targeted by the plan in crates/signal-auras-core/src/stats.rs and crates/signal-auras-cli/src/runner.rs

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Establish shared repeat overload counters and helper semantics before story-specific behavior.

- [x] T003 Add repeat overload counter fields and summary rendering tests in crates/signal-auras-core/src/stats.rs
- [x] T004 Implement repeat overload counter recording and final summary fields in crates/signal-auras-core/src/stats.rs
- [x] T005 Add helper tests for repeat macro queue identity and active/pending detection in crates/signal-auras-cli/src/runner.rs
- [x] T006 Implement repeat macro queue identity helpers in crates/signal-auras-cli/src/runner.rs

**Checkpoint**: Foundation ready - user story implementation can begin.

---

## Phase 3: User Story 1 - Keep Held Repeats Stable Under Overload (Priority: P1) MVP

**Goal**: Slow repeat output never creates overlapping output or unbounded pending work for the same held repeat.

**Independent Test**: Simulate a held repeat with output slower than its interval for at least 10,000 due opportunities and verify one active/pending repeat macro at most, skipped/coalesced counts increase, and the runner can still schedule later work.

### Verification for User Story 1

- [x] T007 [US1] Add slow-output overload and 10,000-tick bounded queue tests in crates/signal-auras-cli/src/runner.rs

### Implementation for User Story 1

- [x] T008 [US1] Implement skip/coalesce scheduling for overloaded repeat ticks in crates/signal-auras-cli/src/runner.rs
- [x] T009 [US1] Record executed and skipped/coalesced repeat tick counters in crates/signal-auras-cli/src/runner.rs

**Checkpoint**: User Story 1 is independently testable.

---

## Phase 4: User Story 2 - Prioritize Release and Cancellation (Priority: P2)

**Goal**: A processed cancellation release prevents all later repeat output for that held repeat, including due ticks skipped during overload.

**Independent Test**: Simulate overloaded repeat output, process release/cancel events before due ticks, and verify zero later repeat macro output starts while already-started output may finish.

### Verification for User Story 2

- [x] T010 [US2] Add cancellation race and shutdown-during-overload tests in crates/signal-auras-cli/src/runner.rs

### Implementation for User Story 2

- [x] T011 [US2] Enforce cancellation-before-repeat scheduling for overloaded repeat ticks in crates/signal-auras-cli/src/runner.rs
- [x] T012 [US2] Ensure queued repeat cancellation updates cancellation counters without replaying skipped work in crates/signal-auras-cli/src/runner.rs

**Checkpoint**: User Stories 1 and 2 both work independently.

---

## Phase 5: User Story 3 - Diagnose Dropped or Coalesced Repeats (Priority: P3)

**Goal**: Verbose diagnostics and final summaries explain executed, skipped/coalesced, and cancelled repeat behavior without noisy per-tick output or private payloads.

**Independent Test**: Run normal, overloaded, cancelled, and quiet-mode scenarios and assert diagnostics/counters are accurate and payload-safe.

### Verification for User Story 3

- [x] T013 [US3] Add repeat overload diagnostic rendering tests in crates/signal-auras-core/src/stats.rs
- [x] T014 [US3] Add verbose repeat overload log tests in crates/signal-auras-cli/src/runner.rs

### Implementation for User Story 3

- [x] T015 [US3] Emit bounded verbose repeat overload diagnostics from crates/signal-auras-cli/src/runner.rs
- [x] T016 [US3] Update README repeat runtime diagnostics in README.md

**Checkpoint**: All user stories are independently functional.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Preserve compatibility and run repository verification.

- [x] T017 Add Lua repeat configuration regression coverage in tests/contract/lua_api.rs
- [x] T018 Run cargo fmt --check and fix formatting issues
- [x] T019 Run cargo clippy --all-targets -- -D warnings and fix findings
- [x] T020 Run cargo test and fix failures
- [x] T021 Run nix flake check when feasible and document limitations

## Phase 7: Architecture Review Follow-Up - Non-Repeat Trigger Collisions

**Goal**: Repeated or mashed input for an already-active non-repeat trigger follows a deterministic bounded policy, keeps the always-on runner live, records skipped/denied diagnostics, and cleans up active trigger state after completion or cancellation.

**Independent Test**: Start a non-repeat macro, trigger the same binding again before completion, verify the collision is skipped/coalesced/denied without returning a fatal runner error, then complete or cancel the macro and verify a later legitimate trigger can run.

- [ ] T022 [P] [US4] Add non-repeat already-active trigger collision tests in `crates/signal-auras-cli/src/runner.rs`
- [ ] T023 [P] [US4] Add stats rendering tests for denied/skipped non-repeat collisions in `crates/signal-auras-core/src/stats.rs`
- [ ] T024 [US4] Implement deterministic non-repeat collision policy in `crates/signal-auras-cli/src/runner.rs`
- [ ] T025 [US4] Record denied/skipped non-repeat collision stats and bounded diagnostics in `crates/signal-auras-cli/src/runner.rs`
- [ ] T026 [US4] Ensure active trigger state cleanup after macro completion, cancellation, denial, and shutdown in `crates/signal-auras-cli/src/runner.rs`
- [ ] T027 [US4] Update README runtime diagnostics for non-repeat trigger overload in `README.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies.
- **Foundational (Phase 2)**: Depends on Setup completion and blocks user stories.
- **User Stories**: Implement in priority order P1 -> P2 -> P3 for this single-agent pass.
- **Polish**: Depends on all user stories.

### User Story Dependencies

- **User Story 1 (P1)**: Starts after Foundation and delivers bounded overload behavior.
- **User Story 2 (P2)**: Builds on US1 scheduling to enforce processed-release priority.
- **User Story 3 (P3)**: Builds on US1/US2 counters and decisions for diagnostics.

### Parallel Opportunities

- T001 and T002 can be checked independently.
- T003/T004 affect stats.rs and must run sequentially.
- T005/T006 affect runner.rs and must run sequentially.
- Story tests must be written before their implementation tasks.

---

## Implementation Strategy

### MVP First

1. Complete setup and foundational counter/queue helpers.
2. Implement US1 bounded skip/coalesce behavior and verify slow-output stress tests.
3. Continue with cancellation safety in US2.
4. Add diagnostics and documentation in US3.
5. Run formatting, linting, tests, and Nix checks where feasible.
