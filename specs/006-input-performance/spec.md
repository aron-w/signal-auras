# Feature Specification: Input Motion Performance and Consistency

**Feature Branch**: `006-input-performance`

**Created**: 2026-05-26

**Status**: Draft

**Input**: User description: "Build a blazingly fast, efficient, and reliable input motion tool so evdev/uinput motions are low-latency, fair across devices, robust across device hotplug, and reliably cancel repeats such as held left-click spam under KDE Plasma Wayland, with a test suite that confirms the performance and reliability targets."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Low-Latency Mixed Input Observation (Priority: P1)

A user runs the existing PoE2 motion script on KDE Plasma Wayland and expects keyboard, mouse button, and wheel motions to be observed promptly and dispatched in the same order the desktop produced them.

**Why this priority**: Delayed or missed input makes every motion unreliable, including existing keyboard, wheel, and repeat workflows.

**Independent Test**: Simulate mixed keyboard, mouse button, and wheel events across multiple input devices; verify all supported events are dispatched without starvation and meet the latency target.

**Acceptance Scenarios**:

1. **Given** multiple active input devices, **When** keyboard and pointer events arrive close together, **Then** all supported events are observed and dispatched without one device starving another.
2. **Given** the runtime is idle, **When** an input event arrives, **Then** observation wakes promptly instead of waiting for a fixed polling sleep.
3. **Given** unsupported evdev event types or key auto-repeat events, **When** they arrive, **Then** they are ignored without delaying supported events.

---

### User Story 2 - Reliable Repeat Cancellation (Priority: P2)

A user holds the existing leader plus double left-click repeat motion and expects repeat clicking to stop as soon as the held input is released.

**Why this priority**: A stuck left-click repeat can spam the focused application and is the highest-risk failure mode in the current feature.

**Independent Test**: Simulate a repeat activation followed by release events racing with repeat deadlines; verify release/cancel processing wins before any later repeat action executes.

**Acceptance Scenarios**:

1. **Given** the left-click repeat is active, **When** any `while_held` input is released, **Then** the repeat is cancelled before another repeat tick is emitted.
2. **Given** a macro action delay is pending, **When** input releases during that delay, **Then** the runtime continues observing input and can cancel active repeats.
3. **Given** duplicate press or release noise from devices, **When** the repeat state changes, **Then** cancellation remains idempotent and no stopped repeat resumes without a new trigger sequence.

---

### User Story 3 - Device Hotplug and Diagnosable Operation (Priority: P3)

A user runs with `devices = "all"` and expects keyboard or pointer device changes to be detected during the current run with clear logs and without silently losing all motion input.

**Why this priority**: KDE keyboard modes and device switches can change evdev paths, which currently makes input disappear or become inconsistent.

**Independent Test**: Simulate added, removed, and reappearing event devices; verify the provider rescans, keeps active devices fair, and logs rescan results.

**Acceptance Scenarios**:

1. **Given** `devices = "all"`, **When** an active input device is removed, **Then** the provider marks it inactive, logs the path, and continues reading remaining devices.
2. **Given** `devices = "all"`, **When** a new readable event device appears, **Then** the provider opens it during the current run without requiring restart.
3. **Given** unsafe evdev/uinput permissions are missing or revoked, **When** setup or rescan tries to use the device, **Then** the runtime fails closed or logs the skipped path with remediation and no hidden privilege escalation.

### Edge Cases

- No new Lua motion syntax, trigger tokens, or user-facing motion capabilities are introduced by this feature.
- Missing `/dev/input` access, missing `/dev/uinput` access, unreadable new devices, removed devices, and denied grabs are diagnosable.
- `devices = "all"` remains current-run only and never persists device state or installs services.
- Existing explicit-device configurations do not rescan arbitrary new devices unless they opted into `devices = "all"`.
- Repeat ticks due at the same time as input releases process input release first after the release has been observed.
- Verbose logging remains opt-in and must avoid logging private text input payloads.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST observe supported keyboard, mouse button, and wheel evdev events with deterministic dispatch order across active devices.
- **FR-002**: System MUST avoid fixed-delay polling as the primary live input wait mechanism when an input provider can expose readiness or deadlines.
- **FR-003**: System MUST provide fair handling across keyboard and pointer devices so one busy device cannot indefinitely starve another active device.
- **FR-004**: System MUST continue ignoring evdev key auto-repeat events while preserving physical press and release transitions.
- **FR-005**: System MUST cancel active repeats when any configured `while_held` token is released.
- **FR-006**: System MUST prevent any repeat tick from executing after its cancellation release event has been processed.
- **FR-007**: System MUST keep observing cancellation input while generated macro delays or repeat intervals are pending.
- **FR-008**: System MUST rescan devices during the current run when unsafe evdev input is configured with `devices = "all"`.
- **FR-009**: System MUST log device addition, removal, skipped unreadable devices, active device counts, repeat start/cancel/tick, and input dispatch latency when verbose diagnostics are enabled.
- **FR-010**: System MUST preserve explicit unsafe evdev/uinput consent boundaries, permission checks, current-run behavior, and revocation behavior.
- **FR-011**: System MUST fail closed with diagnosable errors when required observation, grab, or synthesized input permissions are unavailable.
- **FR-012**: System MUST keep resource usage low by sleeping or blocking until input readiness, repeat deadlines, hotplug deadlines, or shutdown checks require work.
- **FR-013**: System MUST include automated tests with simulated input devices, repeat cancellation races, hotplug/rescan behavior, latency percentiles, and input scalability metrics.
- **FR-014**: System MUST not add new user-facing motion features until these reliability requirements are satisfied.
- **FR-015**: Runtime summaries MUST report average, p95, p99, and maximum motion dispatch latency without unbounded per-event memory growth.

### Key Entities

- **Observed Input Event**: A supported physical input transition with source device, token, state, and observation timestamp.
- **Input Provider Runtime**: Current-run unsafe evdev reader state, including active devices, removed devices, and rescan policy.
- **Repeat Runtime State**: Active repeat ownership, held-token state, next tick deadline, and cancellation status.
- **Latency Diagnostic**: Measured duration from provider observation to runtime dispatch and macro scheduling.
- **Unsafe Input Consent Boundary**: Explicit script configuration and OS permissions required for evdev observation, evdev grab, and uinput output.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Automated latency metric tests confirm p95 input-observed-to-runtime-dispatch latency at or below 20 ms and p99 at or below 50 ms under normal test load.
- **SC-002**: Repeat cancellation tests show zero repeat macro actions after the cancellation release event has been processed.
- **SC-003**: Scalability tests drain at least 1,000 queued simulated input events across at least 8 devices in under 1 second while dispatching every device's events.
- **SC-004**: Hotplug tests show `devices = "all"` reports removed or newly readable event devices within 1 second.
- **SC-005**: Idle and repeat-active profiling tests show the runtime blocks or sleeps until the next event/deadline instead of spinning continuously.
- **SC-006**: Verbose diagnostics include enough timestamps, source device summaries, and repeat lifecycle events to explain delayed input or cancellation behavior.
- **SC-007**: Existing Lua motion examples, including the PoE2 example, continue to load without API changes.
- **SC-008**: Feature verification passes with documented Nix commands or records unavailable Nix checks with the exact failure.

## Assumptions

- KDE Plasma Wayland remains the real compositor target for this increment.
- Unsafe evdev/uinput remains an explicit high-trust local backend and is not made the default.
- Existing PoE2 motions are regression targets: F3 leader, F5 hideout, leader wheel left/right, and leader double-left-click repeat.
- The implementation may add Rust internal scheduling and diagnostic types without changing the public Lua DSL.
