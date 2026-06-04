# Tasks: Overlay Render Providers

**Input**: Design documents from `/specs/018-overlay-render-providers/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Verification**: TDD is mandatory. Every user story includes failing tests before implementation for Rust library or Lua parsing behavior. Manual compositor verification is documented for real KDE/PoE2 overlay surface behavior because compositor pass-through cannot be fully exercised in the current automated test harness.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Establish feature files and exports used by all stories.

- [X] T001 Add overlay module declaration and public exports in `crates/signal-auras-core/src/lib.rs`
- [X] T002 Add Wayland overlay adapter module placeholder and export in `crates/signal-auras-wayland/src/lib.rs`
- [X] T003 [P] Add overlay manual verification section to `tests/compositor/manual-wayland-verification.md`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core model and parser foundation that blocks all user stories.

- [X] T004 Add failing core overlay validation tests in `tests/contract/rust_library.rs`
- [X] T005 Add failing Lua overlay parser tests in `tests/contract/lua_api.rs`
- [X] T006 Implement provider-neutral overlay definitions, visuals, styles, state bindings, lifecycle states, and definition-set validation in `crates/signal-auras-core/src/overlay.rs`
- [X] T007 Integrate overlay definitions into `ControllerProgram` in `crates/signal-auras-core/src/controller.rs`
- [X] T008 Parse `sa.overlay.mount(...)` declarations in `crates/signal-auras-lua/src/sandbox.rs`

**Checkpoint**: Foundation ready - overlay definitions validate and are available from controller programs.

---

## Phase 3: User Story 1 - Show PoE2 Tracker Bars (Priority: P1) MVP

**Goal**: Convert typed PoE2 tracker states into native-provider progress-bar snapshots gated by trusted focus and fresh state.

**Independent Test**: Simulate trusted focus and typed tracker snapshots for Heavy Stun and Refutation, then verify active snapshots, ready style, inactive focus behavior, stale/missing source behavior, and no input/macro side effects.

### Verification for User Story 1

- [X] T009 [P] [US1] Add failing state-to-visual mapping tests for Heavy Stun and Refutation in `tests/contract/rust_library.rs`
- [X] T010 [P] [US1] Add failing inactive focus, stale source, and missing source tests in `tests/contract/rust_library.rs`
- [X] T011 [US1] Add overlay declaration to `examples/poe2.lua` without overwriting existing user edits

### Implementation for User Story 1

- [X] T012 [US1] Implement overlay snapshot mapping from `TrackerState` values in `crates/signal-auras-core/src/overlay.rs`
- [X] T013 [US1] Implement focus/capability/source gating diagnostics for overlay snapshots in `crates/signal-auras-core/src/overlay.rs`
- [X] T014 [US1] Expose overlay snapshots through controller program state in `crates/signal-auras-core/src/controller.rs`

**Checkpoint**: User Story 1 is independently testable with simulated tracker states.

---

## Phase 4: User Story 2 - Declare Provider-Neutral Overlays (Priority: P2)

**Goal**: Validate provider selection and declaration shape independently from any one renderer.

**Independent Test**: Parse valid and invalid `sa.overlay.mount(...)` declarations, including provider names, duplicate visual ids, invalid rectangles, invalid opacity, missing bindings, and future provider fallback diagnostics.

### Verification for User Story 2

- [X] T015 [P] [US2] Add failing Lua validation tests for invalid provider, duplicate visual ids, invalid rectangles, invalid opacity, and missing bindings in `tests/contract/lua_api.rs`
- [X] T016 [P] [US2] Add failing provider selection and unavailable-provider diagnostics tests in `tests/contract/rust_library.rs`

### Implementation for User Story 2

- [X] T017 [US2] Complete Lua overlay field parsing and validation in `crates/signal-auras-lua/src/sandbox.rs`
- [X] T018 [US2] Implement provider availability and future-provider unavailable diagnostics in `crates/signal-auras-core/src/overlay.rs`
- [X] T019 [US2] Add native in-memory renderer and unavailable real-provider placeholder in `crates/signal-auras-wayland/src/overlay.rs`

**Checkpoint**: User Stories 1 and 2 validate provider-neutral declarations and diagnostics.

---

## Phase 5: User Story 3 - Keep Overlay Rendering Isolated (Priority: P3)

**Goal**: Prove overlays cannot access or trigger sensitive automation behavior and future UI providers receive sanitized snapshots only.

**Independent Test**: Attempt forbidden Lua/provider access and verify snapshots contain only sanitized render data while callbacks/macros/input remain untouched.

### Verification for User Story 3

- [X] T020 [P] [US3] Add failing Lua sandbox tests for raw screen/input/compositor/provider escape attempts in `tests/contract/lua_api.rs`
- [X] T021 [P] [US3] Add failing sanitized snapshot and no macro/input side-effect tests in `tests/contract/rust_library.rs`

### Implementation for User Story 3

- [X] T022 [US3] Enforce overlay forbidden fields and sanitized snapshot contents in `crates/signal-auras-core/src/overlay.rs`
- [X] T023 [US3] Ensure Lua parser rejects overlay callback, macro, screen, input, compositor, filesystem, and network authority fields in `crates/signal-auras-lua/src/sandbox.rs`
- [X] T024 [US3] Add cleanup/hide lifecycle behavior to the in-memory renderer in `crates/signal-auras-wayland/src/overlay.rs`

**Checkpoint**: All user stories are independently functional.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Finish documentation and verification.

- [X] T025 [P] Update `specs/018-overlay-render-providers/quickstart.md` if implementation commands or example shape changed
- [X] T026 Run `cargo fmt --check`
- [X] T027 Run targeted overlay tests with `cargo test overlay`
- [X] T028 Run full `cargo test`
- [X] T029 Run `cargo clippy --all-targets -- -D warnings`
- [X] T030 Run Nix verification commands where feasible and record any unavailable checks in final notes

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies.
- **Foundational (Phase 2)**: Depends on Setup completion and blocks all user stories.
- **User Story 1 (Phase 3)**: Depends on Foundational completion.
- **User Story 2 (Phase 4)**: Depends on Foundational completion; can run after US1 or in parallel with care.
- **User Story 3 (Phase 5)**: Depends on Foundational completion; uses snapshots and parser behavior from US1/US2.
- **Polish (Phase 6)**: Depends on all selected stories.

### User Story Dependencies

- **US1**: MVP, no dependency on US2/US3 after foundation.
- **US2**: Provider declaration and diagnostics can be tested independently after foundation.
- **US3**: Security isolation builds on the same definitions and snapshots but remains independently testable through sanitized outputs and parser rejection.

### Parallel Opportunities

- T003 can run in parallel with T001/T002.
- T004 and T005 can be written in parallel before implementation.
- T009/T010 and T015/T016 and T020/T021 are file-separated test additions when coordinated.
- Documentation update T025 can run in parallel with final verification commands after code stabilizes.

---

## Parallel Example: User Story 1

```bash
# Verification first
cargo test --test rust_library overlay_state
cargo test --test rust_library overlay_inactive

# Then implementation
# Edit crates/signal-auras-core/src/overlay.rs
# Edit crates/signal-auras-core/src/controller.rs
```

---

## Implementation Strategy

### MVP First

1. Complete setup and foundational parser/model work.
2. Implement US1 state-to-progress-bar mapping for Heavy Stun and Refutation.
3. Validate US1 with simulated tracker states before adding broader provider diagnostics.

### Incremental Delivery

1. US1: typed PoE2 bars from tracker states.
2. US2: provider-neutral declaration validation and provider diagnostics.
3. US3: security isolation and sanitized renderer boundaries.
4. Polish: example, docs, formatting, tests, clippy, and Nix verification.
