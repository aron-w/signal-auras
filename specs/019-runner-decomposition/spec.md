# Feature Specification: Runner Architecture Decomposition

**Feature Branch**: `019-runner-decomposition`

**Created**: 2026-06-05

**Status**: Draft

**Input**: User description: "Split runner architecture review findings into behavioral contracts and defer structural runner decomposition until lifecycle cleanup, callback responsiveness, and focus policy behavior are protected by tests."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Reuse Lifecycle Inputs Without Long Argument Lists (Priority: P1)

A maintainer changes live runtime startup or cleanup behavior and can review a small lifecycle configuration type instead of following scattered parameters through `runner.rs`.

**Why this priority**: Lifecycle ownership is the highest-risk runner responsibility because it controls input grabs, virtual devices, callback registrations, and shutdown cleanup.

**Independent Test**: Refactor lifecycle entry points behind explicit configuration/session types while existing lifecycle cleanup, startup-failure cleanup, and shutdown tests continue to pass unchanged.

**Acceptance Scenarios**:

1. **Given** lifecycle behavior is protected by tests, **When** runner startup is reviewed, **Then** resource ownership is visible through named config/session fields instead of a long argument list.
2. **Given** startup fails after partial acquisition, **When** cleanup runs, **Then** the same lifecycle session cleanup path is used and tests still prove idempotency.

---

### User Story 2 - Isolate Runtime Loop Coordination (Priority: P2)

A maintainer changes input, callback, timer, hotplug, repeat, or shutdown wake ordering and can do so inside a focused runtime-loop coordinator boundary.

**Why this priority**: Event-loop latency and correctness depend on wake ordering, so coordination must be reviewable without mixing CLI parsing, controller execution, and cleanup.

**Independent Test**: Move loop coordination behind a reusable type while callback wakeup, no-new-work-after-shutdown, and focus pass-through tests continue to pass.

**Acceptance Scenarios**:

1. **Given** callbacks and input are both ready, **When** the runtime loop drains work, **Then** existing ordering and diagnostics are preserved.
2. **Given** shutdown starts while work is pending, **When** the coordinator advances, **Then** no new macro/controller work is scheduled.

---

### User Story 3 - Separate Controller Execution From CLI Orchestration (Priority: P3)

A maintainer changes Lua controller callback execution or diagnostics and can test it through a reusable Rust boundary rather than CLI-only code.

**Why this priority**: Lua callback responsiveness and sandbox behavior must remain stable while runner structure changes.

**Independent Test**: Extract controller execution wiring so Lua runtime tests and CLI runner tests cover the same controller-execution contract.

**Acceptance Scenarios**:

1. **Given** a controller callback is pending, **When** the runner executes controller work, **Then** callback budgets, wakeups, and diagnostics remain unchanged.
2. **Given** a Lua capability is denied, **When** controller execution is attempted, **Then** denial remains Rust-owned and diagnosable without exposing ambient OS access to Lua.

### Edge Cases

- Decomposition must not change Lua-facing APIs, consent prompts, process-scope semantics, or compositor support claims.
- Refactors must not introduce daemon state, hidden IPC, persistent caches, global registries, or an async runtime.
- Existing diagnostics must keep enough resource, callback, focus, and shutdown context for review while avoiding private command-line or window text.
- Structural changes must be incremental and revertible by module boundary, not a broad rewrite.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST keep runner decomposition separate from behavior-changing lifecycle cleanup, callback responsiveness, and focus policy work.
- **FR-002**: System MUST introduce explicit lifecycle configuration and session ownership types before removing long lifecycle argument lists.
- **FR-003**: System MUST route startup failure cleanup and normal shutdown through the same reusable session cleanup boundary where practical.
- **FR-004**: System MUST isolate runtime-loop coordination for input, callbacks, timers, hotplug, repeats, focus, and shutdown wakeups.
- **FR-005**: System MUST isolate Lua controller execution wiring from CLI argument parsing and live runtime orchestration.
- **FR-006**: System MUST preserve existing public Lua APIs, existing consent model, fail-closed permission behavior, and current-run resource ownership.
- **FR-007**: System MUST include automated regression coverage before and after each extracted boundary.
- **FR-008**: System MUST keep all core automation semantics in reusable Rust library or narrow adapter types rather than CLI-only code.
- **FR-009**: System MUST document any remaining manual compositor verification for boundaries that cannot be automated.

### Key Entities

- **Lifecycle Configuration**: Named startup inputs needed to create current-run runtime resources.
- **Runtime Session**: Owned current-run resources and an idempotent cleanup boundary.
- **Runtime Loop Coordinator**: Wake ordering and work-drain boundary for input, callbacks, timers, hotplug, repeats, focus, and shutdown.
- **Controller Execution Boundary**: Rust-owned contract that runs Lua controller work under budgets, capabilities, and diagnostics.
- **Diagnostics Context**: Privacy-bounded fields that explain lifecycle, wakeup, focus, controller, and cleanup decisions.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Lifecycle entry points no longer trigger `too_many_arguments` clippy findings.
- **SC-002**: Startup failure, normal shutdown, duplicate cleanup, callback wakeup, focus pass-through, and Lua controller tests pass before and after decomposition.
- **SC-003**: At least three cohesive runner responsibilities are represented by named types or modules with focused tests.
- **SC-004**: No public Lua API, consent behavior, default policy, or compositor support claim changes as a result of the refactor.
- **SC-005**: `cargo fmt --check`, `cargo test`, and `cargo clippy --all-targets -- -D warnings` pass after each delivered decomposition increment.

## Assumptions

- Lifecycle cleanup guarantees, callback responsiveness, and focus policy unification are implemented first under their existing behavior specs.
- Runner decomposition is a follow-up refactor and should not block urgent correctness fixes.
- The existing Rust workspace and Spec Kit verification commands remain the reproducible path for this work.
