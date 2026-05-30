# Feature Specification: Callback Wakeups

**Feature Branch**: `009-callback-wakeups`

**Created**: 2026-05-30

**Status**: Draft

**Input**: User description: "KDE callback events must wake the live runner promptly. Define low-jitter shortcut delivery, idle efficiency, and coexistence with input, repeat, and shutdown wakeups. Success criteria should measure callback-to-dispatch latency without naming implementation mechanisms in the spec."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Low-Jitter Shortcut Callback Delivery (Priority: P1)

A user triggers a compositor-provided shortcut and expects the associated macro decision to happen promptly even when no physical input event occurs at the same time.

**Why this priority**: A compositor shortcut callback that waits for unrelated input feels broken and makes shortcut-driven automation unreliable.

**Independent Test**: Simulate callback events while the runner is idle and while it is handling normal work; measure the time from callback receipt to macro dispatch decision and verify no callback waits for unrelated input.

**Acceptance Scenarios**:

1. **Given** the live runner is idle and a configured callback shortcut is active, **When** the callback event arrives, **Then** the runner dispatches the callback's macro decision within the latency target.
2. **Given** no keyboard, pointer, repeat, or shutdown event occurs, **When** a callback event arrives, **Then** the callback still wakes the runner and is not delayed until another event happens.
3. **Given** callback events arrive in a short burst, **When** each event maps to an enabled binding, **Then** every callback is considered in arrival order without starvation.

---

### User Story 2 - Efficient Idle Operation (Priority: P2)

A user leaves the runner active for long periods and expects it to stay quiet and efficient until a real callback, input event, repeat deadline, or shutdown request requires work.

**Why this priority**: Prompt callbacks should not require wasteful idle activity or noisy diagnostics when the desktop is inactive.

**Independent Test**: Run the runner through an extended idle interval with callback support enabled; verify idle activity remains bounded and a callback arriving after the idle interval is still dispatched within the latency target.

**Acceptance Scenarios**:

1. **Given** no callback, input, repeat, or shutdown work is pending, **When** the runner remains idle, **Then** it avoids continuous busy work and produces no repetitive idle diagnostics.
2. **Given** the runner has been idle for an extended interval, **When** a callback event arrives, **Then** the callback dispatch decision still meets the same latency target as a warm runner.
3. **Given** callback support is configured but unavailable due to desktop permission or compositor limitations, **When** the runner starts, **Then** it reports the unavailable callback source clearly and continues only for capabilities that remain explicitly configured and allowed.

---

### User Story 3 - Coexist With Input, Repeat, and Shutdown Wakeups (Priority: P3)

A user running mixed automation expects callback shortcuts, physical input, repeat timers, cancellation releases, and shutdown requests to share the live runner without one class of work blocking the others.

**Why this priority**: The runner must remain predictable when multiple desktop events arrive close together, especially for cancellation and shutdown behavior.

**Independent Test**: Simulate callback events interleaved with physical input, repeat activity, cancellation releases, and shutdown requests; verify each class is handled according to its configured semantics and no class indefinitely delays another.

**Acceptance Scenarios**:

1. **Given** a callback event and physical input event arrive close together, **When** both map to enabled automation behavior, **Then** both are considered without one source starving the other.
2. **Given** repeat output is active, **When** a cancellation release and a callback event arrive close together, **Then** cancellation remains effective before later repeat output can continue.
3. **Given** a shutdown request arrives while callback work is pending, **When** the runner processes wakeups, **Then** shutdown is honored promptly and no new macro is started after shutdown begins.

### Edge Cases

- Callback events that arrive while a macro decision is denied by focus, permissions, or configuration remain denied and do not retry silently.
- Callback bursts larger than normal desktop usage are bounded by documented queue, drop, or backpressure behavior rather than unbounded memory growth.
- Callback events received after shutdown has begun are ignored or reported according to a documented shutdown policy and do not start new macros.
- Missing compositor support, revoked permissions, disabled callback configuration, and callback source errors are diagnosable.
- Callback diagnostics avoid logging private input payloads or unrelated desktop metadata.
- Existing input, repeat, and shutdown behavior remains available when callback support is disabled.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST wake the live runner promptly when a configured compositor callback event arrives.
- **FR-002**: System MUST dispatch callback macro decisions without requiring an unrelated keyboard, pointer, repeat, or shutdown event.
- **FR-003**: System MUST preserve callback arrival order for callbacks that are accepted for processing.
- **FR-004**: System MUST define bounded behavior for callback bursts, including how dropped, coalesced, or deferred callbacks are reported if limits are reached.
- **FR-005**: System MUST remain idle-efficient when no callback, input, repeat, or shutdown work is pending.
- **FR-006**: System MUST let callback, physical input, repeat, cancellation, and shutdown work coexist without indefinite starvation.
- **FR-007**: System MUST honor shutdown promptly and prevent new callback-started macro work after shutdown begins.
- **FR-008**: System MUST preserve existing input consent, macro execution consent, process-awareness gates, and Lua configuration semantics for callback-triggered macros.
- **FR-009**: System MUST fail closed with a diagnosable error when callback permissions, compositor support, or configured callback sources are unavailable.
- **FR-010**: System MUST expose diagnostics for callback receipt, dispatch latency, denied callback work, callback burst limiting, unavailable callback support, and shutdown interactions when verbose diagnostics are enabled.
- **FR-011**: System MUST keep diagnostics privacy-bounded and avoid logging private input payloads or unrelated desktop metadata.
- **FR-012**: System MUST include automated coverage for idle callback delivery, callback bursts, mixed input and callback events, repeat cancellation interactions, shutdown interactions, unavailable callback support, and diagnostics.

### Key Entities

- **Callback Event**: A desktop-provided shortcut notification that may trigger an enabled automation binding.
- **Callback Dispatch Decision**: The point where a callback is accepted, denied, dropped, coalesced, or deferred according to configuration, consent, and runtime state.
- **Runner Wakeup Source**: A reason the live runner has work to consider, such as callback, physical input, repeat deadline, cancellation, or shutdown.
- **Callback Diagnostic**: Privacy-bounded information about callback receipt, dispatch latency, denial, burst limiting, or unavailable support.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Automated latency tests report callback-to-dispatch p95 at or below 20 ms and p99 at or below 50 ms under normal test load.
- **SC-002**: Extended-idle tests show a callback arriving after at least 5 minutes of idle time still meets the callback-to-dispatch latency target.
- **SC-003**: Burst tests process or explicitly report the disposition of at least 1,000 simulated callback events with zero silent losses.
- **SC-004**: Idle-efficiency tests show no continuous busy activity and no repetitive idle diagnostics when no callback, input, repeat, or shutdown work is pending.
- **SC-005**: Mixed-source tests show callback, input, repeat cancellation, and shutdown scenarios complete without starvation in 100% of covered cases.
- **SC-006**: Shutdown tests show no callback-started macro begins after shutdown has started.
- **SC-007**: Diagnostics tests show callback receipt, callback-to-dispatch latency, burst limiting, unavailable callback support, and denial reasons are observable when verbose diagnostics are enabled.
- **SC-008**: Existing Lua configurations and callback-triggered macros continue to load without migration.
- **SC-009**: Feature verification passes with documented Nix commands or records unavailable Nix checks with the exact failure.

## Assumptions

- KDE Plasma Wayland remains the real desktop target for callback shortcut behavior.
- Callback-triggered macros use the same macro execution, process gating, and consent model as existing configured automation.
- Callback wakeup reliability is a runtime behavior change and does not add new Lua syntax.
- Callback burst limits may be defined during planning as long as every limited callback has an observable disposition.
