# Feature Specification: Unified Input Motions

**Feature Branch**: `005-input-motions`

**Created**: 2026-05-26

**Status**: Draft

**Input**: User description: "Reframe leader sequences, mouse sequences, and repeat behavior as unified input motions with one trigger notation and explicit repeat ownership."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Leader Keyboard Motion (Priority: P1)

A user defines a leader sequence such as `<Leader> f f` that emits an existing macro.

**Why this priority**: Keyboard motions prove the new notation can express common leader workflows while preserving existing macro behavior.

**Independent Test**: Load a Lua script with `leader = "F13"` and `trigger = { "<Leader>", "f", "f" }`; verify validation accepts the sequence and the motion macro is stored without trigger keys leaking into generated output.

**Acceptance Scenarios**:

1. **Given** a script with `<Leader> f f`, **When** the script loads, **Then** the motion is accepted as one logical trigger sequence.
2. **Given** `mode = "consume"`, **When** the motion executes through a provider that supports consumption, **Then** trigger inputs are suppressed and only the macro output is emitted.

---

### User Story 2 - Mouse Hold Repeat Motion (Priority: P2)

A user defines `<Leader> <LClick> <LClick>` where the second click can remain held and start repeat clicking while `<Leader>` and `<LClick>` remain held.

**Why this priority**: This is the behavior that requires motions to own repeat state instead of modeling hold as a separate flag.

**Independent Test**: Load a Lua script with a passthrough mouse trigger and `repeat.while_held = { "<Leader>", "<LClick>" }`; verify repeat interval, held-state requirements, and emitted repeat macro are validated.

**Acceptance Scenarios**:

1. **Given** `<Leader> <LClick> <LClick>` and `while_held = { "<Leader>", "<LClick>" }`, **When** the second click remains physically down, **Then** the trigger may complete and enter repeat state.
2. **Given** repeat is active, **When** any `while_held` input is released, **Then** repeat stops before emitting further actions.

---

### User Story 3 - Delay Precedence (Priority: P3)

A user configures default and per-motion generated-action delays while retaining explicit macro `delay(ms)` actions.

**Why this priority**: Delay behavior affects all generated macro output and must be deterministic before real repeat scheduling is added.

**Independent Test**: Load scripts with global defaults and motion overrides; verify resolved motion delays and rejection of negative delays.

**Acceptance Scenarios**:

1. **Given** `defaults.inter_action_delay_ms = 10`, **When** a motion omits a local override, **Then** generated actions use 10 ms between actions.
2. **Given** a motion sets `inter_action_delay_ms = 25`, **When** the motion is validated, **Then** 25 ms overrides the global default for that motion.
3. **Given** a macro contains `delay(50)`, **When** generated actions are executed, **Then** the explicit delay remains an action in sequence.

### Edge Cases

- Invalid motion tokens, empty triggers, duplicate motion triggers, malformed repeat intervals, repeat definitions without a macro, and negative delay values are rejected before registration.
- `F13` is treated as an example leader key only; scripts choose the concrete leader token explicitly.
- `<Leader> f f` and `<Leader> <LClick> <LClick>` use the same list notation.
- Existing `hotkeys` and structured `bindings` remain backward compatible.
- Motions remain current-run only and inherit explicit scope, consent, revocation, and no-hidden-global-behavior constraints.
- Required input observation, input consumption, and synthesized input capability gaps fail closed with diagnosable errors.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST support `motions = { ... }` as a list of motion definitions.
- **FR-002**: Each motion MUST have `trigger = { ... }` with one or more uniform tokens.
- **FR-003**: Supported tokens MUST include `<Leader>`, printable keys, function keys, `<LClick>`, `<RClick>`, `<MClick>`, `<WheelUp>`, and `<WheelDown>`.
- **FR-004**: Each motion MUST support `mode = "consume"` and `mode = "passthrough"`, defaulting to `consume`.
- **FR-005**: Each motion MUST define either `macro` or `repeat`; a repeat MUST define `while_held`, `interval_ms`, and an emitted macro.
- **FR-006**: Repeat intervals MUST reject zero, negative, and `min > max` values.
- **FR-007**: The final matching mouse token MAY remain physically down and satisfy `repeat.while_held`.
- **FR-008**: `defaults.inter_action_delay_ms` MUST apply between generated macro actions, and `motion.inter_action_delay_ms` MUST override it.
- **FR-009**: Explicit `delay(ms)` macro actions MUST remain supported independently of inter-action delays.
- **FR-010**: Motion validation MUST reject duplicate normalized triggers.
- **FR-011**: Existing `hotkeys` and `bindings` scripts MUST continue to load unchanged.
- **FR-012**: Missing observation, consumption, or synthesis capabilities MUST fail closed before activation.

### Key Entities

- **Motion Token**: A normalized input token such as `<Leader>`, a key, or a mouse button.
- **Motion Trigger**: Ordered token sequence that activates one motion.
- **Motion Definition**: Trigger, event mode, optional macro, optional repeat behavior, and resolved inter-action delay.
- **Repeat Definition**: Held-state requirements, interval range, emitted macro, and cancellation rule.
- **Automation Defaults**: Global defaults for generated macro behavior.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Valid keyboard and mouse motion scripts load successfully.
- **SC-002**: Invalid motion schema cases fail with script-validation diagnostics.
- **SC-003**: Existing hotkey and binding contract tests continue to pass.
- **SC-004**: Capability planning includes input observation, consumption when needed, and synthesized input for emitted macros.
- **SC-005**: Automated Rust tests cover token parsing, Lua motion parsing, duplicate detection, repeat validation, delay precedence, and backward compatibility.
- **SC-006**: Feature verification passes with documented Nix commands or records any unavailable Nix/network checks.

## Assumptions

- KDE Plasma Wayland remains the only real compositor target for this increment.
- Real desktop-wide motion sequence observation is adapter work and may remain unavailable until a compositor-specific provider exists.
- `hotkeys` and `bindings` are compatibility surfaces; `motions` is the preferred future notation for sequences and repeat.
