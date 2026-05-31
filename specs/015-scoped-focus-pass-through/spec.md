# Feature Specification: Scoped Focus Pass-Through

**Feature Branch**: `015-scoped-focus-pass-through`

**Created**: 2026-05-31

**Status**: Draft

**Input**: User description: "if current focused window is not the one specified, nothing should be processed / prevented. activation / deactivation should be logged as info level."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Pass Through Outside Scoped Focus (Priority: P1)

A user configures process-scoped automation and expects Signal Auras to become inactive when the current focused window or process does not match the configured scope. While inactive, the focused application should receive the user's original input normally.

**Why this priority**: Preventing or processing input in the wrong application is the highest-risk failure for process-aware automation.

**Independent Test**: Simulate a process-scoped configuration with a non-matching focused process, then trigger scoped hotkeys and motions; verify no macro runs, no repeat starts, no consumed/prevented input is recorded, and passthrough behavior is reported.

**Acceptance Scenarios**:

1. **Given** a process-scoped binding or motion and fresh focused-process metadata for a non-matching process, **When** the user presses the configured trigger input, **Then** Signal Auras does not process the trigger and does not prevent the original input.
2. **Given** a scoped evdev grab or consume-capable input path is inactive because focus is outside scope, **When** observed physical input arrives, **Then** the runtime passes the input through before any motion sequence, repeat, or macro scheduling behavior can affect it.
3. **Given** a scoped repeat motion was previously active, **When** focus changes to a non-matching process, **Then** repeat output is cancelled and no further repeat ticks execute while inactive.

---

### User Story 2 - Activate Only When Focus Matches (Priority: P2)

A user switches back to the configured application and expects scoped automation to resume only after current focus metadata proves the configured process is focused.

**Why this priority**: Users need automation to resume predictably without restarting the runner, but only when the focus decision is trusted.

**Independent Test**: Simulate focus moving from non-matching to matching metadata, then trigger the same scoped automation; verify normal processing resumes and is tied to the matching focus state.

**Acceptance Scenarios**:

1. **Given** the runtime is inactive because focus is outside scope, **When** fresh focus metadata later matches the configured process rule, **Then** scoped automation becomes active for subsequent trigger input.
2. **Given** scoped automation is active and a matching trigger is pressed, **When** the configured binding or motion is otherwise valid and consented, **Then** the macro or repeat behavior proceeds under existing capability rules.
3. **Given** focus metadata is stale, unavailable, denied, ambiguous, or untrusted, **When** trigger input arrives, **Then** scoped automation remains inactive and no macro output or input prevention occurs.

---

### User Story 3 - Log Focus Activation State Changes (Priority: P3)

An operator debugging process-aware automation needs clear info-level logs when the runner changes between active and inactive scoped focus states.

**Why this priority**: Pass-through behavior must be explainable without requiring verbose per-event logs.

**Independent Test**: Simulate focus transitions matching to non-matching and non-matching to matching; verify exactly one info-level activation or deactivation log is emitted per state change with privacy-bounded fields.

**Acceptance Scenarios**:

1. **Given** scoped automation changes from inactive to active, **When** the focus decision becomes a trusted match, **Then** an info-level activation log is emitted.
2. **Given** scoped automation changes from active to inactive, **When** focus becomes non-matching, stale, unavailable, denied, ambiguous, or untrusted, **Then** an info-level deactivation log is emitted.
3. **Given** focus remains in the same active or inactive state across repeated input events or metadata refreshes, **When** those events are handled, **Then** activation/deactivation logs are not repeated as per-event spam.

### Edge Cases

- Explicit global scope keeps its existing behavior and is not deactivated by focused-process mismatch.
- Process-scoped hotkey callbacks, motion input, repeat ticks, consumed bindings, and grabbed evdev input all share the same active/inactive scoped-focus decision.
- Metadata that is stale, unavailable, denied, ambiguous, or has an untrusted timestamp makes process-scoped automation inactive.
- A focus transition to inactive cancels scoped repeat state, queued scoped macro output, and any armed input grab before further output or prevention can occur.
- Metadata that becomes matching after an input was passed through does not retroactively process that earlier input.
- Providers that cannot guarantee pass-through while inactive must fail closed or avoid activating prevention until focus is trusted.
- Activation and deactivation diagnostics must avoid command-line arguments, window titles, text payloads, and unrelated process data.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST define a scoped-focus state for process-scoped automation with active and inactive outcomes.
- **FR-002**: System MUST treat process-scoped automation as inactive when the current trusted focused window or process does not match the configured process rule.
- **FR-003**: System MUST treat process-scoped automation as inactive when focus metadata is stale, unavailable, permission-denied, ambiguous, missing, or has an untrusted timestamp.
- **FR-004**: System MUST NOT process scoped hotkey callbacks, motion trigger sequences, repeat ticks, or macro scheduling while scoped focus is inactive.
- **FR-005**: System MUST NOT prevent, consume, grab-suppress, or otherwise block original user input because of a process-scoped binding while scoped focus is inactive.
- **FR-006**: System MUST pass through original physical input while scoped focus is inactive when the input provider has already observed or temporarily intercepted it.
- **FR-007**: System MUST cancel scoped active repeats, queued scoped macro output, and armed scoped input grabs when scoped focus changes from active to inactive.
- **FR-008**: System MUST resume scoped automation for subsequent input when fresh trusted focus metadata matches the configured process rule.
- **FR-009**: System MUST emit an info-level activation log when process-scoped automation changes from inactive to active.
- **FR-010**: System MUST emit an info-level deactivation log when process-scoped automation changes from active to inactive.
- **FR-011**: Activation and deactivation logs MUST include the configured rule, new state, reason, and freshness context when available.
- **FR-012**: Activation and deactivation logs MUST be emitted only on state transitions, not for every input event or unchanged focus refresh.
- **FR-013**: System MUST preserve the existing Lua scope and motion syntax with no required script migration.
- **FR-014**: System MUST preserve explicit current-run process inspection, input observation, input prevention, macro execution, and synthesized input consent boundaries.
- **FR-015**: System MUST fail closed with diagnosable feedback when the selected compositor or input provider cannot guarantee no processing and no prevention while inactive.
- **FR-016**: System MUST include automated coverage for non-matching pass-through, inactive no-processing, active resumption, inactive cancellation, stale/unavailable inactive behavior, and info-level transition logs.

### Key Entities

- **Scoped Focus State**: The current active or inactive decision for process-scoped automation, including reason and focus freshness context.
- **Configured Process Rule**: The process scope declared by the Lua script or selected for the current run through the terminal prompt.
- **Inactive Pass-Through Decision**: A decision that original input must continue to the focused application and no automation work may be scheduled.
- **Focus Activation Log**: A privacy-bounded info-level diagnostic emitted when scoped automation changes active state.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Automated tests show 100% of non-matching focused-process trigger attempts produce no macro output and no scoped input prevention.
- **SC-002**: Automated tests show inactive grabbed or intercepted input is passed through exactly once and is not consumed by scoped automation.
- **SC-003**: Automated tests show active repeats and queued scoped macro output are cancelled when scoped focus deactivates.
- **SC-004**: Automated tests show scoped automation resumes on subsequent input after fresh trusted matching focus metadata becomes available.
- **SC-005**: Tests cover stale, unavailable, denied, ambiguous, missing, and untrusted focus metadata as inactive states.
- **SC-006**: Log tests verify activation and deactivation are emitted at info level exactly once per state transition.
- **SC-007**: Privacy checks confirm transition logs do not include command-line arguments, window titles, text payloads, or unrelated process data.
- **SC-008**: Existing Lua scripts and examples using `scope.processes`, hotkeys, and motions continue to load without migration.
- **SC-009**: Feature verification passes with documented Nix commands or records unavailable Nix checks with the exact failure.

## Assumptions

- KDE Plasma Wayland remains the primary compositor target for this reliability increment.
- The "specified" focused window is represented by the existing process scope selected from Lua or the current-run prompt.
- Explicit global scope is intentionally unaffected by this feature.
- Existing stale-focus freshness rules and privacy-bounded diagnostics remain in force.
- Provider implementations may need to choose fail-closed startup behavior when pass-through cannot be guaranteed safely.
