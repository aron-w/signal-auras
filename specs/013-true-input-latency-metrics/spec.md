# Feature Specification: True Input Latency Metrics

**Feature Branch**: `013-true-input-latency-metrics`

**Created**: 2026-05-30

**Status**: Draft

**Input**: User description: "Current metrics measure dispatch-after-userspace-read, not kernel-event-to-action latency, because evdev timestamps are discarded. Preserve evdev kernel timestamps where available, report true event age/backlog latency, keep current dispatch latency available or clearly renamed, and test timestamp parsing and metric calculation."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Preserve Kernel Event Time (Priority: P1)

An operator debugging input latency needs the runtime to keep the timestamp supplied by evdev instead of replacing it with only a userspace read time.

**Why this priority**: Without kernel timestamps, metrics cannot distinguish real input backlog from slow dispatch after the event was read.

**Independent Test**: Parse simulated evdev events with valid kernel timestamps and verify observed input events preserve the timestamp alongside the userspace observation time.

**Acceptance Scenarios**:

1. **Given** an evdev event contains a valid kernel timestamp, **When** the adapter decodes it, **Then** the observed event preserves that kernel timestamp for later metrics.
2. **Given** an evdev event lacks a usable timestamp, **When** the adapter decodes it, **Then** the runtime records timestamp unavailable and continues with existing dispatch metrics.
3. **Given** multiple event types are decoded, **When** keyboard, pointer button, and wheel events are observed, **Then** timestamp preservation applies consistently where the kernel timestamp is present.

---

### User Story 2 - Report True Event Age and Backlog (Priority: P2)

An operator reviewing stats needs to see kernel-event-to-action age or backlog latency separately from dispatch-after-read latency.

**Why this priority**: True event age exposes delayed reads, device backlog, and scheduler stalls that current dispatch latency cannot measure.

**Independent Test**: Feed events with controlled kernel timestamps and action dispatch times; verify average, percentile, and max event-age metrics are calculated correctly.

**Acceptance Scenarios**:

1. **Given** an event has a preserved kernel timestamp, **When** the runtime dispatches a macro decision, **Then** stats record the elapsed time from kernel event to action decision.
2. **Given** the runtime is behind on input reads, **When** older events are dispatched, **Then** event-age metrics increase even if dispatch-after-read remains low.
3. **Given** kernel timestamps are unavailable, **When** events are dispatched, **Then** stats clearly report unavailable true-latency samples without corrupting dispatch metrics.

---

### User Story 3 - Keep Existing Dispatch Metrics Understandable (Priority: P3)

An operator comparing old and new diagnostics needs the existing dispatch-after-read metrics to remain available or be clearly renamed.

**Why this priority**: Existing latency checks are still useful for userspace scheduling regressions and should not be confused with true kernel-to-action latency.

**Independent Test**: Render final stats with both true event-age metrics and current dispatch-after-read metrics; verify labels distinguish the two.

**Acceptance Scenarios**:

1. **Given** both metric types are available, **When** diagnostics are rendered, **Then** true event-age metrics and dispatch-after-read metrics have distinct names.
2. **Given** existing tests depend on dispatch latency, **When** metrics are updated, **Then** those metrics remain available or are renamed with migration notes.
3. **Given** verbose diagnostics are enabled, **When** input events are processed, **Then** logs identify whether latency is true event age or dispatch-after-read delay.

### Edge Cases

- Evdev timestamps may be zero, unavailable, invalid, older than the process clock origin, or impossible to compare with the runtime clock.
- Timestamp parsing must cover keyboard, mouse button, and wheel events.
- True event-age metrics must not replace dispatch-after-read metrics silently.
- Metrics remain bounded and privacy-safe; no key text, macro text, process command lines, or window titles are logged.
- No new input observation capability, daemon, persistence, or Lua syntax is introduced.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST parse evdev kernel timestamps from raw input events where available.
- **FR-002**: System MUST preserve kernel timestamps on decoded observed input events.
- **FR-003**: System MUST preserve the existing userspace observation/read time for dispatch-after-read metrics.
- **FR-004**: System MUST report true event age or backlog latency from kernel event timestamp to action decision where timestamps are comparable.
- **FR-005**: System MUST keep current dispatch latency metrics available or clearly rename them to distinguish dispatch-after-read from kernel-event-to-action latency.
- **FR-006**: System MUST count unavailable, invalid, or incomparable kernel timestamps without corrupting latency summaries.
- **FR-007**: System MUST expose privacy-bounded stats and verbose diagnostics for true event age/backlog latency.
- **FR-008**: System MUST preserve existing Lua input-provider and motion APIs.
- **FR-009**: System MUST include automated coverage for timestamp parsing, unavailable timestamp handling, metric calculation, diagnostic labels, and existing dispatch metric compatibility.

### Key Entities

- **Kernel Event Timestamp**: Timestamp supplied by evdev in a raw input event.
- **Userspace Observation Time**: Runtime monotonic time when Signal Auras read or accepted the event.
- **Event Age Metric**: Kernel-event-to-action elapsed time used to identify backlog and read delays.
- **Dispatch-After-Read Metric**: Existing userspace-read-to-action elapsed time used to identify runtime dispatch delays.
- **Timestamp Availability State**: Whether an event has a valid, comparable, unavailable, or invalid kernel timestamp.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Timestamp parsing tests cover keyboard, pointer button, and wheel evdev events with preserved kernel timestamps.
- **SC-002**: Metric tests show true event-age average, p95, p99, and max values match controlled kernel timestamp inputs.
- **SC-003**: Backlog tests show true event-age metrics increase for older kernel events even when dispatch-after-read latency remains low.
- **SC-004**: Diagnostics tests show true event-age labels are distinct from dispatch-after-read latency labels.
- **SC-005**: Unavailable timestamp tests show samples are counted or excluded explicitly without corrupting summaries.
- **SC-006**: Existing dispatch latency tests continue to pass or are updated with clear rename expectations.
- **SC-007**: Feature verification passes with documented Nix commands or records unavailable Nix checks with the exact failure.

## Assumptions

- The first implementation targets evdev-backed unsafe input observation, because that is where kernel timestamps are currently discarded.
- Kernel timestamps may need conversion to a runtime-comparable monotonic reference; if exact conversion is unavailable, the runtime must report that true event age is unavailable rather than inventing precision.
- This feature is metrics-only and does not change binding matching, macro semantics, input permissions, or Lua syntax.
