# Tasks: Lua Hotkey Runner

**Input**: Design documents from `/specs/001-lua-hotkey-runner/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Verification**: TDD is mandatory. Every user story includes failing tests before implementation for Rust library behavior. Prefer automated tests for contracts, parser/matcher logic, macro scheduling, Lua capability enforcement, security boundaries, and Nix reproducibility. Manual compositor verification is allowed only when automation is not practical; record the exact procedure and reason.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

- Rust workspace: `Cargo.toml`, `crates/*/src/`, and `tests/` at repository root
- Library first: core behavior lives in `crates/signal-auras-core/` before CLI, Lua, or Wayland integration code
- Feature docs remain under `specs/001-lua-hotkey-runner/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Initialize the Rust/Nix project structure needed by every story.

- [X] T001 Update `flake.nix` with Rust toolchain, cargo, rustfmt, clippy, pkg-config, and native libraries needed by planned Lua/Wayland crates
- [X] T002 Create workspace root `Cargo.toml` with members for `crates/signal-auras-core`, `crates/signal-auras-lua`, `crates/signal-auras-wayland`, and `crates/signal-auras-cli`
- [X] T003 Create crate manifests in `crates/signal-auras-core/Cargo.toml`, `crates/signal-auras-lua/Cargo.toml`, `crates/signal-auras-wayland/Cargo.toml`, and `crates/signal-auras-cli/Cargo.toml`
- [X] T004 Create initial module files in `crates/signal-auras-core/src/lib.rs`, `crates/signal-auras-lua/src/lib.rs`, `crates/signal-auras-wayland/src/lib.rs`, and `crates/signal-auras-cli/src/main.rs`
- [X] T005 [P] Create contract test directories and placeholders in `tests/contract/cli_runner.rs`, `tests/contract/lua_api.rs`, `tests/contract/rust_library.rs`, and `tests/integration/runner_flow.rs`
- [X] T006 [P] Verify `.gitignore` contains Rust/Nix build outputs including `target/`, `debug/`, `release/`, `*.rs.bk`, `*.rlib`, `*.prof*`, `*.log`, `.env*`, `.direnv/`, and `result`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Establish shared types, safety boundaries, and test harnesses that block all user stories.

**CRITICAL**: No user story work can begin until this phase is complete.

- [X] T007 [P] Write failing tests for diagnosable error phases and capabilities in `crates/signal-auras-core/src/error.rs`
- [X] T008 [P] Write failing tests for hotkey/process/macro value validation in `crates/signal-auras-core/src/hotkey.rs`, `crates/signal-auras-core/src/scope.rs`, and `crates/signal-auras-core/src/macro_plan.rs`
- [X] T009 [P] Write failing adapter trait contract tests for unsupported protocol, denied permission, and unavailable active process in `tests/contract/rust_library.rs`
- [X] T010 Implement `DiagnosableError`, `ErrorPhase`, and `Capability` in `crates/signal-auras-core/src/error.rs`
- [X] T011 Implement shared hotkey, process name, scope, macro action, macro definition, and registration ID types in `crates/signal-auras-core/src/hotkey.rs`, `crates/signal-auras-core/src/scope.rs`, and `crates/signal-auras-core/src/macro_plan.rs`
- [X] T012 Implement capability adapter traits `ActiveProcessProvider`, `HotkeyRegistrar`, and `MacroExecutor` in `crates/signal-auras-core/src/lib.rs`
- [X] T013 Implement Wayland adapter error mapping stubs in `crates/signal-auras-wayland/src/adapter.rs`, `crates/signal-auras-wayland/src/diagnostics.rs`, and `crates/signal-auras-wayland/src/portal.rs`
- [X] T014 Add manual compositor verification procedure file at `tests/compositor/manual-wayland-verification.md` based on `specs/001-lua-hotkey-runner/quickstart.md`
- [X] T015 Run `nix develop -c cargo test` against workspace `Cargo.toml` and confirm foundational tests fail only for not-yet-implemented story behavior

**Checkpoint**: Foundation ready. User story implementation can now begin.

---

## Phase 3: User Story 1 - Run a scoped Lua hotkey macro (Priority: P1) MVP

**Goal**: Start the CLI with exactly one scoped Lua file, validate it, log startup config, and register hotkeys for the declared process scope through a mockable adapter.

**Independent Test**: Run the CLI with a Lua file declaring `scope.processes = { "poe2.exe" }` and an `F5` macro; verify startup logs show script path, scope, validation, and registration result before waiting for triggers.

### Verification for User Story 1

- [X] T016 [P] [US1] Write failing core config validation tests for scoped scripts, empty hotkeys, malformed scope, duplicate hotkeys, unsupported hotkeys, and unsupported macro actions in `crates/signal-auras-core/src/config.rs`
- [X] T017 [P] [US1] Write failing Lua API contract tests for the v1 sample script, macro constructors, and denied ambient APIs in `tests/contract/lua_api.rs`
- [X] T018 [P] [US1] Write failing CLI contract tests for exactly-one-argument validation and scoped startup output in `tests/contract/cli_runner.rs`
- [X] T019 [P] [US1] Write failing integration test for scoped registration with mock hotkey registrar in `tests/integration/runner_flow.rs`

### Implementation for User Story 1

- [X] T020 [P] [US1] Implement validated `LuaAutomationConfiguration` and `HotkeyBinding` construction in `crates/signal-auras-core/src/config.rs`
- [X] T021 [P] [US1] Implement macro action validation and ordered macro plan storage in `crates/signal-auras-core/src/macro_plan.rs`
- [X] T022 [US1] Implement sandboxed Lua loading and constructor extraction in `crates/signal-auras-lua/src/sandbox.rs` and `crates/signal-auras-lua/src/lib.rs`
- [X] T023 [US1] Implement CLI argument parsing for `signal-auras run <lua-file>` in `crates/signal-auras-cli/src/main.rs`
- [X] T024 [US1] Implement startup orchestration with script load, validation, capability probe, scoped registration, and logs in `crates/signal-auras-cli/src/runner.rs`
- [X] T025 [US1] Implement mock-friendly Wayland registration adapter skeleton in `crates/signal-auras-wayland/src/adapter.rs`
- [X] T026 [US1] Run `nix develop -c cargo test --test lua_api --test cli_runner --test runner_flow` for `tests/contract/lua_api.rs`, `tests/contract/cli_runner.rs`, and `tests/integration/runner_flow.rs` and ensure US1 tests pass

**Checkpoint**: US1 is independently functional as the MVP.

---

## Phase 4: User Story 2 - Choose scope when Lua omits it (Priority: P2)

**Goal**: Prompt in the terminal for process names or explicit global scope when the Lua file omits scope, and keep that choice current-run only.

**Independent Test**: Run the CLI with a scope-free Lua file; verify process-name selection registers scoped hotkeys, global selection requires explicit confirmation, and cancel exits without registration.

### Verification for User Story 2

- [X] T027 [P] [US2] Write failing consent tests for process selection, explicit global selection, cancel, non-interactive stdin, and no default global behavior in `crates/signal-auras-core/src/consent.rs`
- [X] T028 [P] [US2] Write failing CLI prompt contract tests for process, global, cancel, and non-interactive flows in `tests/contract/cli_runner.rs`
- [X] T029 [P] [US2] Write failing integration test proving prompt scope is not persisted between runs in `tests/integration/runner_flow.rs`

### Implementation for User Story 2

- [X] T030 [P] [US2] Implement `ScopeSelection` consent constructors and current-run-only rules in `crates/signal-auras-core/src/consent.rs`
- [X] T031 [US2] Implement terminal prompt UI for missing scope in `crates/signal-auras-cli/src/prompt.rs`
- [X] T032 [US2] Integrate missing-scope prompt into runner startup before hotkey registration in `crates/signal-auras-cli/src/runner.rs`
- [X] T033 [US2] Add visible consent logs for prompt-selected process scope, explicit global scope, cancel, and non-interactive failure in `crates/signal-auras-cli/src/runner.rs`
- [X] T034 [US2] Run `nix develop -c cargo test --test cli_runner --test runner_flow` for `tests/contract/cli_runner.rs` and `tests/integration/runner_flow.rs` and ensure US1 and US2 tests pass

**Checkpoint**: US1 and US2 work independently.

---

## Phase 5: User Story 3 - Trigger macros only for matching active process (Priority: P3)

**Goal**: Compare each hotkey trigger with active process scope and execute macro actions in order only when allowed.

**Independent Test**: Simulate active process names and verify `F5` sends `/hideout` only for `poe2.exe`, denies non-matches, counts denials, and stops a macro on action failure.

### Verification for User Story 3

- [X] T035 [P] [US3] Write failing scope matcher tests for matching process, non-matching process, unknown active process, and explicit global scope in `crates/signal-auras-core/src/scope.rs`
- [X] T036 [P] [US3] Write failing macro scheduler tests for ordered key/text/delay execution, action failure stop, and repeated-trigger denial in `crates/signal-auras-core/src/macro_plan.rs`
- [X] T037 [P] [US3] Write failing stats tests for triggers, successes, failures, denied actions, permission failures, and scope mismatches in `crates/signal-auras-core/src/stats.rs`
- [X] T038 [P] [US3] Write failing integration test for allowed and denied trigger handling with mock active process provider and macro executor in `tests/integration/runner_flow.rs`

### Implementation for User Story 3

- [X] T039 [P] [US3] Implement active-process scope decision logic in `crates/signal-auras-core/src/scope.rs`
- [X] T040 [P] [US3] Implement macro execution planning, delay handling, action failure behavior, and deny-while-running policy in `crates/signal-auras-core/src/macro_plan.rs`
- [X] T041 [P] [US3] Implement runtime stats counters and final summary model in `crates/signal-auras-core/src/stats.rs`
- [X] T042 [US3] Integrate hotkey trigger handling with active-process provider, scope decision, macro executor, denial logs, and stats in `crates/signal-auras-cli/src/runner.rs`
- [X] T043 [US3] Implement synthesized-input adapter skeleton and diagnosable unavailable-permission errors in `crates/signal-auras-wayland/src/adapter.rs`
- [X] T044 [US3] Run `nix develop -c cargo test --test runner_flow` for `tests/integration/runner_flow.rs` and ensure US1 through US3 tests pass

**Checkpoint**: US1, US2, and US3 work independently.

---

## Phase 6: User Story 4 - Stop cleanly and see final stats (Priority: P4)

**Goal**: Handle Ctrl-C by unregistering hotkeys, stopping new triggers, and printing final runtime stats.

**Independent Test**: Start the runner with mock registrations, trigger allowed and denied macros, simulate Ctrl-C, and verify unregister calls plus final stats output.

### Verification for User Story 4

- [X] T045 [P] [US4] Write failing shutdown stats tests for Ctrl-C, startup error, runtime error, and in-flight macro shutdown in `crates/signal-auras-core/src/stats.rs`
- [X] T046 [P] [US4] Write failing CLI shutdown contract tests for unregister-all and final summary output in `tests/contract/cli_runner.rs`
- [X] T047 [P] [US4] Write failing integration test for Ctrl-C shutdown path with mock registrar and executor in `tests/integration/runner_flow.rs`

### Implementation for User Story 4

- [X] T048 [P] [US4] Implement shutdown reason and final summary rendering support in `crates/signal-auras-core/src/stats.rs`
- [X] T049 [US4] Implement Ctrl-C signal handling and shutdown orchestration in `crates/signal-auras-cli/src/runner.rs`
- [X] T050 [US4] Ensure `HotkeyRegistrar::unregister_all` is called on Ctrl-C, startup cleanup, and runtime-error cleanup in `crates/signal-auras-cli/src/runner.rs`
- [X] T051 [US4] Add final summary terminal output for elapsed runtime, triggers, successes, failures, denials, and permission failures in `crates/signal-auras-cli/src/runner.rs`
- [X] T052 [US4] Run `nix develop -c cargo test` against workspace `Cargo.toml` and ensure all automated tests pass

**Checkpoint**: All user stories are independently functional.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Validate reproducibility, formatting, security documentation, and manual Wayland gaps.

- [X] T053 [P] Update `README.md` with v1 CLI usage, consent model, Lua sample, and unsupported Wayland behavior
- [X] T054 [P] Add example scripts in `examples/poe2-hideout.lua` and `examples/prompt-scope.lua`
- [X] T055 [P] Review Lua sandbox hardening and add missing denied-capability regression tests in `tests/contract/lua_api.rs`
- [X] T056 [P] Update `tests/compositor/manual-wayland-verification.md` with actual compositor/protocol support selected during implementation
- [X] T057 Run `nix develop -c cargo fmt --check` against workspace `Cargo.toml` and `crates/*/src/`
- [X] T058 Run `nix develop -c cargo clippy --all-targets -- -D warnings` against workspace `Cargo.toml` and `crates/*/src/`
- [X] T059 Run `nix develop -c cargo test` against workspace `Cargo.toml` and `tests/`
- [X] T060 Run quickstart failure scenarios from `specs/001-lua-hotkey-runner/quickstart.md` and record any manual gaps in `tests/compositor/manual-wayland-verification.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- Setup (Phase 1): no dependencies
- Foundational (Phase 2): depends on Setup completion and blocks all user stories
- User Story phases: depend on Foundational completion
- Polish (Phase 7): depends on all desired user stories being complete

### User Story Dependencies

- US1 (P1): can start after Foundational and is the MVP
- US2 (P2): can start after Foundational but integrates cleanly after US1 runner startup exists
- US3 (P3): can start after Foundational but is easiest after US1 registration and US2 scope resolution exist
- US4 (P4): can start after Foundational but is easiest after US3 stats and trigger handling exist

### Within Each User Story

- Write failing tests first
- Implement pure core behavior before adapters and CLI orchestration
- Implement permission/capability checks before side effects
- Complete the story checkpoint before moving to the next priority

### Parallel Opportunities

- T005 and T006 can run after T001-T004 are underway because they touch separate files
- T007, T008, and T009 can run in parallel
- US1 verification tasks T016-T019 can run in parallel
- US2 verification tasks T027-T029 can run in parallel
- US3 verification tasks T035-T038 can run in parallel
- US4 verification tasks T045-T047 can run in parallel
- Polish tasks T053-T056 can run in parallel

---

## Parallel Example: User Story 1

```bash
# Launch all US1 verification tasks together:
Task: "T016 Write failing core config validation tests in crates/signal-auras-core/src/config.rs"
Task: "T017 Write failing Lua API contract tests in tests/contract/lua_api.rs"
Task: "T018 Write failing CLI contract tests in tests/contract/cli_runner.rs"
Task: "T019 Write failing integration test in tests/integration/runner_flow.rs"

# After tests exist, launch independent implementation tasks:
Task: "T020 Implement validated LuaAutomationConfiguration in crates/signal-auras-core/src/config.rs"
Task: "T021 Implement macro action validation in crates/signal-auras-core/src/macro_plan.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 setup.
2. Complete Phase 2 foundational safety boundaries and adapter contracts.
3. Complete Phase 3 US1 scoped Lua startup and registration.
4. Stop and validate US1 independently with the focused test command in T026.

### Incremental Delivery

1. US1: scoped Lua startup and registration.
2. US2: terminal scope prompt and explicit global consent.
3. US3: active-process gated trigger execution.
4. US4: Ctrl-C shutdown and final stats.
5. Polish: reproducibility, examples, docs, and manual compositor verification.

### Notes

- `tasks.md` intentionally uses mockable Wayland adapters for automated tests until a real compositor harness exists.
- No task may add persistence, daemon behavior, IPC, autostart, or hidden global registration for v1.
- Every completed task must be marked `[X]` before implementation continues to the next dependent task.
