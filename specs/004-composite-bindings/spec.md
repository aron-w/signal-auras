# Feature Specification: Composite Input Bindings

**Feature Branch**: `004-composite-bindings`

**Created**: 2026-05-26

**Status**: Draft

**Input**: User description: "Composite input bindings for modifier-held mouse wheel and mouse button triggers that execute existing macros without trigger modifiers polluting output."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Mouse Wheel Rebinding (Priority: P1)

A user binds `Ctrl` plus mouse wheel up or down to keyboard navigation output for the current run.

**Why this priority**: Wheel rebinding is the smallest composite trigger that proves modifier plus pointer input can drive macros.

**Independent Test**: Load a script with two wheel bindings and verify the correct macro executes only for matching trigger events.

**Acceptance Scenarios**:

1. **Given** a current-run script with `Ctrl` plus wheel up bound to `Left`, **When** that trigger is observed, **Then** the macro emits `Left`.
2. **Given** a consumed wheel binding, **When** the provider cannot suppress the original wheel event, **Then** activation fails before registration with a diagnosable error.

---

### User Story 2 - Mouse Button Macro Trigger (Priority: P2)

A user binds `Ctrl` plus left click to a multi-action macro such as `Alt+Right`, text input, and `Enter`.

**Why this priority**: Button triggers prove the binding model supports non-wheel pointer input and existing macro sequences.

**Independent Test**: Load a script with `Ctrl` plus left click and verify the macro definition is accepted and executes in order.

**Acceptance Scenarios**:

1. **Given** `Ctrl` plus left click is bound to a macro, **When** the binding is triggered, **Then** the macro emits `Alt+Right`, the configured text, and `Enter`.
2. **Given** `Ctrl` is physically held for the trigger, **When** the macro emits `Alt+Right`, **Then** the output is treated as the requested macro output and not as `Ctrl+Alt+Right`.

---

### User Story 3 - Explicit Passthrough Mode (Priority: P3)

A user marks a binding as passthrough when the original pointer event should still reach the target application.

**Why this priority**: Some workflows need macro side effects without suppressing the original pointer action.

**Independent Test**: Load a passthrough binding and verify the trigger records passthrough behavior while still executing the macro.

**Acceptance Scenarios**:

1. **Given** a binding with `mode = "passthrough"`, **When** the trigger fires, **Then** the macro executes and the original event is not marked consumed.

### Edge Cases

- Unknown modifiers, mouse buttons, wheel directions, modes, empty triggers, and multiple primary trigger fields are rejected before registration.
- Duplicate composite triggers are rejected after modifier normalization.
- Legacy `hotkeys` scripts continue to load unchanged.
- All registrations remain current-run only and are cleaned up on shutdown or partial failure.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST support a unified binding list with structured trigger fields for modifiers, mouse input, and future keyboard input.
- **FR-002**: System MUST continue supporting existing `hotkeys` scripts without changing their Lua shape.
- **FR-003**: System MUST normalize modifier order and reject duplicate or unknown modifiers.
- **FR-004**: System MUST support `left`, `right`, and `middle` mouse buttons.
- **FR-005**: System MUST support `up` and `down` mouse wheel directions.
- **FR-006**: System MUST require exactly one primary trigger per structured binding.
- **FR-007**: System MUST default missing binding mode to `consume` and accept explicit `passthrough`.
- **FR-008**: System MUST fail closed before activation when consumed pointer events cannot be guaranteed.
- **FR-009**: System MUST execute macros as bare intended output, independent of trigger-held modifiers.
- **FR-010**: System MUST preserve current-run-only registration, scope selection, shutdown cleanup, and no hidden global behavior.

### Key Entities

- **Binding Trigger**: A normalized trigger composed of optional modifiers and one primary keyboard or pointer input.
- **Binding Mode**: The side-effect policy for the original input event: `consume` or `passthrough`.
- **Composite Binding**: A trigger, mode, scope, macro definition, and registration state.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Valid wheel and button binding scripts load successfully.
- **SC-002**: Invalid trigger schemas fail before registration with diagnosable validation errors.
- **SC-003**: Legacy hotkey scripts continue to pass existing contract tests.
- **SC-004**: Unsupported consume capability fails before activation and executes no macro actions.
- **SC-005**: Automated Rust tests cover parsing, normalization, duplicate detection, mode defaults, capability gating, lifecycle cleanup, and macro execution.
- **SC-006**: Feature verification passes with documented Nix commands.

## Assumptions

- KDE Plasma Wayland remains the only real compositor target for this increment.
- Composite pointer observation and consumption are adapter capabilities and may be unavailable in the current provider.
- Mouse support starts with wheel up/down and left/right/middle click.
- The `key` trigger field is accepted for future-compatible structured keyboard bindings.
