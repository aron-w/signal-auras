# Feature Specification: Runtime Shutdown Reliability

**Feature Branch**: `012-runtime-shutdown-reliability`

**Created**: 2026-05-30

**Status**: Draft

**Input**: User description: "Review runtime thread startup and SIGINT handling. SIGINT may be unblocked in spawned listener threads before the runtime signal fd is created, which can cause abrupt termination instead of cleanup. Make shutdown reliable through the runtime path, keep listener/helper threads from receiving default terminating signals, wake the main loop promptly, and release virtual input devices and grabs cleanly."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Route Terminal Signals Through Cleanup (Priority: P1)

A user presses Ctrl-C or sends SIGTERM while the runner is active and expects the normal shutdown summary and cleanup path to run.

**Why this priority**: Abrupt termination can leave current-run registrations, grabs, or virtual devices in a bad state.

**Independent Test**: Start the runtime with simulated signal delivery and verify SIGINT and SIGTERM are observed by the runtime shutdown path, produce a shutdown reason, and run cleanup exactly once.

**Acceptance Scenarios**:

1. **Given** the runner is active, **When** SIGINT is delivered, **Then** the runtime shutdown path handles it and releases current-run resources.
2. **Given** the runner is active, **When** SIGTERM is delivered, **Then** the runtime shutdown path handles it and releases current-run resources.
3. **Given** shutdown has started, **When** another shutdown signal arrives, **Then** cleanup remains idempotent and does not start new macro work.

---

### User Story 2 - Keep Helper Threads From Receiving Default Terminating Signals (Priority: P2)

A user starts the live runner with KDE callback listeners, input observers, and helper threads and expects those threads not to accidentally receive default terminating signals before cleanup can run.

**Why this priority**: Signal-mask ordering is a startup safety boundary for a multi-threaded runtime.

**Independent Test**: Exercise runtime startup ordering with signal masks and helper-thread creation; verify SIGINT/SIGTERM are blocked before listener/helper threads spawn and remain routed to the runtime signal handler.

**Acceptance Scenarios**:

1. **Given** runtime startup begins, **When** listener/helper threads are created, **Then** SIGINT and SIGTERM are already blocked for inherited thread masks.
2. **Given** the runtime signal fd is created after masks are established, **When** signals are delivered, **Then** listener/helper threads do not terminate the process directly.
3. **Given** startup fails after signal masks are installed, **When** cleanup unwinds, **Then** masks and resources are restored or left in a documented safe state.

---

### User Story 3 - Wake and Release Promptly on Shutdown (Priority: P3)

A user stops the runner while it is idle, waiting on input, or holding grabs and expects prompt wakeup and clean release.

**Why this priority**: Shutdown must remain serviceable even when no physical input, callback, repeat, or hotplug event arrives.

**Independent Test**: Simulate shutdown while the main loop is idle and while virtual input/grab resources are active; verify the loop wakes promptly and cleanup reports virtual input devices and grabs released.

**Acceptance Scenarios**:

1. **Given** the main loop is idle, **When** shutdown starts, **Then** the loop wakes promptly without waiting for unrelated input.
2. **Given** virtual input devices or evdev grabs are active, **When** shutdown runs, **Then** all current-run virtual devices and grabs are released or reported as cleanup failures.
3. **Given** macro output is pending, **When** shutdown starts, **Then** no new macro work begins after shutdown and pending cleanup is counted.

### Edge Cases

- SIGINT/SIGTERM delivered during startup, active runtime, idle waits, macro output, callback bursts, or cleanup are handled consistently.
- Listener/helper threads inherit blocked SIGINT/SIGTERM masks before they can run user-facing work.
- Shutdown wakeups do not require unrelated keyboard, pointer, callback, hotplug, or timer events.
- Cleanup of virtual devices, grabs, callbacks, scripts, and registrations is idempotent and diagnosable.
- No daemon, autostart, hidden IPC, or persistent shutdown state is introduced.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST handle SIGINT and SIGTERM through the runtime shutdown path.
- **FR-002**: System MUST prevent listener/helper threads from accidentally receiving default terminating SIGINT/SIGTERM before cleanup can run.
- **FR-003**: System MUST establish signal-mask ordering before spawning runtime listener/helper threads.
- **FR-004**: System MUST wake the main loop promptly when shutdown is requested.
- **FR-005**: System MUST release current-run virtual input devices, evdev grabs, KDE bridge scripts, callbacks, and shortcut registrations during shutdown cleanup.
- **FR-006**: System MUST avoid starting new macro work after shutdown begins.
- **FR-007**: System MUST report cleanup successes and failures in diagnostics or final summaries.
- **FR-008**: System MUST keep existing Lua configuration APIs unchanged.
- **FR-009**: System MUST include automated coverage for SIGINT, SIGTERM, helper-thread signal-mask ordering, shutdown wakeups, and cleanup idempotency.

### Key Entities

- **Runtime Shutdown Source**: A signal, callback, test lifecycle event, or explicit runtime request that starts shutdown.
- **Runtime Signal Guard**: The signal-mask and signal-fd ownership that routes SIGINT/SIGTERM to the main runtime.
- **Helper Thread**: A listener or observer thread that must inherit safe signal masks.
- **Cleanup Report**: Current-run resource release attempts, successes, and failures.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Automated signal tests show SIGINT and SIGTERM both produce runtime shutdown reasons and run cleanup.
- **SC-002**: Startup-order tests show listener/helper threads cannot receive unblocked default SIGINT/SIGTERM before the runtime signal fd is ready.
- **SC-003**: Idle shutdown tests show the main loop wakes within the existing runtime wake target without unrelated input.
- **SC-004**: Cleanup tests show virtual input devices and evdev grabs are released or reported in 100% of covered shutdown cases.
- **SC-005**: Regression tests show no macro work begins after shutdown starts.
- **SC-006**: Feature verification passes with documented Nix commands or records unavailable Nix checks with the exact failure.

## Assumptions

- This feature hardens the existing terminal-started runner and does not add a daemon or persistent supervisor.
- The runtime can use the existing `signalfd`/wake-fd event-loop primitives from feature 007 and callback wakeups from feature 009.
- Live compositor cleanup may still require supplemental manual KDE verification, but signal routing and resource cleanup semantics must have automated coverage where practical.
