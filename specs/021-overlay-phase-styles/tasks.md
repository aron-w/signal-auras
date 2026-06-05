# Tasks: Overlay Phase Styles

**Input**: Design documents from `/specs/021-overlay-phase-styles/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Verification**: TDD is mandatory. Add failing tests for Rust core overlay mapping and Lua parser validation before implementation.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

## Phase 1: Setup

- [X] T001 Confirm `.specify/memory/constitution.md`, `specs/017-poe2-state-tracking/plan.md`, `specs/018-overlay-render-providers/plan.md`, and `specs/021-overlay-phase-styles/plan.md` are read before edits

---

## Phase 2: Foundational

- [X] T002 [P] Add contract documentation for detector-vs-overlay ownership in `specs/017-poe2-state-tracking/contracts/lua-state-api.md`
- [X] T003 [P] Add contract documentation for `activated` and `active` overlay styles in `specs/018-overlay-render-providers/contracts/lua-overlay-api.md`

---

## Phase 3: User Story 1 - Tracker Language Stays Observational (Priority: P1)

**Goal**: Detector phase rules reject visual style fields.

**Independent Test**: Lua parser rejects `fill`, `background`, or `opacity` inside radial detector phase rules and still accepts recognition-only rules.

### Verification

- [X] T004 [US1] Add Lua parser tests for rejecting detector phase style fields in `crates/signal-auras-lua/src/sandbox.rs`

### Implementation

- [X] T005 [US1] Reject `fill`, `background`, and `opacity` in radial phase parsing in `crates/signal-auras-lua/src/sandbox.rs`
- [X] T006 [US1] Remove unused radial phase style fields from core detector data in `crates/signal-auras-core/src/screen_state.rs`

---

## Phase 4: User Story 2 - Overlay Owns Phase Presentation (Priority: P2)

**Goal**: Overlay progress bars can style radial `activated` and `active` phases.

**Independent Test**: Core overlay snapshots apply configured `activated` and `active` style overrides while preserving defaults when absent.

### Verification

- [X] T007 [US2] Add core overlay tests for radial `activated` and `active` style overrides in `crates/signal-auras-core/src/overlay.rs`
- [X] T008 [US2] Add Lua parser tests for `activated` and `active` visual styles in `crates/signal-auras-lua/src/sandbox.rs`

### Implementation

- [X] T009 [US2] Extend progress-bar visual style model and radial snapshot mapping in `crates/signal-auras-core/src/overlay.rs`
- [X] T010 [US2] Parse `activated` and `active` visual styles in `crates/signal-auras-lua/src/sandbox.rs`

---

## Phase 5: User Story 3 - PoE2 Example Reads as the Intended Contract (Priority: P3)

**Goal**: The PoE2 example demonstrates the tracker/overlay split.

**Independent Test**: `examples/poe2.lua` validates and contains phase styling only in the overlay visual.

### Verification

- [X] T011 [US3] Add or update PoE2 Lua contract test coverage in `tests/contract/lua_api.rs`

### Implementation

- [X] T012 [US3] Move Refutation phase styles from detector phases to overlay visual styles in `examples/poe2.lua`

---

## Final Phase: Polish & Validation

- [X] T013 Run `cargo test -p signal-auras-core overlay`
- [X] T014 Run `cargo test -p signal-auras-lua overlay`
- [X] T015 Run `cargo test --test lua_api poe2`
- [X] T016 Mark completed tasks in `specs/021-overlay-phase-styles/tasks.md`

## Dependencies & Execution Order

- Phase 1 before all edits.
- Phase 2 can run before or after implementation but must finish before final review.
- US1 and US2 both touch `sandbox.rs`, so execute them sequentially.
- US3 depends on US1 and US2 parser/model support.
- Final validation depends on all selected user stories.

## Implementation Strategy

1. Complete US1 to enforce recognition-only detector rules.
2. Complete US2 to give presentation a clean overlay-owned home.
3. Complete US3 to update the public example.
4. Run targeted checks and review the diff for API boundary clarity.
