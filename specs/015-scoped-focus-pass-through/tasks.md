# Tasks: Scoped Focus Pass-Through

**Input**: Design documents from `/specs/015-scoped-focus-pass-through/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Verification**: TDD is mandatory. Every user story includes failing tests before implementation for Rust library and runner behavior.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

## Phase 1: Setup

**Purpose**: Confirm existing project hygiene before code changes.

- [X] T001 Verify `.gitignore` covers Rust/Nix generated files in `.gitignore`

---

## Phase 2: Foundational

**Purpose**: Add shared scoped-focus state primitives used by all stories.

- [X] T002 [P] Add scoped focus state unit tests in crates/signal-auras-core/src/scope.rs
- [X] T003 Implement `ScopedFocusState` and transition helpers in crates/signal-auras-core/src/scope.rs
- [X] T004 Export scoped focus primitives from crates/signal-auras-core/src/lib.rs

**Checkpoint**: Core state model ready for runner integration.

---

## Phase 3: User Story 1 - Pass Through Outside Scoped Focus (Priority: P1) MVP

**Goal**: Process-scoped automation is inactive outside matching focus and does not process or prevent original input.

**Independent Test**: Simulate non-matching focus for scoped hotkeys, composite triggers, motions, repeat ticks, and grabbed observed input; verify no macro output, no consumed/prevented input, and pass-through reporting.

### Verification for User Story 1

- [X] T005 [P] [US1] Add inactive hotkey/composite no-consumption tests in tests/integration/runner_flow.rs
- [X] T006 [P] [US1] Add inactive motion/repeat no-processing tests in tests/integration/runner_flow.rs
- [X] T007 [P] [US1] Add inactive grabbed input pass-through test in tests/integration/runner_flow.rs

### Implementation for User Story 1

- [X] T008 [US1] Gate scoped trigger scheduling before consumption counters in crates/signal-auras-cli/src/runner.rs
- [X] T009 [US1] Gate scoped motion trigger and repeat scheduling while inactive in crates/signal-auras-cli/src/runner.rs
- [X] T010 [US1] Pass through grabbed observed input when scoped focus is inactive in crates/signal-auras-cli/src/runner.rs
- [X] T011 [US1] Add fail-closed diagnostics for providers that cannot guarantee inactive pass-through in crates/signal-auras-cli/src/runner.rs

**Checkpoint**: User Story 1 is fully functional and testable independently.

---

## Phase 4: User Story 2 - Activate Only When Focus Matches (Priority: P2)

**Goal**: Scoped automation resumes only after fresh trusted metadata matches the configured process rule.

**Independent Test**: Simulate focus moving from non-matching/untrusted to fresh matching metadata, then verify subsequent scoped trigger input works under existing consent rules.

### Verification for User Story 2

- [X] T012 [P] [US2] Add active resumption and inactive metadata denial tests in tests/integration/runner_flow.rs
- [X] T013 [P] [US2] Add scoped queued macro/repeat cancellation tests in tests/integration/runner_flow.rs

### Implementation for User Story 2

- [X] T014 [US2] Track scoped focus transitions during lifecycle events in crates/signal-auras-cli/src/runner.rs
- [X] T015 [US2] Cancel process-scoped repeat state and queued macro output on deactivation in crates/signal-auras-cli/src/runner.rs
- [X] T016 [US2] Preserve explicit global behavior when process focus changes in crates/signal-auras-cli/src/runner.rs

**Checkpoint**: User Stories 1 and 2 both work independently.

---

## Phase 5: User Story 3 - Log Focus Activation State Changes (Priority: P3)

**Goal**: Emit privacy-bounded info logs once per active/inactive transition.

**Independent Test**: Simulate matching and non-matching focus transitions and verify exactly one info-level activation/deactivation log per state change.

### Verification for User Story 3

- [X] T017 [P] [US3] Add scoped focus transition log formatting tests in crates/signal-auras-cli/src/runner.rs
- [X] T018 [P] [US3] Add privacy-bounded transition field tests in crates/signal-auras-core/src/scope.rs

### Implementation for User Story 3

- [X] T019 [US3] Emit transition-only info logs from scoped focus tracker in crates/signal-auras-cli/src/runner.rs
- [X] T020 [US3] Ensure transition logs omit private process/window/macro payloads in crates/signal-auras-core/src/scope.rs

**Checkpoint**: All user stories are independently functional.

---

## Phase 6: Polish & Cross-Cutting

- [X] T021 [P] Verify Lua compatibility for existing scope and motion syntax in tests/contract/lua_api.rs
- [X] T022 [P] Update manual compositor verification notes in tests/compositor/manual-wayland-verification.md
- [X] T023 Run `nix develop -c cargo fmt --check`
- [X] T024 Run `nix develop -c cargo clippy --all-targets -- -D warnings`
- [X] T025 Run `nix develop -c cargo test`
- [X] T026 Run `nix flake check`

## Phase 7: Architecture Review Follow-Up - Shared Freshness Policy

**Goal**: Scoped focus pass-through uses the same core freshness threshold as all other focus-gated runtime paths.

**Independent Test**: Runner stale-motion regression coverage proves scoped press/motion decisions deny beyond the core 2-second threshold and no longer use a CLI-local 30-second window.

- [X] T027 [P] [US2] Add shared freshness policy regression coverage in `crates/signal-auras-cli/src/runner.rs`
- [X] T028 [US2] Route scoped focus tracker, live press, motion trigger, and repeat decisions through the core policy path in `crates/signal-auras-cli/src/runner.rs`

---

## Dependencies & Execution Order

- Phase 1 before all code changes.
- Phase 2 before all user stories.
- User Story 1 is the MVP and must complete before User Story 2.
- User Story 2 must complete before User Story 3.
- Polish runs after all user stories.

## Parallel Examples

- T002 can run independently of setup verification.
- T005, T006, and T007 can be drafted together because they cover separate runner scenarios in the same test file but must be merged carefully.
- T012 and T013 can be drafted together after User Story 1.
- T017 and T018 can be drafted together because they touch different crates.

## Implementation Strategy

1. Deliver core scoped-focus state first.
2. Deliver P1 pass-through/no-processing as the MVP.
3. Add resumption and cancellation.
4. Add transition logging.
5. Run the full Rust and Nix verification path.
