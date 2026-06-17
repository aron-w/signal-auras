# Tasks: Interactive Device Cache

**Input**: Design documents from `/specs/022-interactive-device-cache/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Verification**: TDD is mandatory. Rust tests precede implementation for core
provider shape, Lua parsing, device identity, cache validation, prompt decisions,
doctor diagnostics, and example loading.

## Phase 1: Setup

- [X] T001 Create feature design artifacts in `specs/022-interactive-device-cache/`

---

## Phase 2: Foundational

- [X] T002 [P] Add core interactive provider tests in `crates/signal-auras-core/src/config.rs`
- [X] T003 [P] Add Lua parser tests for `devices = "interactive"` in `crates/signal-auras-lua/src/sandbox.rs`
- [X] T004 [P] Add evdev device identity tests in `crates/signal-auras-wayland/src/evdev.rs`
- [X] T005 Add interactive cache and prompt tests in `crates/signal-auras-cli/src/input_cache.rs` and `crates/signal-auras-cli/src/prompt.rs`
- [X] T006 Implement core provider shape in `crates/signal-auras-core/src/config.rs`
- [X] T007 Implement Lua parsing for interactive providers in `crates/signal-auras-lua/src/sandbox.rs`
- [X] T008 Implement evdev device identity probing in `crates/signal-auras-wayland/src/evdev.rs`
- [X] T009 Implement CLI prompt extensions in `crates/signal-auras-cli/src/prompt.rs`
- [X] T010 Implement runtime cache library in `crates/signal-auras-cli/src/input_cache.rs`

---

## Phase 3: User Story 1 - Reuse Valid Script Device Cache (P1)

**Goal**: Valid runtime caches resolve to strict selected paths without prompting.

**Independent Test**: Simulate a valid cache and verify startup resolution returns selected paths and no prompt decision.

- [X] T011 [US1] Add valid-cache resolution tests in `crates/signal-auras-cli/src/input_cache.rs`
- [X] T012 [US1] Wire cache resolution before real runner input-provider configuration in `crates/signal-auras-cli/src/runner.rs`
- [X] T013 [US1] Add doctor valid-cache reporting tests in `crates/signal-auras-cli/src/runner.rs`

---

## Phase 4: User Story 2 - Select Devices Interactively on First Startup (P2)

**Goal**: Missing cache prompts, writes cache, and starts with selected devices.

**Independent Test**: Simulate missing cache and interactive selection; verify the cache is written and selected paths are returned.

- [X] T014 [US2] Add missing-cache prompt/write tests in `crates/signal-auras-cli/src/input_cache.rs`
- [X] T015 [US2] Implement terminal checklist selection in `crates/signal-auras-cli/src/prompt.rs`
- [X] T016 [US2] Update `examples/poe2.lua` to use `devices = "interactive"`
- [X] T017 [US2] Add PoE2 example Lua loading test in `tests/contract/lua_api.rs`

---

## Phase 5: User Story 3 - Repair Missing Permissions During Startup (P3)

**Goal**: Missing selected permissions can be repaired through explicit selected-device ACL commands.

**Independent Test**: Simulate permission-incomplete selected paths and verify the repair command is scoped to selected evdev paths and `/dev/uinput`.

- [X] T018 [US3] Add permission repair tests in `crates/signal-auras-cli/src/input_cache.rs`
- [X] T019 [US3] Implement selected-device ACL repair helper in `crates/signal-auras-cli/src/input_cache.rs`
- [X] T020 [US3] Revalidate after repair before cache write in `crates/signal-auras-cli/src/input_cache.rs`

---

## Phase 6: User Story 4 - Diagnose Cache and Device State Safely (P4)

**Goal**: `doctor input` explains interactive cache state without side effects.

**Independent Test**: Run doctor report tests for missing, valid, stale, and permission-incomplete cache states.

- [X] T021 [US4] Add interactive doctor report tests in `crates/signal-auras-cli/src/runner.rs`
- [X] T022 [US4] Implement interactive cache diagnostics in `crates/signal-auras-cli/src/runner.rs`

---

## Phase 7: Polish

- [X] T023 Update README unsafe input guidance in `README.md`
- [X] T024 Mark all completed tasks in `specs/022-interactive-device-cache/tasks.md`
- [X] T025 Run cargo formatting, tests, clippy, and Nix verification commands

## Dependencies & Execution Order

- Phase 1 before all other work.
- Phase 2 blocks all user stories.
- User Story 1 is the MVP and should complete before US2-US4 integration.
- US2 and US3 depend on foundational cache/prompt behavior.
- US4 depends on cache validation behavior but must remain read-only.

## Implementation Strategy

Implement the provider shape and cache library first, then wire only real
startup paths. Keep the live event loop unchanged after interactive resolution.
