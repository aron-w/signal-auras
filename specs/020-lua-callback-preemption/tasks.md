# Tasks: Lua Callback Preemption

**Input**: Design documents from `/specs/020-lua-callback-preemption/`

**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/lua-callback-preemption.md`, `quickstart.md`

**Verification**: TDD is mandatory. Each user story starts with failing Rust/Lua/runner tests before implementation. Required final verification: `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `XDG_CACHE_HOME=/tmp/nix-cache nix flake check` when the local environment supports it.

## Phase 1: Setup

**Purpose**: Confirm the feature slice has explicit Spec Kit work items before implementation.

- [x] T001 Confirm 020 feature artifacts and requirements checklist are present in `specs/020-lua-callback-preemption/`

---

## Phase 2: Foundational

**Purpose**: Add the shared library/runtime contracts that block user story work.

- [x] T002 [P] Add tests for scheduler state release after preemption and distinct callback dispositions in `crates/signal-auras-core/src/controller.rs`
- [x] T003 [P] Add tests for Lua runtime budget types and nonzero enforcement in `crates/signal-auras-lua/src/runtime.rs`
- [x] T004 Implement library-owned callback budget defaults, `Preempted` disposition, and scheduler preemption release in `crates/signal-auras-core/src/controller.rs`
- [x] T005 Implement Lua callback execution budget types and exported contract surface in `crates/signal-auras-lua/src/runtime.rs` and `crates/signal-auras-lua/src/lib.rs`

**Checkpoint**: Core and Lua runtime contracts compile independently and expose the primitives needed by the runner.

---

## Phase 3: User Story 1 - Stop Runaway Lua Callbacks (Priority: P1)

**Goal**: A non-yielding callback cannot monopolize the runtime loop and leaves the scheduler reusable after interruption.

**Independent Test**: Trigger an infinite-loop imperative callback and verify it returns a preempted result, emits no post-timeout output, and releases the scheduler active slot.

### Verification for User Story 1

- [x] T006 [P] [US1] Add Lua contract test for infinite loop before first host request in `tests/contract/lua_api.rs`
- [x] T007 [P] [US1] Add runner test proving a runaway callback is classified as preempted and emits no output in `crates/signal-auras-cli/src/runner/controller.rs`
- [x] T008 [US1] Add scheduler reuse assertion after preempted task in `crates/signal-auras-core/src/controller.rs`

### Implementation for User Story 1

- [x] T009 [US1] Install per-coroutine `mlua` instruction hooks around callback resume in `crates/signal-auras-lua/src/runtime.rs`
- [x] T010 [US1] Convert hook budget interruption into `LuaCallbackStep::Preempted` without exposing it as an ordinary script failure in `crates/signal-auras-lua/src/runtime.rs`
- [x] T011 [US1] Pass task budgets from runner to Lua runtime and release scheduler state through preemption handling in `crates/signal-auras-cli/src/runner/controller.rs`
- [x] T012 [US1] Record and log privacy-bounded preemption diagnostics in `crates/signal-auras-cli/src/runner/controller.rs` and `crates/signal-auras-core/src/stats.rs`

**Checkpoint**: User Story 1 is functional and independently testable.

---

## Phase 4: User Story 2 - Preserve Yielding Callback Semantics (Priority: P2)

**Goal**: Existing yielding callbacks continue to sleep, resume, request host APIs, and complete without false preemption.

**Independent Test**: Run callbacks that yield through `sa.sleep`, `sa.window.*`, `sa.input.*`, and denied capability paths; verify results match existing behavior except for explicit budget enforcement on active Lua execution.

### Verification for User Story 2

- [x] T013 [P] [US2] Add Lua contract test for infinite loop after resuming from `sa.sleep` in `tests/contract/lua_api.rs`
- [x] T014 [P] [US2] Add Lua contract test for bounded work plus `sa.sleep` completion in `tests/contract/lua_api.rs`
- [x] T015 [P] [US2] Add or preserve capability-denial classification test under preemption in `tests/contract/lua_api.rs`

### Implementation for User Story 2

- [x] T016 [US2] Ensure callback budget counts only active resume execution and excludes pending sleep time in `crates/signal-auras-cli/src/runner/controller.rs`
- [x] T017 [US2] Ensure hook cleanup occurs after yield, completion, failure, or preemption in `crates/signal-auras-lua/src/runtime.rs`
- [x] T018 [US2] Preserve existing Lua-facing API names, callback syntax, import behavior, and host request parsing in `crates/signal-auras-lua/src/runtime.rs`

**Checkpoint**: User Stories 1 and 2 both pass independently.

---

## Phase 5: User Story 3 - Diagnose Callback Budget Decisions (Priority: P3)

**Goal**: Developers can distinguish preempted callbacks from completed, slow, failed, denied, dropped, skipped, and cancelled work.

**Independent Test**: Trigger representative callback dispositions and verify diagnostics/stats distinguish preemption without leaking private window/process metadata.

### Verification for User Story 3

- [x] T019 [P] [US3] Add stats tests for preempted callback counting in `crates/signal-auras-core/src/stats.rs`
- [x] T020 [P] [US3] Add runner diagnostics assertion for preempted disposition and elapsed/budget fields in `tests/contract/cli_runner.rs` or `crates/signal-auras-cli/src/runner/controller.rs`

### Implementation for User Story 3

- [x] T021 [US3] Add `RuntimeStats` preempted callback counter and record helper in `crates/signal-auras-core/src/stats.rs`
- [x] T022 [US3] Emit structured tracing fields for callback name, trigger label, budget, elapsed, queue depth, and disposition in `crates/signal-auras-cli/src/runner/controller.rs`
- [x] T023 [US3] Review quickstart verification notes; no update needed because observed commands match `specs/020-lua-callback-preemption/quickstart.md`

**Checkpoint**: All planned user stories are functional and diagnosable.

---

## Phase 6: Polish & Verification

**Purpose**: Close the feature with reproducible checks and a focused review.

- [x] T024 Run targeted tests for core scheduler, Lua runtime contracts, and CLI runner contracts
- [x] T025 Run `cargo fmt --check`
- [x] T026 Run `cargo test`
- [x] T027 Run `cargo clippy --all-targets -- -D warnings`
- [x] T028 Run `XDG_CACHE_HOME=/tmp/nix-cache nix flake check`
- [x] T029 Review `git diff --check` and source diff for architecture, privacy, scheduler ownership, and unrelated churn
- [x] T030 Commit the focused 020 implementation with a Conventional Commit message

---

## Dependencies & Execution Order

- Phase 1 precedes all implementation.
- Phase 2 blocks all user stories because it defines the shared scheduler/runtime contract.
- User Story 1 is the MVP and must complete before User Story 2 runner refinements.
- User Story 2 depends on the US1 hook and runner integration but remains independently testable through Lua contract tests.
- User Story 3 depends on preemption classification existing in US1.
- Phase 6 runs after the implemented stories and review are complete.

## Parallel Opportunities

- T002 and T003 can be written in parallel.
- T006 and T007 can be written in parallel after Phase 2 tests exist.
- T013, T014, and T015 can be written in parallel after hook support exists.
- T019 and T020 can be written in parallel after `Preempted` is wired through the runner.

## Implementation Strategy

1. Complete Phase 1 and Phase 2.
2. Deliver User Story 1 as the MVP: preempt infinite-loop callbacks, release scheduler state, and prevent post-timeout output.
3. Preserve yielding callback semantics through User Story 2 tests.
4. Add diagnostic distinction through User Story 3.
5. Run the full required verification and commit.
