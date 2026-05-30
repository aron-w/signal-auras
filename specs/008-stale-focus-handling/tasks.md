# Tasks: Stale Focus Handling

**Input**: Design documents from `/specs/008-stale-focus-handling/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Verification**: TDD is mandatory for the core freshness and runner no-emission behavior. Use automated tests for all specified scenarios; manual KDE verification is supplemental only.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

## Phase 1: Setup (Shared Infrastructure)

- [x] T001 Confirm local `main` is integrated into branch `008-stale-focus-handling`
- [x] T002 [P] Verify `.gitignore` already covers Rust/Nix generated files

---

## Phase 2: Foundational (Blocking Prerequisites)

- [x] T003 Add core focus freshness policy, denial kind, and privacy-bounded diagnostic data in `crates/signal-auras-core/src/scope.rs`
- [x] T004 Re-export the new core focus freshness and denial types from `crates/signal-auras-core/src/lib.rs`

---

## Phase 3: User Story 1 - Prevent Wrong-Process Macro Execution (Priority: P1) MVP

**Goal**: Process-scoped bindings allow fresh matching metadata and deny stale metadata before macro scheduling or input emission.

**Independent Test**: A fresh matching context allows a binding, while the same context older than 2 seconds is denied and leaves executor action count unchanged.

### Verification for User Story 1

- [x] T005 [P] [US1] Add freshness boundary tests below, at, and above the default 2 second threshold in `tests/contract/rust_library.rs`
- [x] T006 [P] [US1] Add runner no-emission test for stale process metadata in `tests/integration/runner_flow.rs`

### Implementation for User Story 1

- [x] T007 [US1] Implement freshness-aware scope evaluation in `crates/signal-auras-core/src/scope.rs`
- [x] T008 [US1] Update trigger and motion denial handling to use freshness-aware decisions in `crates/signal-auras-cli/src/runner.rs`

---

## Phase 4: User Story 2 - Handle Lost or Delayed Focus Metadata (Priority: P2)

**Goal**: Unavailable, delayed, denied, or recovered focus metadata is handled conservatively and recovers on the next fresh matching trigger.

**Independent Test**: Unavailable and permission-denied metadata deny process-scoped bindings, then a later fresh matching context allows the same binding without restart.

### Verification for User Story 2

- [x] T009 [P] [US2] Add unavailable, permission-denied, untrusted timestamp, and recovery tests in `tests/contract/rust_library.rs`
- [x] T010 [P] [US2] Add runner recovery/no-emission test in `tests/integration/runner_flow.rs`

### Implementation for User Story 2

- [x] T011 [US2] Treat missing, unavailable, denied, ambiguous, stale, and unordered focus timestamps as fail-closed outcomes in `crates/signal-auras-core/src/scope.rs`
- [x] T012 [US2] Ensure metadata-unavailable runtime counters distinguish metadata failures from process mismatches in `crates/signal-auras-cli/src/runner.rs`

---

## Phase 5: User Story 3 - Diagnose Stale-Focus Denials (Priority: P3)

**Goal**: Verbose diagnostics distinguish stale metadata, unavailable metadata, permission denial, and process mismatch while preserving privacy.

**Independent Test**: Diagnostic strings include denial kind, configured rule, age, and threshold for stale metadata and omit command-line/window text.

### Verification for User Story 3

- [x] T013 [P] [US3] Add diagnostic classification and privacy tests in `tests/contract/rust_library.rs`
- [x] T014 [P] [US3] Add CLI denial diagnostic tests in `tests/contract/cli_runner.rs`

### Implementation for User Story 3

- [x] T015 [US3] Add privacy-bounded denial diagnostic rendering in `crates/signal-auras-core/src/scope.rs`
- [x] T016 [US3] Log denial diagnostics from runner trigger and motion paths in `crates/signal-auras-cli/src/runner.rs`

---

## Phase 6: Polish & Cross-Cutting Concerns

- [x] T017 Update `README.md` and `tests/compositor/manual-wayland-verification.md` with stale-focus diagnostics and manual verification notes
- [x] T018 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `nix flake check` or document limitations
- [x] T019 Mark completed tasks in `specs/008-stale-focus-handling/tasks.md`

## Phase 7: Architecture Review Follow-Up

**Goal**: Close the live KDE bridge gap where reading cached active-process state refreshed `captured_at` and made stale matching focus metadata look fresh forever.

**Independent Test**: A cached KDE active-process snapshot for a matching process keeps its original callback timestamp, becomes stale without a new callback, and denies a process-scoped macro before scheduling or input emission.

- [x] T020 [P] [US1] Add cached KDE focus timestamp regression coverage in `crates/signal-auras-wayland/src/kde_bridge.rs`
- [x] T021 [US1] Preserve original callback receipt timestamps when constructing KDE active-process contexts in `crates/signal-auras-wayland/src/kde_bridge.rs`
- [x] T022 [US1] Stop `active_process_context()` reads from refreshing cached KDE focus freshness in `crates/signal-auras-wayland/src/kde_bridge.rs`
- [x] T023 [US1] Run focused KDE bridge and stale-focus tests with `cargo test -p signal-auras-wayland kde_bridge` and related contract/integration tests
- [x] T024 [US1] Mark the architecture review follow-up tasks complete in `specs/008-stale-focus-handling/tasks.md`

## Dependencies & Execution Order

- Setup before foundational work.
- Foundational work blocks user stories.
- Implement User Story 1 first, then User Story 2, then User Story 3.
- Tests for each user story precede implementation where practical.
- Polish depends on all user stories.

## Parallel Opportunities

- T002 can run independently after setup.
- T005 and T006 can be authored in parallel.
- T009 and T010 can be authored in parallel.
- T013 and T014 can be authored in parallel.

## Implementation Strategy

Deliver the MVP by completing setup, foundational types, and User Story 1. Then add fail-closed recovery behavior for lost metadata, followed by diagnostic rendering and documentation.
