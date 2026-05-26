# Tasks: Lua Editor Tooling for Signal Auras Scripts

**Input**: Design documents from `/specs/003-lua-editor-tooling/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Verification**: TDD is mandatory for runtime library behavior. This feature does not add runtime library behavior, so verification is based on editor metadata checks, Nix tool availability, documentation, and existing runtime tests.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Establish feature documentation and workspace context.

- [x] T001 Create Speckit feature artifacts in specs/003-lua-editor-tooling/
- [x] T002 Update AGENTS.md to reference specs/003-lua-editor-tooling/plan.md

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Define editor-only boundaries before user story implementation.

- [x] T003 [P] Confirm runtime crates are not required for this editor-only feature in specs/003-lua-editor-tooling/plan.md
- [x] T004 [P] Document the editor-only Lua DSL contract in specs/003-lua-editor-tooling/contracts/lua-editor-api.md

**Checkpoint**: Foundation ready - user story implementation can now begin.

---

## Phase 3: User Story 1 - Edit Signal Auras Scripts Without False DSL Diagnostics (Priority: P1) MVP

**Goal**: Existing Signal Auras Lua examples no longer show false undefined-global diagnostics for valid DSL globals.

**Independent Test**: Check both existing examples through LuaLS/Neovim and confirm `macro`, `key`, `text`, and `delay` are accepted while unrelated diagnostics remain enabled.

### Verification for User Story 1

- [x] T005 [P] [US1] Add workspace LuaLS verification notes for examples/poe2-hideout.lua and examples/prompt-scope.lua in specs/003-lua-editor-tooling/quickstart.md

### Implementation for User Story 1

- [x] T006 [US1] Add LuaLS workspace configuration in .luarc.json for Lua 5.4 and Signal Auras DSL globals
- [x] T014 [US1] Normalize existing example scripts to valid Lua delay call syntax in examples/poe2-hideout.lua and examples/prompt-scope.lua

**Checkpoint**: User Story 1 is independently verifiable with editor diagnostics.

---

## Phase 4: User Story 2 - Discover the Lua DSL Shape in the Editor (Priority: P2)

**Goal**: The editor exposes the supported Signal Auras Lua DSL call shapes without runtime changes.

**Independent Test**: Inspect the editor metadata file and confirm all four DSL globals are documented with arguments and return placeholders.

### Verification for User Story 2

- [x] T007 [P] [US2] Verify the DSL stub describes macro, key, text, and delay in lua-types/signal-auras.lua

### Implementation for User Story 2

- [x] T008 [US2] Add editor-only EmmyLua metadata stub in lua-types/signal-auras.lua

**Checkpoint**: User Story 2 is independently verifiable by editor metadata inspection.

---

## Phase 5: User Story 3 - Reproduce Lua Editor Tooling Through Nix (Priority: P3)

**Goal**: Contributors can obtain LuaLS through the project dev shell.

**Independent Test**: Run `nix develop -c lua-language-server --version` from the repository root.

### Verification for User Story 3

- [x] T009 [P] [US3] Document the LuaLS dev-shell availability check in specs/003-lua-editor-tooling/quickstart.md

### Implementation for User Story 3

- [x] T010 [US3] Add lua-language-server to the flake dev shell in flake.nix

**Checkpoint**: User Story 3 is independently verifiable through Nix.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Validate the complete feature and ensure no runtime behavior changed.

- [x] T011 Run `nix develop -c lua-language-server --version`
- [x] T012 Run `nix develop -c cargo test`
- [x] T013 Review git diff to confirm no runtime Lua sandbox automation semantics changed
- [x] T015 Run `nix develop -c lua-language-server --check=. --check_format=pretty --checklevel=Information --logpath=/tmp/signal-auras-luals-log`
- [x] T016 Run `nix develop -c cargo test -p signal-auras-lua`
- [x] T017 Isolate existing KDE unsupported-capability contract test from the live desktop environment in tests/contract/cli_runner.rs

**Validation Note**: T012 initially exposed that `real_runner_fails_before_registration_when_global_shortcut_capability_is_unsupported` depended on the live desktop environment through `RealWaylandAdapter::new()`. The test now uses an explicit KDE environment with missing KGlobalAccel service, and the full suite passes.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies.
- **Foundational (Phase 2)**: Depends on Setup completion.
- **User Stories**: Depend on Foundational completion.
- **Polish**: Depends on all implemented user stories.

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational.
- **User Story 2 (P2)**: Can start after Foundational and complements US1.
- **User Story 3 (P3)**: Can start after Foundational.

### Parallel Opportunities

- T003 and T004 can run in parallel.
- T005, T007, and T009 touch separate verification/documentation concerns.
- US1, US2, and US3 implementation touches separate files and can be reviewed independently.

## Implementation Strategy

1. Deliver US1 first for the MVP editor diagnostic fix.
2. Add US2 so the DSL is discoverable in editors.
3. Add US3 to make the tooling reproducible through Nix.
4. Run the documented checks and verify runtime files remain untouched.
