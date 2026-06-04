# Tasks: PoE2 Screen State Tracking

**Input**: Design documents from `/specs/017-poe2-state-tracking/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Verification**: TDD is mandatory for Rust library behavior, Lua capability enforcement, and tracker polling gates.

## Phase 1: Setup

- [X] T001 Create `specs/017-poe2-state-tracking/` artifacts from the supplied feature specification.
- [X] T002 [P] Update `AGENTS.md` to reference `specs/017-poe2-state-tracking/plan.md`.

## Phase 2: Foundational

- [X] T003 [P] Define Rust tracker contract in `specs/017-poe2-state-tracking/contracts/state-tracker-library.md`.
- [X] T004 [P] Define Lua state API contract in `specs/017-poe2-state-tracking/contracts/lua-state-api.md`.

## Phase 3: User Story 1 - Track Refutation Cooldown (Priority: P1)

**Goal**: Detect and estimate Refutation radial cooldown state from fixture-backed samples.

**Independent Test**: Run detector tests against `examples/poe2/refutation_cooldown.webm` and verify ready, fraction, remaining, total estimate, and confidence behavior.

- [X] T005 [P] [US1] Add radial cooldown detector and history tests in `crates/signal-auras-core/src/screen_state.rs`.
- [X] T006 [US1] Implement radial cooldown detector config, state, and cooldown estimation in `crates/signal-auras-core/src/screen_state.rs`.
- [X] T007 [US1] Add Refutation fixture regression test in `tests/contract/rust_library.rs`.

## Phase 4: User Story 2 - Track Heavy Stun Progress (Priority: P2)

**Goal**: Detect Heavy Stun bar visibility and progress from fixture-backed samples.

**Independent Test**: Run detector tests against `examples/poe2/progress_heavy_stun.webm` at 50 ms sample intervals and verify visible/progress/confidence behavior.

- [X] T008 [P] [US2] Add horizontal progress detector tests in `crates/signal-auras-core/src/screen_state.rs`.
- [X] T009 [US2] Implement horizontal progress detector config and state in `crates/signal-auras-core/src/screen_state.rs`.
- [X] T010 [US2] Add Heavy Stun fixture regression test in `tests/contract/rust_library.rs`.

## Phase 5: User Story 3 - Register State Trackers Safely (Priority: P3)

**Goal**: Let Lua register passive state trackers without callbacks, input capture, or screen reads during startup.

**Independent Test**: Load `examples/poe2.lua` and verify the two trackers validate, require `screen_read`, and do not add callbacks or synthesized input requirements for tracker behavior.

- [X] T011 [P] [US3] Add Lua parser tests for `sa.state.track` in `crates/signal-auras-lua/src/sandbox.rs`.
- [X] T012 [P] [US3] Add Lua API contract tests for PoE2 trackers in `tests/contract/lua_api.rs`.
- [X] T013 [US3] Implement `StateTrackerDefinitionSet` integration with `ControllerProgram` in `crates/signal-auras-core/src/controller.rs`.
- [X] T014 [US3] Implement `sa.state.track` parsing and validation in `crates/signal-auras-lua/src/sandbox.rs`.
- [X] T015 [US3] Add passive tracker definitions to `examples/poe2.lua`.

## Phase 6: Runtime Gating & Diagnostics

**Goal**: Model fail-closed polling and shared screen samples without implementing a compositor capture stack in this increment.

- [X] T016 [P] Add poller capability/focus/batching tests in `crates/signal-auras-core/src/screen_state.rs`.
- [X] T017 Implement `StateTrackerPoller` denied/inactive/shared-sample behavior in `crates/signal-auras-core/src/screen_state.rs`.
- [X] T018 Add `screen_read` capability reporting support in `crates/signal-auras-core/src/error.rs` and `crates/signal-auras-wayland/src/capability.rs`.

## Phase 7: Polish & Verification

- [X] T019 Export screen state APIs from `crates/signal-auras-core/src/lib.rs` and expose parsed trackers through the Lua-loaded `ControllerProgram`.
- [X] T020 Run `cargo fmt` for edited Rust files.
- [X] T021 Run `cargo test -p signal-auras-core screen_state`.
- [X] T022 Run `cargo test --test lua_api state_trackers`.
- [X] T023 Run `cargo test --test rust_library poe2_screen_state`.
- [X] T024 Run full `cargo test`.
- [X] T025 Run `cargo clippy --all-targets -- -D warnings`.
- [X] T026 Run Nix verification commands where feasible.

**Verification note**: `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `XDG_CACHE_HOME=/tmp/nix-cache nix develop -c cargo fmt --check`, `XDG_CACHE_HOME=/tmp/nix-cache nix develop -c cargo test`, `XDG_CACHE_HOME=/tmp/nix-cache nix develop -c cargo clippy --all-targets -- -D warnings`, and `XDG_CACHE_HOME=/tmp/nix-cache nix flake check` pass. `nix fmt` is unavailable because this flake does not define `formatter.x86_64-linux`.

## Dependencies & Execution Order

- Phase 1 and Phase 2 precede implementation.
- US1 is MVP and can be verified independently.
- US2 depends only on foundational detector types.
- US3 depends on tracker definitions from core.
- Runtime gating depends on validated tracker definitions.

## Parallel Opportunities

- T002, T003, and T004 can run in parallel.
- T005 and T008 can run in parallel after foundational types are present.
- T011, T012, and T016 can run in parallel after tracker definitions exist.

## MVP Scope

US1 is the MVP: a Rust-owned Refutation radial cooldown tracker with fixture-backed tests and estimated remaining cooldown. The completed feature also includes Heavy Stun progress, passive Lua registration, and fail-closed polling diagnostics.
