# Tasks: Full Keyboard Key Coverage

**Input**: Design documents from `/specs/014-full-key-coverage/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Verification**: TDD is mandatory for key parsing, canonicalization, alias compatibility, duplicate detection, evdev decoding, uinput output mapping, diagnostics, discovery/no-persistence behavior, and Lua compatibility.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

## Phase 1: Setup

- [X] T001 Verify current key parsing, motion token parsing, macro key output, evdev decoding, uinput output, and doctor command entry points in `crates/signal-auras-core/src/`, `crates/signal-auras-wayland/src/`, and `crates/signal-auras-cli/src/runner.rs`
- [X] T002 [P] Confirm no new runtime dependency is needed in `Cargo.toml`, `Cargo.lock`, `flake.nix`, and `specs/014-full-key-coverage/research.md`

## Phase 2: Foundational

**Purpose**: Shared key vocabulary and canonicalization that block all user stories.

- [X] T003 Add core key vocabulary parsing, alias, category, and evdev-code lookup tests in `crates/signal-auras-core/src/key.rs`
- [X] T004 Implement generated Linux key table, `KeyToken`, canonical parsing, alias lookup, category classification, and evdev-code lookup in `crates/signal-auras-core/src/key.rs`
- [X] T005 Export the shared key module and update public re-exports in `crates/signal-auras-core/src/lib.rs`
- [X] T006 Add duplicate-detection tests for alias-equivalent keyboard triggers in `crates/signal-auras-core/src/config.rs`
- [X] T007 Update `HotkeyId`, `BindingTrigger`, `MotionToken`, and `MacroAction::key` storage/normalization to use canonical key identities in `crates/signal-auras-core/src/hotkey.rs`, `crates/signal-auras-core/src/motion.rs`, `crates/signal-auras-core/src/macro_plan.rs`, and `crates/signal-auras-core/src/config.rs`

## Phase 3: User Story 1 - Bind Any Exposed Keyboard Key (P1) MVP

**Goal**: Any standard exposed keyboard-like key can be configured as a leader, motion token, structured binding key, repeat hold token, or legacy hotkey.

**Independent Test**: Scripts using representative standard key categories validate and normalize consistently across all trigger surfaces.

- [X] T008 [P] [US1] Add Lua trigger coverage tests for leader, motions, repeat holds, structured binding keys, and hotkeys in `tests/contract/lua_api.rs`
- [X] T009 [P] [US1] Add core motion and hotkey parsing tests for function, navigation, editing, keypad, media, modifier, punctuation, and alias keys in `crates/signal-auras-core/src/motion.rs` and `crates/signal-auras-core/src/hotkey.rs`
- [X] T010 [P] [US1] Add evdev raw key decoding tests for representative standard keys and unknown/vendor codes in `crates/signal-auras-wayland/src/evdev.rs`
- [X] T011 [US1] Wire shared key parsing through Lua sandbox trigger parsing in `crates/signal-auras-lua/src/sandbox.rs`
- [X] T012 [US1] Decode evdev keyboard raw codes into canonical motion/key events and preserve unknown-code diagnostics in `crates/signal-auras-wayland/src/evdev.rs`
- [X] T013 [US1] Preserve capability planning and provider fail-closed behavior for expanded keyboard triggers in `crates/signal-auras-core/src/error.rs`, `crates/signal-auras-wayland/src/kde.rs`, and `crates/signal-auras-wayland/src/kde_bridge.rs`

## Phase 4: User Story 2 - Emit Supported Keys From Macros (P2)

**Goal**: Macro `key` actions use the same visible key names as triggers where the selected output backend supports emission.

**Independent Test**: Macros using representative standard keys emit through uinput or report unsupported-output diagnostics without substitution.

- [X] T014 [P] [US2] Add macro key parsing and canonicalization tests in `crates/signal-auras-core/src/macro_plan.rs`
- [X] T015 [P] [US2] Add uinput named-key output tests for representative navigation, editing, keypad, function, media, punctuation, and unsupported keys in `crates/signal-auras-wayland/src/uinput.rs`
- [X] T016 [P] [US2] Add runner failure tests showing unsupported macro output fails closed without substituting keys in `tests/integration/runner_flow.rs`
- [X] T017 [US2] Update uinput output mapping to emit supported canonical key tokens by evdev code in `crates/signal-auras-wayland/src/uinput.rs`
- [X] T018 [US2] Update macro execution diagnostics to report canonical token, backend, and unsupported-output reason in `crates/signal-auras-wayland/src/diagnostics.rs` and `crates/signal-auras-cli/src/runner.rs`

## Phase 5: User Story 3 - Discover Key Names Safely (P3)

**Goal**: An explicit current-run doctor command reports observed key codes, canonical names, aliases, triggerability, emittability, and unavailable reasons without persistence.

**Independent Test**: Simulated doctor runs report known, unknown, unsupported, denied, and unobserved key cases and retain no state between runs.

- [X] T019 [P] [US3] Add key discovery report formatting tests in `crates/signal-auras-cli/src/runner.rs`
- [X] T020 [P] [US3] Add CLI contract tests for `doctor keys <lua-file>` argument handling and fail-closed permission paths in `tests/contract/cli_runner.rs`
- [X] T021 [P] [US3] Add no-persistence tests for repeated key discovery runs in `crates/signal-auras-cli/src/runner.rs`
- [X] T022 [US3] Implement current-run key discovery report types and rendering in `crates/signal-auras-cli/src/runner.rs`
- [X] T023 [US3] Add `doctor keys <lua-file>` command routing without changing `doctor input` passive behavior in `crates/signal-auras-cli/src/main.rs` and `crates/signal-auras-cli/src/runner.rs`
- [X] T024 [US3] Document manual Keychron K5 Pro discovery verification in `tests/compositor/manual-wayland-verification.md`

## Phase 6: User Story 4 - Preserve Existing Lua Key Names (P4)

**Goal**: Existing scripts continue to load while canonical names and aliases are visible in diagnostics and docs.

**Independent Test**: Existing examples and alias cases validate unchanged, and alias-equivalent triggers are treated as duplicates.

- [X] T025 [P] [US4] Add regression tests for legacy aliases and existing examples in `tests/contract/lua_api.rs`
- [X] T026 [P] [US4] Add diagnostic tests showing canonical names first and aliases where useful in `crates/signal-auras-cli/src/runner.rs`
- [X] T027 [US4] Update README key naming and discovery documentation in `README.md`
- [X] T028 [US4] Update Lua editor metadata for expanded key action names if needed in `lua-types/signal-auras.lua`

## Phase 7: Polish and Verification

- [X] T029 [P] Run `nix develop -c cargo fmt --check`
- [X] T030 Run `nix develop -c cargo clippy --all-targets -- -D warnings`
- [X] T031 Run `nix develop -c cargo test`
- [X] T032 Run `nix flake check` when feasible
- [X] T033 Review `specs/014-full-key-coverage/quickstart.md` against implemented behavior and update if command names or diagnostics changed

## Dependencies and Order

- Setup and foundational tasks block all user stories.
- US1 depends on the foundational core key identity model.
- US2 depends on foundational macro key identity and can proceed after T004/T005/T007.
- US3 depends on key identity and evdev/output support helpers from US1/US2.
- US4 can run after foundational aliases exist and should be reconciled after US1/US2 diagnostics.
- Verification tasks in each story precede implementation tasks.

## Parallel Opportunities

- T002 can run after T001 without source edits.
- T008, T009, and T010 can be drafted in parallel because they touch different test files/modules.
- T014, T015, and T016 can be drafted in parallel.
- T019, T020, and T021 can be drafted in parallel.
- T025 and T026 can be drafted in parallel.

## Implementation Strategy

Deliver the shared key vocabulary first. Then implement US1 as the MVP for full trigger parsing and evdev decoding, US2 for macro output, US3 for safe discovery diagnostics, and US4 for compatibility/docs. Keep all behavior current-run only, use core library contracts before adapter/CLI wiring, and avoid new runtime dependencies unless planning is updated with a constitution-compliant justification.
