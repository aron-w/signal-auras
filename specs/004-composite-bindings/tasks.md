# Tasks: Composite Input Bindings

## Phase 1: Core Model

- [x] Add modifier, mouse trigger, binding trigger, and binding mode data types.
- [x] Normalize modifier order and reject duplicate or unknown modifiers.
- [x] Convert legacy `hotkeys` into the unified binding model.
- [x] Reject duplicate normalized triggers.

## Phase 2: Lua Contract

- [x] Parse structured `bindings` in addition to legacy `hotkeys`.
- [x] Validate trigger shape, mouse values, modifiers, modes, and macro presence.
- [x] Add Lua contract tests for composite wheel and button bindings.

## Phase 3: Runner and Adapter Contract

- [x] Route keyboard and composite trigger events through one lifecycle path.
- [x] Track consume and passthrough trigger behavior in runtime stats.
- [x] Add composite pointer observation and consumption capability kinds.
- [x] Fail closed before activation when consumed pointer support is unavailable.
- [x] Keep shutdown and partial-failure cleanup behavior.

## Phase 4: Documentation and Verification

- [x] Update Lua API documentation and editor stub.
- [x] Add SpecKit artifacts for the feature.
- [x] Run Nix verification commands.
- [ ] Record manual KDE Wayland verification when a real provider exists.
