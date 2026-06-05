# Tasks: Lua Controller Runtime

**Input**: Design documents from `/specs/016-lua-controller-runtime/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Verification**: TDD is mandatory for Rust library behavior and Lua capability enforcement.

## Phase 1: Setup

- [X] T001 Create `specs/016-lua-controller-runtime/` artifacts from the supplied feature specification.
- [X] T002 [P] Update `AGENTS.md` to reference `specs/016-lua-controller-runtime/plan.md`.

## Phase 2: Foundational

- [X] T003 [P] Define Rust controller registration, callback scheduling, output batching, and capability contracts in `specs/016-lua-controller-runtime/contracts/rust-controller-library.md`.
- [X] T004 [P] Define Lua controller loader/import API and compatibility contract in `specs/016-lua-controller-runtime/contracts/lua-controller-api.md`.

## Phase 3: User Story 1 - Register Lua Controllers (Priority: P1)

**Goal**: Load a multi-file controller and validate registrations before runtime activation.

**Independent Test**: Load a controller source and a rooted imported module; verify registrations are collected and duplicates fail.

- [X] T005 [P] [US1] Add controller registration validation tests in `crates/signal-auras-core/src/controller.rs`.
- [X] T006 [P] [US1] Add Lua controller source/import loader tests in `crates/signal-auras-lua/src/sandbox.rs`.
- [X] T007 [US1] Implement `ControllerRegistration` and `ControllerRegistrationSet` in `crates/signal-auras-core/src/controller.rs`.
- [X] T008 [US1] Implement `load_lua_controller_source` and `load_lua_controller_file` in `crates/signal-auras-lua/src/sandbox.rs`.
- [X] T009 [US1] Export controller APIs from `crates/signal-auras-core/src/lib.rs` and `crates/signal-auras-lua/src/lib.rs`.

## Phase 4: User Story 2 - Run Non-Blocking Input Callbacks (Priority: P1)

**Goal**: Model bounded Lua callback scheduling with explicit dispositions.

**Independent Test**: Simulate repeated trigger scheduling and denied capability scheduling without running Lua on the input path.

- [X] T010 [P] [US2] Add callback scheduler tests in `crates/signal-auras-core/src/controller.rs`.
- [X] T011 [US2] Implement `LuaCallbackScheduler`, `LuaCallbackTask`, and callback dispositions in `crates/signal-auras-core/src/controller.rs`.

## Phase 5: User Story 3 - Use Rust-Backed Output From Lua (Priority: P2)

**Goal**: Model ordered Rust-backed output batching with capability failure closed.

**Independent Test**: Enqueue text/key requests under available capability and verify denied capability enqueues no work.

- [X] T012 [P] [US3] Add output batch ordering and denial tests in `crates/signal-auras-core/src/controller.rs`.
- [X] T013 [US3] Implement `RustOperationBatch` in `crates/signal-auras-core/src/controller.rs`.

## Phase 6: User Story 4 - Preserve Existing Config Scripts (Priority: P3)

**Goal**: Keep existing declarative Lua loader unchanged while adding controller APIs separately.

**Independent Test**: Existing Lua tests continue to compile and targeted controller tests do not modify declarative parsing.

- [X] T014 [US4] Keep existing `load_lua_source` and `load_lua_file` API behavior unchanged in `crates/signal-auras-lua/src/sandbox.rs`.

## Phase 7: Runtime Activation Integration

**Goal**: Activate validated controller programs through the CLI/runtime path while keeping OS-facing work in Rust.

**Independent Test**: Start a controller runner with a scripted callback event and verify validation, registration, scheduling, output batching, capability denial, and cleanup.

- [X] T015 [P] Add controller program callback/output tests in `crates/signal-auras-core/src/controller.rs`.
- [X] T016 [P] Add Lua controller program parsing tests in `crates/signal-auras-lua/src/sandbox.rs`.
- [X] T017 [P] Add CLI controller runner activation tests in `tests/contract/cli_runner.rs`.
- [X] T018 [US1] Implement `ControllerProgram` and callback output capability aggregation in `crates/signal-auras-core/src/controller.rs`.
- [X] T019 [US3] Implement parsed `sa.callback` and `sa.input.*` controller output APIs in `crates/signal-auras-lua/src/sandbox.rs`.
- [X] T020 [US2] Integrate `LuaCallbackScheduler` with controller runner lifecycle in `crates/signal-auras-cli/src/runner.rs`.
- [X] T021 [US3] Drain controller callback output batches through `MacroExecutor::execute_input_request` in `crates/signal-auras-cli/src/runner.rs`.
- [X] T022 [US1] Add real-adapter controller activation entry point with capability probe before registration in `crates/signal-auras-cli/src/runner.rs`.
- [X] T023 [US4] Route controller-style `signal-auras run <lua-file>` inputs to the controller activation path without changing declarative Lua loading.

## Phase 8: Polish & Verification

- [X] T024a Add embedded Lua coroutine runtime tests for sleep, window, logging, capability denial, and ordered input host requests in `tests/contract/lua_api.rs`.
- [X] T024b Add live runner tests for imperative callback execution after verified focus in `tests/contract/cli_runner.rs`.
- [X] T024c Add KDE/KWin window metadata, lookup, activation, and focus verification adapter support in `crates/signal-auras-wayland/src/kde_bridge.rs`.
- [X] T024d Add PoE2 FilterBlade controller relay example and uinput text mapping regression coverage in `examples/poe2.lua` and `crates/signal-auras-wayland/src/uinput.rs`.
- [X] T024 Run `cargo fmt` for edited Rust files.
- [X] T025 Run `cargo fmt --check`.
- [X] T026 Run `cargo test -p signal-auras-core controller`.
- [X] T027 Run `cargo test -p signal-auras-lua controller`.
- [X] T028 Run targeted controller CLI tests with `cargo test --test cli_runner controller_runner`.
- [X] T029 Run full `cargo test`.
- [X] T030 Run Nix verification commands where feasible.

**Verification note**: `examples/poe2.lua` is now a controller-style script with an imperative FilterBlade reload callback. Feature-targeted controller tests pass. `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `XDG_CACHE_HOME=/tmp/nix-cache nix flake check` pass.

## Phase 9: Architecture Review Follow-Up - Imperative Callback Responsiveness

**Goal**: Host-yielding imperative Lua callbacks remain pending work instead of blocking the runtime thread.

**Independent Test**: A callback that calls `sa.sleep(100)` is accepted, does not call `ControllerHost::sleep`, emits no later output before a timer wake, and is cancelled cleanly on shutdown.

- [X] T031 [P] [US2] Add active callback cancellation support and tests in `crates/signal-auras-core/src/controller.rs`
- [X] T032 [P] [US2] Add `sa.sleep` non-blocking/cancellation contract test in `tests/contract/cli_runner.rs`
- [X] T033 [US2] Add pending continuation queue and timer wake handling in `crates/signal-auras-cli/src/runner.rs`
- [X] T034 [US2] Update FilterBlade controller test to resume after an explicit timer wake in `tests/contract/cli_runner.rs`

## Phase 10: Architecture Review Follow-Up - Lua Sandbox/Runtime Unification

**Goal**: Controller validation and imperative callback execution share one Lua sandbox policy for denied globals, while declarative Lua compatibility keeps its existing parser fallback through the shared policy.

**Independent Test**: A controller script with `require`, `io.open`, and `debug.traceback` only in local names or strings loads successfully, while actual ambient global access is rejected by structured Lua validation.

- [X] T035 [P] [US4] Add controller sandbox parity tests in `crates/signal-auras-lua/src/sandbox.rs`
- [X] T036 [US4] Add shared denied-global sandbox policy module in `crates/signal-auras-lua/src/sandbox_policy.rs`
- [X] T037 [US4] Route `ImperativeLuaController` sandbox installation through the shared policy in `crates/signal-auras-lua/src/runtime.rs`
- [X] T038 [US4] Route controller loader validation through structured `mlua` execution with no-op registration/import APIs in `crates/signal-auras-lua/src/sandbox.rs`
- [X] T039 [US4] Keep declarative `load_lua_source` ambient API denial on the shared compatibility token list in `crates/signal-auras-lua/src/sandbox.rs`

## Phase 11: Architecture Review Follow-Up - Runtime Source-Tree Parity

**Goal**: Imperative controller runtime activation loads the same rooted `sa.import` source tree as registration/program validation.

**Independent Test**: A main controller imports a module that defines an imperative callback using `sa.sleep`; the runtime source-tree helper includes the callback, `ImperativeLuaController::load_source` accepts the resolved source with the original `sa.import` call still present, and the CLI runner schedules/resumes the imported callback.

- [X] T040 [P] [US2] Add Lua runtime source-tree helper test in `tests/contract/lua_api.rs`
- [X] T041 [P] [US2] Add CLI runner multi-file imported imperative callback test in `tests/contract/cli_runner.rs`
- [X] T042 [US2] Export a narrow rooted controller runtime source-tree helper from `crates/signal-auras-lua/src/sandbox.rs` and `crates/signal-auras-lua/src/lib.rs`
- [X] T043 [US2] Route `load_imperative_controller_runtime` in `crates/signal-auras-cli/src/runner.rs` through the resolved controller source tree
- [X] T044 [US2] Install a no-op `sa.import` in `ImperativeLuaController` runtime setup in `crates/signal-auras-lua/src/runtime.rs`

## Dependencies & Execution Order

- Setup and foundational artifacts precede implementation.
- US1 registration contracts are required before US2 scheduler and US3 output integration.
- US4 compatibility must remain true throughout all edits.
- Phase 10 depends on completed controller and imperative runtime APIs from Phases 7-9.
- Phase 11 depends on completed source-tree import loading and imperative runtime APIs from Phases 7-10.
- Verification tasks run after formatting and implementation.

## Parallel Opportunities

- T003 and T004 can run in parallel.
- T005 and T006 can run in parallel.
- T010 and T012 can run in parallel after US1 contracts exist.

## MVP Scope

US1 was the MVP: validated multi-file Lua controller registration before runtime activation. This implementation also completes library contracts for US2 and US3, wires live runner activation for imperative Lua callbacks, and includes the PoE2 FilterBlade relay as a manual KDE Wayland proof point.
