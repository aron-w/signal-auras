# Tasks: KDE Plasma Wayland Adapter

**Input**: Design documents from `/specs/002-wayland-compositor-adapter/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Verification**: TDD is mandatory. Every user story MUST include failing tests before implementation for Rust library behavior, adapter contracts, CLI behavior, and security boundaries. Manual KDE Plasma Wayland verification is required for desktop-wide behavior that cannot be automated yet.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Prepare KDE/portal dependencies, module skeletons, and verification scaffolding used by all stories.

- [X] T001 Add `zbus` and `ashpd` dependencies for KDE D-Bus and portal integration in `crates/signal-auras-wayland/Cargo.toml`
- [X] T002 Add required KDE/portal development and verification packages to `flake.nix`
- [X] T003 [P] Add KDE provider module declarations in `crates/signal-auras-wayland/src/lib.rs`
- [X] T004 [P] Create KDE provider facade skeleton in `crates/signal-auras-wayland/src/kde.rs`
- [X] T005 [P] Create current-run KDE bridge skeleton in `crates/signal-auras-wayland/src/kde_bridge.rs`
- [X] T006 [P] Update KDE manual verification baseline in `tests/compositor/manual-wayland-verification.md`
- [X] T007 [P] Update KDE quickstart verification commands in `specs/002-wayland-compositor-adapter/quickstart.md`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared KDE session selection, capability, diagnostic, consent, and cleanup behavior that MUST be complete before user story work.

**CRITICAL**: No user story implementation can begin until this phase is complete.

- [X] T008 [P] Add failing KDE provider selection tests for KDE Wayland, non-KDE Wayland, X11, missing KWin, and missing portal cases in `tests/contract/rust_library.rs`
- [X] T009 [P] Add failing CLI startup tests for unsupported non-KDE and non-Wayland sessions in `tests/contract/cli_runner.rs`
- [X] T010 [P] Add failing cleanup tests for KDE bridge and portal teardown after setup failure in `tests/integration/runner_flow.rs`
- [X] T011 Define `KdeSession`, `KdeServiceAvailability`, and selected-provider models in `crates/signal-auras-wayland/src/capability.rs`
- [X] T012 Implement KDE Plasma Wayland session detection and fail-closed provider selection in `crates/signal-auras-wayland/src/capability.rs`
- [X] T013 Implement KDE/portal diagnostic mapping for unsupported session, missing service, denied permission, provider error, and invalidation in `crates/signal-auras-wayland/src/diagnostics.rs`
- [X] T014 Implement current-run KDE bridge state, handles, idempotent unload model, and cleanup report mapping in `crates/signal-auras-wayland/src/kde_bridge.rs`
- [X] T015 Wire `KdePlasmaAdapter` construction and capability probing through `crates/signal-auras-wayland/src/kde.rs`
- [X] T016 Wire CLI real adapter selection to require KDE Plasma Wayland before activation in `crates/signal-auras-cli/src/runner.rs`
- [X] T017 Add runtime stats fields for KDE bridge setup and cleanup in `crates/signal-auras-core/src/stats.rs`
- [X] T018 Preserve Lua sandbox isolation from KDE session, bridge, D-Bus, portal, and active-window APIs in `tests/contract/lua_api.rs`

**Checkpoint**: KDE provider foundation ready. User story implementation can now begin.

---

## Phase 3: User Story 1 - Register Real Desktop Shortcuts (Priority: P1) MVP

**Goal**: Register configured shortcuts with KDE Plasma Wayland, deliver desktop-wide events to the current runner, report unsupported or denied registration support before activation, and unregister every handle during shutdown.

**Independent Test**: Start the runner in a KDE Plasma Wayland session with one explicitly scoped shortcut, confirm registration diagnostics, trigger the shortcut from another focused application, then stop the runner and confirm cleanup.

### Verification for User Story 1

- [X] T019 [P] [US1] Add failing adapter contract tests for KDE global shortcut capability states in `tests/contract/rust_library.rs`
- [X] T020 [P] [US1] Add failing adapter contract tests for KDE bridge install, callback/event mapping, unload, and idempotent cleanup in `tests/contract/rust_library.rs`
- [X] T021 [P] [US1] Add failing CLI tests for KDE shortcut registration output, unsupported shortcut capability, reserved shortcut rejection, and partial cleanup in `tests/contract/cli_runner.rs`
- [X] T022 [P] [US1] Add failing integration tests for partial KDE shortcut registration cleanup after a later hotkey rejection in `tests/integration/runner_flow.rs`
- [X] T023 [US1] Document KDE global shortcut registration, event delivery, reserved shortcut, and cleanup steps in `tests/compositor/manual-wayland-verification.md`

### Implementation for User Story 1

- [X] T024 [P] [US1] Implement KDE global shortcut capability probing in `crates/signal-auras-wayland/src/shortcut.rs`
- [X] T025 [P] [US1] Implement KDE bridge callback/event channel for current-run shortcut delivery in `crates/signal-auras-wayland/src/kde_bridge.rs`
- [X] T026 [US1] Implement KDE global shortcut registration and owned handle tracking in `crates/signal-auras-wayland/src/shortcut.rs`
- [X] T027 [US1] Implement `next_shortcut_event` delivery from KDE bridge events in `crates/signal-auras-wayland/src/shortcut.rs`
- [X] T028 [US1] Integrate KDE shortcut registration and event delivery into `KdePlasmaAdapter` in `crates/signal-auras-wayland/src/kde.rs`
- [X] T029 [US1] Update CLI runner startup, runtime, and shutdown output for KDE shortcut registration and cleanup in `crates/signal-auras-cli/src/runner.rs`
- [X] T030 [US1] Add KDE reserved shortcut, already-owned shortcut, unsupported key, and cleanup diagnostics in `crates/signal-auras-wayland/src/diagnostics.rs`

**Checkpoint**: User Story 1 is independently functional and demonstrable as the MVP.

---

## Phase 4: User Story 2 - Match Shortcuts Against Active Process Metadata (Priority: P2)

**Goal**: Read active-process metadata from KDE/KWin when a shortcut event is handled and use it for conservative process-scoped matching.

**Independent Test**: Configure one shortcut for a known KDE application process, focus matching and non-matching applications, and confirm the shortcut only executes in the matching context with logged match decisions.

### Verification for User Story 2

- [X] T031 [P] [US2] Add failing core tests for KDE active-process contexts with app id, window class, PID, stale state, privileged surfaces, and unavailable state in `tests/contract/rust_library.rs`
- [X] T032 [P] [US2] Add failing adapter contract tests for KWin active-window snapshot conversion in `tests/contract/rust_library.rs`
- [X] T033 [P] [US2] Add failing CLI tests for process-scoped startup failure when KDE active-process metadata capability is denied or unavailable in `tests/contract/cli_runner.rs`
- [X] T034 [P] [US2] Add failing integration tests that shortcut events read fresh active-process context at handling time in `tests/integration/runner_flow.rs`
- [X] T035 [US2] Document KDE active-process match, non-match, privileged surface, and stale metadata verification steps in `tests/compositor/manual-wayland-verification.md`

### Implementation for User Story 2

- [X] T036 [P] [US2] Extend active-process core context conversion support for KDE app id and window class in `crates/signal-auras-core/src/scope.rs`
- [X] T037 [P] [US2] Implement KWin active-window metadata retrieval boundary in `crates/signal-auras-wayland/src/process.rs`
- [X] T038 [US2] Implement KDE active-process capability probing and denial mapping in `crates/signal-auras-wayland/src/process.rs`
- [X] T039 [US2] Convert KWin active-window snapshots into `ActiveProcessContext` with confidence and freshness in `crates/signal-auras-wayland/src/process.rs`
- [X] T040 [US2] Integrate KDE active-process provider into `KdePlasmaAdapter` in `crates/signal-auras-wayland/src/kde.rs`
- [X] T041 [US2] Update CLI event handling to print KDE active-process match, non-match, denied, stale, and unavailable decisions in `crates/signal-auras-cli/src/runner.rs`
- [X] T042 [US2] Update active-process match, non-match, unavailable, denied, and stale metadata stats in `crates/signal-auras-core/src/stats.rs`

**Checkpoint**: User Stories 1 and 2 work independently and together.

---

## Phase 5: User Story 3 - Execute Approved Synthesized Input (Priority: P3)

**Goal**: Emit approved key and text macro actions through the KDE/portal input path only after synthesized-input capability and consent checks pass.

**Independent Test**: Run a macro that emits a short text sequence into a focused KDE text application after explicit approval, then confirm denied permission emits zero input and shutdown cancels pending input.

### Verification for User Story 3

- [X] T043 [P] [US3] Add failing core tests for synthesized-input ordering, no-overlap, cancellation, and denied-input stats in `tests/contract/rust_library.rs`
- [X] T044 [P] [US3] Add failing adapter contract tests for portal session creation, key emission, text-to-key translation, unsupported text, denial, provider failure, and cancellation in `tests/contract/rust_library.rs`
- [X] T045 [P] [US3] Add failing CLI tests for xdg-desktop-portal-kde synthesized-input capability denial before macro execution in `tests/contract/cli_runner.rs`
- [X] T046 [P] [US3] Add failing integration tests that denied or unavailable portal input emits zero actions and cleans up portal sessions in `tests/integration/runner_flow.rs`
- [X] T047 [US3] Document KDE synthesized-input success, denial, unsupported text, and Ctrl-C cancellation verification steps in `tests/compositor/manual-wayland-verification.md`

### Implementation for User Story 3

- [X] T048 [P] [US3] Implement portal RemoteDesktop synthesized-input capability probing in `crates/signal-auras-wayland/src/portal.rs`
- [X] T049 [P] [US3] Implement portal session lifecycle, permission denial mapping, and cleanup in `crates/signal-auras-wayland/src/portal.rs`
- [X] T050 [P] [US3] Implement text-to-key translation validation with no partial text emission in `crates/signal-auras-wayland/src/input.rs`
- [X] T051 [US3] Implement ordered synthesized key and text emission through the KDE/portal path in `crates/signal-auras-wayland/src/input.rs`
- [X] T052 [US3] Integrate synthesized input provider into `KdePlasmaAdapter` in `crates/signal-auras-wayland/src/kde.rs`
- [X] T053 [US3] Gate macro execution on KDE/portal synthesized-input capability and current-run consent in `crates/signal-auras-cli/src/runner.rs`
- [X] T054 [US3] Cancel pending portal input and close portal sessions during shutdown cleanup in `crates/signal-auras-cli/src/runner.rs`
- [X] T055 [US3] Preserve Lua sandbox isolation from raw synthesized input and portal APIs in `tests/contract/lua_api.rs`

**Checkpoint**: All user stories are independently functional, but the feature is complete only after KDE manual verification passes.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Validate reproducibility, harden diagnostics, and align documentation after all selected stories are complete.

- [X] T056 [P] Run and record `nix develop -c cargo fmt --check` in `specs/002-wayland-compositor-adapter/quickstart.md`
- [X] T057 [P] Run and record `nix develop -c cargo clippy --all-targets -- -D warnings` in `specs/002-wayland-compositor-adapter/quickstart.md`
- [X] T058 [P] Run and record `nix develop -c cargo test` in `specs/002-wayland-compositor-adapter/quickstart.md`
- [X] T059 [P] Run and record `nix flake check` in `specs/002-wayland-compositor-adapter/quickstart.md`
- [X] T060 Update README capability and limitation notes for KDE Plasma Wayland adapter behavior in `README.md`
- [X] T061 Review and tighten safety-boundary comments for D-Bus object lifetimes, KWin bridge state, portal sessions, and event-loop ownership in `crates/signal-auras-wayland/src/kde.rs`
- [X] T062 Validate all KDE Plasma Wayland manual compositor scenarios and record results in `tests/compositor/manual-wayland-verification.md`
- [X] T063 Run a final placeholder scan for unresolved planning markers in `specs/002-wayland-compositor-adapter/tasks.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies.
- **Foundational (Phase 2)**: Depends on Setup completion and blocks all user stories.
- **User Story 1 (Phase 3)**: Depends on Foundational and is the MVP.
- **User Story 2 (Phase 4)**: Depends on Foundational; it can be developed after or alongside US1 using fake events, but real end-to-end value needs US1 event delivery.
- **User Story 3 (Phase 5)**: Depends on Foundational; it can be tested independently with fake macro execution, but full desktop macro flow needs US1 and benefits from US2.
- **Polish (Phase 6)**: Depends on completed selected user stories.

### User Story Dependencies

- **US1 - Register Real Desktop Shortcuts**: No dependency on US2 or US3.
- **US2 - Match Shortcuts Against Active Process Metadata**: No dependency on US3; uses US1 event flow for end-to-end demo but remains testable with fake events.
- **US3 - Execute Approved Synthesized Input**: No dependency on US2; uses US1 trigger flow for end-to-end demo but remains testable with fake macro execution.

### Within Each User Story

- Write failing automated tests before implementation.
- Document manual KDE verification before implementation for live desktop behavior.
- Implement core data and decision behavior before adapter side effects.
- Implement permission and capability checks before registration, metadata reads, bridge setup, portal sessions, or input emission.
- Integrate CLI output after core and adapter contracts exist.
- Validate the story independently before moving to the next priority.

### Parallel Opportunities

- T003, T004, T005, T006, and T007 can run in parallel after dependencies are understood.
- T008, T009, and T010 can run in parallel because they touch different test scopes.
- T019, T020, T021, and T022 can run in parallel for US1 verification.
- T024 and T025 can run in parallel for US1 implementation after verification tests exist.
- T031, T032, T033, and T034 can run in parallel for US2 verification.
- T036 and T037 can run in parallel for US2 implementation.
- T043, T044, T045, and T046 can run in parallel for US3 verification.
- T048, T049, and T050 can run in parallel for US3 implementation after tests exist.
- T056, T057, T058, and T059 can run in parallel only if build output handling is serialized or separate worktrees are used.

---

## Parallel Example: User Story 1

```bash
Task: "T019 [P] [US1] Add failing adapter contract tests for KDE global shortcut capability states in tests/contract/rust_library.rs"
Task: "T020 [P] [US1] Add failing adapter contract tests for KDE bridge install, callback/event mapping, unload, and idempotent cleanup in tests/contract/rust_library.rs"
Task: "T021 [P] [US1] Add failing CLI tests for KDE shortcut registration output, unsupported shortcut capability, reserved shortcut rejection, and partial cleanup in tests/contract/cli_runner.rs"
Task: "T022 [P] [US1] Add failing integration tests for partial KDE shortcut registration cleanup after a later hotkey rejection in tests/integration/runner_flow.rs"
```

## Parallel Example: User Story 2

```bash
Task: "T031 [P] [US2] Add failing core tests for KDE active-process contexts with app id, window class, PID, stale state, privileged surfaces, and unavailable state in tests/contract/rust_library.rs"
Task: "T032 [P] [US2] Add failing adapter contract tests for KWin active-window snapshot conversion in tests/contract/rust_library.rs"
Task: "T034 [P] [US2] Add failing integration tests that shortcut events read fresh active-process context at handling time in tests/integration/runner_flow.rs"
```

## Parallel Example: User Story 3

```bash
Task: "T043 [P] [US3] Add failing core tests for synthesized-input ordering, no-overlap, cancellation, and denied-input stats in tests/contract/rust_library.rs"
Task: "T044 [P] [US3] Add failing adapter contract tests for portal session creation, key emission, text-to-key translation, unsupported text, denial, provider failure, and cancellation in tests/contract/rust_library.rs"
Task: "T046 [P] [US3] Add failing integration tests that denied or unavailable portal input emits zero actions and cleans up portal sessions in tests/integration/runner_flow.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 setup.
2. Complete Phase 2 foundation.
3. Complete Phase 3 User Story 1.
4. Validate US1 with automated tests and manual KDE global shortcut verification.
5. Stop and demo real desktop-wide shortcut registration and cleanup before adding metadata or synthesized input.

### Incremental Delivery

1. Deliver US1 to prove real KDE desktop-wide registration, event delivery, and cleanup.
2. Add US2 to make registration process-aware with conservative KDE/KWin metadata matching.
3. Add US3 to execute approved key/text macro actions through the KDE/portal path.
4. Run polish verification and update docs after all stories are complete.

### Notes

- Tasks marked `[P]` touch different files or are verification tasks that can be prepared independently.
- Story labels map to the spec priorities: US1 = shortcut registration, US2 = active-process metadata, US3 = synthesized input.
- Manual verification is required only for live KDE behavior that cannot yet be exercised reliably in automated tests.
- Security, consent, Nix reproducibility, Lua isolation, diagnosable KDE failure behavior, and KDE manual verification are release blockers for this feature.
