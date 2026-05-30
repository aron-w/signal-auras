# Feature Specification: Repeat Overload Policy

**Feature Branch**: `010-repeat-overload-policy`

**Created**: 2026-05-30

**Status**: Draft

**Input**: User description: "Repeat ticks must coalesce or drop under load instead of crashing. Define held repeat macro behavior when output is slower than the interval, release/cancel priority, and visible dropped/coalesced repeat diagnostics. Default behavior is to skip or coalesce overlapping repeat ticks, keep the runner alive, and never emit after a processed cancellation release."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Keep Held Repeats Stable Under Overload (Priority: P1)

A user holds a repeat macro whose output takes longer than the repeat interval and expects the runner to stay alive without building an unsafe backlog or crashing.

**Why this priority**: Repeat overload can rapidly multiply macro output, destabilize the runner, and send unexpected input to the desktop.

**Independent Test**: Exercise a held repeat where each macro action is slower than its repeat interval; verify overlapping ticks are skipped or coalesced, resource use remains bounded, and the runner continues serving later work.

**Acceptance Scenarios**:

1. **Given** a repeat macro is held and one repeat action is still in progress, **When** another tick becomes due for the same held repeat, **Then** the later tick is skipped or coalesced instead of starting overlapping output.
2. **Given** repeat output stays slower than the configured interval for an extended hold, **When** many ticks become due, **Then** the runner remains alive and bounded rather than accumulating unlimited pending repeat work.
3. **Given** overload ends while the input is still held, **When** the next allowed repeat opportunity occurs, **Then** repeat behavior resumes according to the documented policy without replaying skipped ticks.

---

### User Story 2 - Prioritize Release and Cancellation (Priority: P2)

A user releases the held input for an overloaded repeat macro and expects repeat output to stop decisively even if ticks were due or skipped during the overload.

**Why this priority**: Cancellation is the safety boundary that prevents a held repeat from continuing after the user has stopped the gesture.

**Independent Test**: Simulate a repeat overload with release events racing against due ticks; verify the processed release cancels the repeat and no later repeat macro action is emitted.

**Acceptance Scenarios**:

1. **Given** a repeat macro is overloaded, **When** a configured cancellation release is processed, **Then** no later repeat tick for that hold may start macro output.
2. **Given** a repeat tick and cancellation release become eligible close together, **When** the cancellation release is processed first, **Then** the tick is skipped and reported as cancelled rather than emitted.
3. **Given** output had already started before cancellation was processed, **When** the cancellation release is processed, **Then** already-started output may finish but no later repeat action begins for that hold.

---

### User Story 3 - Diagnose Dropped or Coalesced Repeats (Priority: P3)

An operator tuning a repeat macro needs visible diagnostics that explain when repeat ticks were skipped, coalesced, cancelled, or executed.

**Why this priority**: Overload handling is intentionally lossy; users need clear counters and reasons so they can tune intervals and macro duration.

**Independent Test**: Run repeats through normal, overloaded, cancelled, and shutdown cases with diagnostics enabled; verify diagnostics include repeat lifecycle events and accurate dropped or coalesced counts without logging private input payloads.

**Acceptance Scenarios**:

1. **Given** verbose diagnostics are enabled, **When** repeat ticks are skipped or coalesced under overload, **Then** diagnostics report the affected binding, reason, count, and time range.
2. **Given** a repeat is cancelled while overloaded, **When** final diagnostics are produced, **Then** they distinguish skipped, coalesced, executed, and cancelled ticks.
3. **Given** verbose diagnostics are disabled, **When** overload occurs, **Then** the runner still applies the policy without producing noisy per-tick output.

---

### User Story 4 - Keep Non-Repeat Trigger Collisions Stable (Priority: P4)

A user presses or mashes a non-repeat trigger while the previous macro for that trigger is still active and expects the always-on runner to continue instead of treating the collision as a fatal runtime error.

**Why this priority**: Non-repeat trigger collisions can happen during normal play and should not stop the runner or leave active-trigger state stuck.

**Independent Test**: Start a non-repeat macro, trigger the same binding again before it completes, and verify the second attempt follows the documented deny/skip/coalesce policy, increments diagnostics, keeps the runner alive, and clears active state after completion or cancellation.

**Acceptance Scenarios**:

1. **Given** a non-repeat trigger already has active macro output, **When** the same trigger is pressed again before completion, **Then** the later attempt is denied, skipped, or coalesced deterministically and the runner continues.
2. **Given** a non-repeat trigger collision is denied or skipped, **When** diagnostics or final stats are inspected, **Then** the denied or skipped action is counted without logging private macro payloads.
3. **Given** the active macro completes or is cancelled, **When** the runner observes cleanup, **Then** active trigger state is removed so later legitimate trigger attempts can run normally.

### Edge Cases

- Repeat intervals that are shorter than a single macro action use the default skip or coalesce policy and never create unbounded pending work.
- Skipped or coalesced ticks are not replayed after overload clears or after release.
- Repeated or mashed input for an already-active non-repeat trigger never returns a fatal error that stops the always-on runner.
- Active trigger state is cleaned up after macro completion, cancellation, permission denial, process/focus denial, or shutdown.
- Cancellation release, shutdown, permission denial, and process/focus denial remain effective while repeat overload is happening.
- Multiple held repeat bindings are isolated so overload in one binding does not corrupt another binding's state.
- Diagnostics are bounded and avoid logging private input payloads or unrelated desktop metadata.
- The Lua configuration API and existing repeat syntax remain unchanged.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST define a default overload policy for repeat ticks that become due while prior repeat output for the same held repeat is still pending or active.
- **FR-002**: System MUST skip or coalesce overlapping repeat ticks by default rather than starting overlapping output for the same held repeat.
- **FR-003**: System MUST keep repeat overload state bounded so overloaded repeats cannot accumulate unlimited pending macro work.
- **FR-004**: System MUST keep the live runner available for input, cancellation, diagnostics, and shutdown while repeat overload is happening.
- **FR-005**: System MUST treat a processed configured release as cancelling the held repeat before any later repeat tick for that hold may emit output.
- **FR-006**: System MUST prevent skipped or coalesced repeat ticks from replaying after overload clears, after cancellation, or during shutdown.
- **FR-007**: System MUST isolate overload accounting between distinct held repeat bindings.
- **FR-008**: System MUST report executed, skipped, coalesced, and cancelled repeat counts when verbose diagnostics are enabled.
- **FR-009**: System MUST report overload diagnostics in bounded summaries or rate-limited events so diagnostics cannot become their own overload source.
- **FR-010**: System MUST preserve existing Lua repeat syntax and macro consent requirements.
- **FR-011**: System MUST fail closed with diagnosable errors when required input or output permissions are unavailable or revoked.
- **FR-012**: System MUST include automated coverage for slow output, long-held repeats, cancellation races, multiple repeat bindings, shutdown during overload, diagnostics, and unchanged Lua configuration loading.
- **FR-013**: System MUST define deterministic overload behavior for non-repeat trigger attempts that arrive while the same trigger already has active or pending macro output.
- **FR-014**: System MUST keep the always-on runner live when repeated or mashed input collides with an already-active non-repeat trigger.
- **FR-015**: System MUST count denied, skipped, or coalesced non-repeat trigger collisions in stats and diagnostics.
- **FR-016**: System MUST always clear active trigger state after macro completion, cancellation, permission denial, process/focus denial, or shutdown.
- **FR-017**: System MUST include automated coverage for already-active non-repeat trigger collisions and active-state cleanup.

### Key Entities

- **Held Repeat**: A configured repeat binding currently active because its required input is held.
- **Repeat Tick**: A scheduled opportunity for a held repeat to emit macro output.
- **Repeat Overload Policy**: The rule that decides whether due repeat ticks execute, skip, coalesce, or cancel when output cannot keep pace.
- **Non-Repeat Collision Policy**: The rule that decides whether an already-active non-repeat trigger attempt is skipped, coalesced, or denied while the runner continues.
- **Active Trigger State**: Runtime tracking for triggers with pending or active macro output that must be removed when the output completes or is cancelled.
- **Cancellation Release**: A configured input release that ends a held repeat after it is processed.
- **Repeat Overload Diagnostic**: Privacy-bounded information about executed, skipped, coalesced, cancelled, and denied repeat work.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Slow-output stress tests keep the runner alive for at least 10,000 due repeat ticks without unbounded pending repeat growth or crash.
- **SC-002**: Overload tests show overlapping repeat ticks for the same held repeat are skipped or coalesced in 100% of covered cases.
- **SC-003**: Cancellation tests show zero repeat macro actions begin after the configured release has been processed.
- **SC-004**: Mixed-work tests show input, cancellation, diagnostics, and shutdown remain serviceable while at least one repeat binding is overloaded.
- **SC-005**: Multi-binding tests show overload counts and cancellation state remain correct for each held repeat independently.
- **SC-006**: Diagnostic tests show executed, skipped, coalesced, and cancelled repeat counts match expected values for the test scenario.
- **SC-007**: Existing Lua repeat configurations continue to load without migration.
- **SC-008**: Feature verification passes with documented Nix commands or records unavailable Nix checks with the exact failure.
- **SC-009**: Non-repeat collision tests show 100% of covered repeated or mashed attempts for an already-active trigger keep the runner alive.
- **SC-010**: Active-state cleanup tests show later legitimate non-repeat trigger attempts can run after completion or cancellation.
- **SC-011**: Diagnostic tests show denied or skipped non-repeat collisions are counted in stats without logging private macro payloads.

## Assumptions

- The default repeat overload policy for this feature is skip or coalesce overlapping ticks, not queue every missed tick.
- A processed cancellation release is the boundary after which no later repeat macro action may begin for the cancelled hold.
- Already-started output may complete if it began before cancellation was processed, but no later repeat output may start.
- This feature changes repeat runtime reliability without adding new Lua syntax.
- The non-repeat collision follow-up reuses the same bounded runtime accounting and diagnostics model rather than adding Lua syntax or a persistent queue.
