# Tasks: Robust Device Selection

**Input**: Design documents from `/specs/011-robust-device-selection/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Verification**: TDD is mandatory for selection policy, doctor diagnostics, and
runtime failure modes. Manual compositor verification is supplemental because
real evdev/uinput access depends on host permissions.

## Phase 1: Setup

- [X] T001 Verify branch, main integration, checklist status, and existing evdev/doctor code paths in `specs/011-robust-device-selection/` and `crates/`
- [X] T002 [P] Generate planning artifacts in `specs/011-robust-device-selection/plan.md`, `research.md`, `data-model.md`, `contracts/`, and `quickstart.md`

## Phase 2: Foundational

- [X] T003 Add shared device eligibility/diagnostic helpers for selected and discovered paths in `crates/signal-auras-wayland/src/evdev.rs`
- [X] T004 Add shared doctor probe/status helpers in `crates/signal-auras-cli/src/runner.rs`

## Phase 3: User Story 1 - Use Selected Stable Devices Predictably (P1)

**Goal**: Explicit selected paths observe only configured usable devices and
report unusable selected paths without fallback.

**Independent Test**: Unit tests open selected stable paths, duplicate paths,
missing paths, and own virtual devices and verify strict selection behavior.

- [X] T005 [P] [US1] Add selected-path strictness tests in `crates/signal-auras-wayland/src/evdev.rs`
- [X] T006 [P] [US1] Add selected-path doctor tests in `crates/signal-auras-cli/src/runner.rs`
- [X] T007 [US1] Implement selected-path deduplication, strict no-fallback policy, and selected diagnostics in `crates/signal-auras-wayland/src/evdev.rs`
- [X] T008 [US1] Implement selected stable-path guidance and duplicate diagnostics in `crates/signal-auras-cli/src/runner.rs`

## Phase 4: User Story 2 - Start With `devices = "all"` Despite Unreadable or Noisy Devices (P2)

**Goal**: Broad discovery skips bad candidates, starts with eligible devices,
and fails closed only when none are usable.

**Independent Test**: Mixed startup tests simulate readable, missing/unreadable,
unsupported/noisy, and self-generated candidates.

- [X] T009 [P] [US2] Add mixed `devices = "all"` startup tests in `crates/signal-auras-wayland/src/evdev.rs`
- [X] T010 [P] [US2] Add noisy unsupported-event fairness tests in `crates/signal-auras-wayland/src/evdev.rs`
- [X] T011 [US2] Implement tolerant all-device opening and no-usable-devices errors in `crates/signal-auras-wayland/src/evdev.rs`
- [X] T012 [US2] Ensure runtime summaries and adapter input-provider summaries remain accurate in `crates/signal-auras-wayland/src/adapter.rs`

## Phase 5: User Story 3 - Recover From Hotplug and Reopen Conditions (P3)

**Goal**: Selected and broad modes report removal/reopen transitions and keep
remaining eligible input active.

**Independent Test**: Simulated providers mark devices inactive, rescan selected
or discovered paths, and verify only allowed paths reopen.

- [X] T013 [P] [US3] Add selected and broad hotplug/reopen tests in `crates/signal-auras-wayland/src/evdev.rs`
- [X] T014 [US3] Implement inactive-device replacement without duplicate active handles in `crates/signal-auras-wayland/src/evdev.rs`
- [X] T015 [US3] Preserve current-run-only hotplug behavior and diagnostics in `crates/signal-auras-wayland/src/evdev.rs`

## Phase 6: User Story 4 - Diagnose Device Selection Safely (P4)

**Goal**: `doctor input` explains configured and discovered selection state
without enabling observation.

**Independent Test**: Doctor unit tests use fake probes for selected,
duplicated, permission-denied, all-device, stable-path, and own-device cases.

- [X] T016 [P] [US4] Add doctor diagnostic coverage in `crates/signal-auras-cli/src/runner.rs`
- [X] T017 [US4] Implement doctor status rendering for stable paths, duplicates, self-generated devices, all-device guidance, and remediation in `crates/signal-auras-cli/src/runner.rs`

## Phase 7: Polish and Verification

- [X] T018 [P] Update manual verification notes in `tests/compositor/manual-wayland-verification.md` if live-only gaps remain
- [X] T019 Run `cargo fmt --check` or `nix develop -c cargo fmt --check`
- [X] T020 Run `cargo clippy --all-targets -- -D warnings` or Nix equivalent
- [X] T021 Run `cargo test` or Nix equivalent
- [X] T022 Run `nix flake check` when feasible

## Dependencies and Order

- Setup and foundational tasks block all user stories.
- Implement user stories in priority order: US1, then US2, then US3, then US4.
- Test tasks for each story precede implementation tasks.
- Polish and verification depend on all stories.

## Parallel Opportunities

- T002 can run after T001 without touching source code.
- Test additions in each story can be drafted independently before matching
  implementation.
- Documentation/manual verification updates can run after code behavior is
  complete.

## Implementation Strategy

Deliver US1 first as the MVP because it preserves least-privilege selected
device behavior. Then harden broad discovery, hotplug/reopen, and read-only
doctor diagnostics without changing the Lua API or adding persistent state.
