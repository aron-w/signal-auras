---

description: "Task list template for feature implementation"
---

# Tasks: [FEATURE NAME]

**Input**: Design documents from `/specs/[###-feature-name]/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Verification**: TDD is mandatory. Every user story MUST include failing tests
before implementation for Rust library behavior. Prefer automated tests for
contracts, parser/matcher logic, macro scheduling, Lua capability enforcement,
security boundaries, and Nix reproducibility. Manual compositor verification is
allowed only when automation is not practical; record the exact procedure and
the reason.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Rust workspace**: `crates/*/src/`, `tests/`, and `nix/` at repository root
- **Library first**: core behavior lives in a library crate before CLI, daemon,
  Lua, or desktop integration code
- Paths shown below assume a Rust workspace - adjust based on plan.md structure

<!--
  ============================================================================
  IMPORTANT: The tasks below are SAMPLE TASKS for illustration purposes only.

  The /speckit-tasks command MUST replace these with actual tasks based on:
  - User stories from spec.md (with their priorities P1, P2, P3...)
  - Feature requirements from plan.md and the Constitution Check
  - Entities from data-model.md
  - Endpoints from contracts/
  - Verification paths required for each user story
  - Security, permission, reproducibility, and Lua isolation requirements

  Tasks MUST be organized by user story so each story can be:
  - Implemented independently
  - Tested independently
  - Delivered as an MVP increment

  DO NOT keep these sample tasks in the generated tasks.md file.
  ============================================================================
-->

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [ ] T001 Create project structure per implementation plan
- [ ] T002 Initialize [language] project with [framework] dependencies
- [ ] T003 [P] Configure linting and formatting tools

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

Examples of foundational tasks (adjust based on your project):

- [ ] T004 Establish Rust workspace/library crate structure per implementation plan
- [ ] T005 [P] Configure Nix flake checks and reproducible dev/test commands
- [ ] T006 [P] Define shared error types and diagnosable failure reporting
- [ ] T007 Define permission/capability model for sensitive automation behavior
- [ ] T008 Configure Lua sandbox/capability test harness if scripting is in scope
- [ ] T009 Configure Wayland/compositor adapter test harness or manual verification fixtures

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - [Title] (Priority: P1) 🎯 MVP

**Goal**: [Brief description of what this story delivers]

**Independent Test**: [How to verify this story works on its own]

### Verification for User Story 1

> **NOTE: Write automated tests FIRST when a test harness exists; otherwise
> document the manual verification procedure before implementation.**

- [ ] T010 [P] [US1] Unit test for Rust library behavior in crates/[crate]/src/[module].rs
- [ ] T011 [P] [US1] Contract test for [Lua/API/CLI behavior] in tests/contract/[name].rs
- [ ] T012 [US1] Manual verification for [scenario] using [command or steps] (if automation is not practical)

### Implementation for User Story 1

- [ ] T013 [P] [US1] Implement pure library type/function in crates/[crate]/src/[module].rs
- [ ] T014 [P] [US1] Implement permission/capability checks in crates/[crate]/src/[module].rs
- [ ] T015 [US1] Implement Wayland/compositor adapter in crates/[crate]/src/[module].rs (depends on T013, T014)
- [ ] T016 [US1] Expose behavior through [CLI/Lua/daemon] in crates/[crate]/src/[module].rs
- [ ] T017 [US1] Add diagnosable errors for denied/unavailable permissions and protocols
- [ ] T018 [US1] Add Nix verification command or check target

**Checkpoint**: At this point, User Story 1 MUST be fully functional and testable independently

---

## Phase 4: User Story 2 - [Title] (Priority: P2)

**Goal**: [Brief description of what this story delivers]

**Independent Test**: [How to verify this story works on its own]

### Verification for User Story 2

- [ ] T019 [P] [US2] Unit test for Rust library behavior in crates/[crate]/src/[module].rs
- [ ] T020 [P] [US2] Contract test for [Lua/API/CLI behavior] in tests/contract/[name].rs
- [ ] T021 [US2] Manual verification for [scenario] using [command or steps] (if automation is not practical)

### Implementation for User Story 2

- [ ] T022 [P] [US2] Implement pure library type/function in crates/[crate]/src/[module].rs
- [ ] T023 [US2] Implement permission/capability checks in crates/[crate]/src/[module].rs
- [ ] T024 [US2] Expose behavior through [CLI/Lua/daemon] in crates/[crate]/src/[module].rs
- [ ] T025 [US2] Integrate with User Story 1 components only through documented library contracts

**Checkpoint**: At this point, User Stories 1 AND 2 MUST both work independently

---

## Phase 5: User Story 3 - [Title] (Priority: P3)

**Goal**: [Brief description of what this story delivers]

**Independent Test**: [How to verify this story works on its own]

### Verification for User Story 3

- [ ] T026 [P] [US3] Unit test for Rust library behavior in crates/[crate]/src/[module].rs
- [ ] T027 [P] [US3] Contract test for [Lua/API/CLI behavior] in tests/contract/[name].rs
- [ ] T028 [US3] Manual verification for [scenario] using [command or steps] (if automation is not practical)

### Implementation for User Story 3

- [ ] T029 [P] [US3] Implement pure library type/function in crates/[crate]/src/[module].rs
- [ ] T030 [US3] Implement permission/capability checks in crates/[crate]/src/[module].rs
- [ ] T031 [US3] Expose behavior through [CLI/Lua/daemon] in crates/[crate]/src/[module].rs

**Checkpoint**: All user stories MUST now be independently functional

---

[Add more user story phases as needed, following the same pattern]

---

## Phase N: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [ ] TXXX [P] Documentation updates in docs/
- [ ] TXXX Code cleanup and refactoring
- [ ] TXXX Performance optimization across all stories
- [ ] TXXX [P] Additional Rust unit/property tests in crates/*/src/
- [ ] TXXX [P] Lua API and capability isolation tests in tests/contract/
- [ ] TXXX [P] Nix flake check and package validation
- [ ] TXXX Security hardening
- [ ] TXXX Run quickstart.md validation

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User stories can then proceed in parallel (if staffed)
  - Or sequentially in priority order (P1 → P2 → P3)
- **Polish (Final Phase)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P2)**: Can start after Foundational (Phase 2) - May integrate with US1 but MUST remain independently testable
- **User Story 3 (P3)**: Can start after Foundational (Phase 2) - May integrate with US1/US2 but MUST remain independently testable

### Within Each User Story

- Rust library tests MUST be written and fail before implementation
- Manual verification steps MUST be documented before implementation when automation is not practical
- Pure library behavior before adapters
- Permission/capability checks before side effects
- Adapters before CLI/daemon/Lua exposure
- Core implementation before integration
- Story complete before moving to next priority

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel
- All Foundational tasks marked [P] can run in parallel (within Phase 2)
- Once Foundational phase completes, all user stories can start in parallel (if team capacity allows)
- All tests for a user story marked [P] can run in parallel
- Independent library modules or adapter tasks within a story marked [P] can run in parallel
- Different user stories can be worked on in parallel by different team members

---

## Parallel Example: User Story 1

```bash
# Launch all automated verification for User Story 1 together:
Task: "Unit test for [library behavior] in crates/[crate]/src/[module].rs"
Task: "Contract test for [Lua/API/CLI behavior] in tests/contract/[name].rs"

# Launch independent implementation tasks for User Story 1 together:
Task: "Implement pure library type/function in crates/[crate]/src/[module].rs"
Task: "Implement permission/capability checks in crates/[crate]/src/[module].rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Test User Story 1 independently
5. Deploy/demo if ready

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Deploy/Demo (MVP!)
3. Add User Story 2 → Test independently → Deploy/Demo
4. Add User Story 3 → Test independently → Deploy/Demo
5. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1
   - Developer B: User Story 2
   - Developer C: User Story 3
3. Stories complete and integrate independently

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story MUST be independently completable and testable
- Verify Rust library tests fail before implementing, or document manual compositor verification before implementing
- Include security, consent, Nix reproducibility, and Lua isolation tasks whenever the feature touches those areas
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence
