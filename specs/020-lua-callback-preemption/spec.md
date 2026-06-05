# Feature Specification: Lua Callback Preemption

**Feature Branch**: `[020-lua-callback-preemption]`

**Created**: 2026-06-06

**Status**: Draft

**Input**: User description: "Bound non-yielding Lua callbacks so pure Lua loops or long CPU callbacks cannot block input responsiveness, shutdown, timer wakeups, or callback scheduling; preserve existing Lua APIs and document diagnostics, budget enforcement, and fail-closed behavior."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Stop Runaway Lua Callbacks (Priority: P1)

A user writes or imports a Lua controller callback that accidentally enters a tight loop or performs too much CPU work without calling any host-yielding API. Signal Auras stops that callback from monopolizing runtime progress and keeps shutdown, timer wakeups, callback scheduling, and input responsiveness serviceable.

**Why this priority**: The previous responsiveness work protects host-yielding callbacks such as `sa.sleep`, but pure Lua loops still threaten the event loop. This is the highest remaining runtime architecture risk.

**Independent Test**: Run a controller callback containing a non-yielding infinite loop or very large CPU loop and verify the callback is interrupted, produces a diagnosable timeout disposition, releases its active scheduler slot, emits no post-timeout output, and allows shutdown to complete.

**Acceptance Scenarios**:

1. **Given** a registered Lua callback with a non-yielding infinite loop, **When** the callback is triggered, **Then** the runtime stops that callback within the configured budget and records a timeout or preempted disposition.
2. **Given** a runaway callback is active, **When** shutdown is requested, **Then** shutdown progresses and no further output from that callback is emitted after cancellation.
3. **Given** a runaway callback owns a trigger's active slot, **When** the callback is preempted, **Then** later trigger attempts are governed by normal scheduler policy rather than remaining permanently blocked.

---

### User Story 2 - Preserve Yielding Callback Semantics (Priority: P2)

A user has existing controller callbacks that call approved host APIs such as `sa.sleep`, `sa.window.*`, `sa.input.*`, and `sa.log`. These callbacks continue to yield, resume, and complete according to the existing runtime contract.

**Why this priority**: Preemption must harden unsafe callback behavior without breaking the stable Lua extension surface or the callback continuation model already in use.

**Independent Test**: Run existing host-yielding callback tests and add a callback that alternates short pure Lua work with `sa.sleep`; verify it resumes on timer wake, completes successfully, and is not misclassified as runaway work.

**Acceptance Scenarios**:

1. **Given** a callback calls `sa.sleep`, **When** it yields to the host timer, **Then** it remains pending work and is not treated as over-budget while waiting for the timer.
2. **Given** a callback performs short bounded Lua work before emitting input, **When** the work completes within budget, **Then** output ordering and capability checks remain unchanged.
3. **Given** a callback fails because a capability is denied, **When** preemption is enabled, **Then** the capability failure remains the primary diagnostic and no extra preemption error is reported.

---

### User Story 3 - Diagnose Callback Budget Decisions (Priority: P3)

A user or developer can understand why a Lua callback was interrupted, which trigger and callback were affected, how long it ran, and what was skipped or cancelled, without exposing private window titles, process data, or input contents beyond already approved diagnostics.

**Why this priority**: Budget enforcement without clear diagnostics is difficult to debug and may look like silent automation failure.

**Independent Test**: Trigger slow, preempted, completed, cancelled, and denied callbacks and verify diagnostics identify the callback, trigger, disposition, elapsed time bucket, and remediation guidance while preserving privacy boundaries.

**Acceptance Scenarios**:

1. **Given** a callback exceeds its budget, **When** diagnostics are enabled, **Then** logs identify the callback name, trigger, budget, elapsed time, and timeout disposition.
2. **Given** multiple callbacks are scheduled, **When** one callback is preempted, **Then** diagnostics distinguish preempted work from skipped, denied, dropped, cancelled, completed, and slow-but-completed work.

### Edge Cases

- A callback enters a tight loop before its first host API call.
- A callback enters a tight loop after resuming from `sa.sleep` or a window host request.
- A callback performs a large but finite CPU loop that completes just before the budget.
- A callback is preempted while another trigger for the same callback arrives.
- Shutdown begins while a callback is being interrupted.
- A callback tries to emit input after it has exceeded its budget.
- Budget enforcement support is unavailable on the current runtime path.
- Diagnostics are emitted for process-scoped callbacks without leaking active window titles or process metadata unless explicitly requested through approved Lua APIs.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST enforce a runtime execution budget for imperative Lua callbacks, including callbacks that do not call any host-yielding API.
- **FR-002**: System MUST interrupt or cancel over-budget callbacks and release their scheduler active state so the trigger cannot remain stuck forever.
- **FR-003**: System MUST keep shutdown, timer wakeups, callback wakeups, repeat cancellation, and input/event responsiveness serviceable while an over-budget Lua callback is being handled.
- **FR-004**: System MUST prevent post-timeout output from a preempted callback; already completed host requests before the timeout MAY remain recorded according to existing output semantics.
- **FR-005**: System MUST preserve existing Lua-facing APIs and behavior for callbacks that complete within budget or yield through approved host APIs.
- **FR-006**: System MUST classify preempted callbacks separately from completed, slow, failed, denied, dropped, skipped, and cancelled callback dispositions.
- **FR-007**: System MUST provide privacy-bounded diagnostics for callback budget, elapsed time, trigger identity, callback identity, disposition, queue depth, and remediation.
- **FR-008**: System MUST define default callback budget policy in the library/runtime contract rather than through CLI-local constants.
- **FR-009**: System MUST fail closed with a diagnosable startup or runtime error if the selected runtime path cannot enforce the required callback budget.
- **FR-010**: System MUST define standalone Rust library behavior before CLI, Lua, or desktop integration behavior.
- **FR-011**: System MUST preserve explicit current-run consent and scoped capabilities for input, process metadata, window metadata, synthesized input, timers, and scripts.
- **FR-012**: System MUST document NixOS verification commands and keep the feature reproducible through the project flake.

### Key Entities *(include if feature involves data)*

- **Callback Execution Budget**: The configured amount of active Lua execution allowed for one callback invocation before interruption is required.
- **Lua Callback Invocation**: One scheduled execution of a registered Lua callback, including trigger, callback name, capabilities, active/pending state, elapsed time, and final disposition.
- **Preemption Disposition**: The recorded result when a callback exceeds its execution budget and is interrupted or cancelled.
- **Runtime Responsiveness Probe**: A test or diagnostic observation showing that shutdown, timer wakeups, callback wakeups, or event polling remained serviceable while budget enforcement occurred.
- **Callback Diagnostic Event**: Privacy-bounded diagnostic output describing callback budget decisions and remediation.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A non-yielding infinite-loop callback is interrupted in automated tests within the configured budget plus a documented enforcement tolerance.
- **SC-002**: Shutdown completes in the test harness while a runaway Lua callback is active, with no post-cancellation synthesized input emitted by that callback.
- **SC-003**: After a callback is preempted, the scheduler no longer reports that callback task as active or pending.
- **SC-004**: Existing host-yielding callback tests for `sa.sleep`, `sa.window.*`, `sa.input.*`, and capability denial continue to pass unchanged.
- **SC-005**: Diagnostics distinguish preempted callbacks from slow-but-completed, failed, denied, skipped, dropped, cancelled, and completed callbacks.
- **SC-006**: Stress tests with repeated triggers keep per-trigger pending work bounded and record every accepted, skipped, denied, dropped, cancelled, failed, completed, slow, or preempted disposition.
- **SC-007**: Verification passes with documented formatting, test, lint, and reproducibility commands.

## Assumptions

- The first increment targets imperative Lua controller callbacks only; declarative macro parsing is out of scope.
- Existing Lua APIs remain stable. Any future breaking Lua API change requires a separate spec and migration notes.
- Callback budget enforcement protects host responsiveness; it does not guarantee that a user's long-running Lua computation can be resumed after preemption.
- If several implementation mechanisms are viable, planning will prefer the smallest library-owned mechanism that can be tested deterministically and fails closed when unavailable.
- KDE Plasma Wayland remains the primary real-desktop target, but most preemption behavior should be testable without a live compositor.
