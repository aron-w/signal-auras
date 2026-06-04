# Feature Specification: PoE2 Screen State Tracking

**Feature Branch**: `[017-poe2-state-tracking]`

**Created**: 2026-06-04

**Status**: Implemented

**Input**: User description: "Track PoE2 UI states only. First tracker is Refutation cooldown from the cooldown swirl; second tracker is Heavy Stun progress from a bar. Video files are test fixtures only. Tracker kind provides emitted state fields. Cooldown estimation and remaining time are calculated in Rust using polling rate and observed state."

## User Scenarios & Testing

### User Story 1 - Track Refutation Cooldown (Priority: P1)

A user declares a Refutation cooldown tracker for the PoE2 skill slot and receives state values without triggering any input or automation reaction.

**Why this priority**: This proves the screen-state tracking model with the first concrete PoE2 state.

**Independent Test**: Run the detector against `examples/poe2/refutation_cooldown.webm` and verify the tracker reports ready state, cooldown fraction, estimated total cooldown, remaining cooldown, and confidence.

**Acceptance Scenarios**:

1. **Given** Refutation is cooling down, **When** the tracker samples the configured skill ROI, **Then** it reports not ready and a positive remaining cooldown estimate.
2. **Given** the cooldown swirl reaches ready state, **When** the tracker samples the ROI, **Then** it reports ready and no remaining cooldown.
3. **Given** consecutive samples show swirl progression, **When** the tracker has enough observations, **Then** Rust estimates total cooldown duration and remaining cooldown in milliseconds.

---

### User Story 2 - Track Heavy Stun Progress (Priority: P2)

A user declares a Heavy Stun progress tracker and receives a continuously updated 0-100 progress value.

**Why this priority**: Heavy Stun uses a different detector class and validates bar-style state tracking.

**Independent Test**: Run the detector against `examples/poe2/progress_heavy_stun.webm` at 50 ms intervals and verify visible state, progress percentage, and confidence.

**Acceptance Scenarios**:

1. **Given** the Heavy Stun bar is visible, **When** the tracker samples the configured bar ROI, **Then** it reports visible and a progress value from 0 to 100.
2. **Given** the bar fill changes, **When** the next 50 ms poll runs, **Then** the reported progress changes accordingly.
3. **Given** the bar is absent or unreadable, **When** the tracker samples the ROI, **Then** it reports not visible or low confidence rather than inventing progress.

---

### User Story 3 - Register State Trackers Safely (Priority: P3)

A user can add state trackers to `examples/poe2.lua` without granting input output, creating hotkeys, or adding reactions.

**Why this priority**: The feature must remain observation-only and permission-scoped.

**Independent Test**: Load a Lua file with both trackers and verify registration succeeds without input capture, synthesized input, callbacks, or screen reads during startup.

**Acceptance Scenarios**:

1. **Given** a valid tracker config, **When** Signal Auras loads the script, **Then** tracker definitions are validated but no screen capture starts during registration.
2. **Given** screen-read permission is denied, **When** runtime starts, **Then** no screen data is read and no state values are updated.
3. **Given** PoE2 is outside trusted focus, **When** polling would otherwise occur, **Then** the tracker remains inactive and does not sample the screen.

### Edge Cases

- ROI is wrong because resolution, UI scale, or skill slot layout changed.
- The target UI element is hidden, occluded, blurred, animated, or too noisy.
- Refutation starts mid-cooldown, so total cooldown must be estimated from partial observations.
- Heavy Stun is at exactly 0%, exactly 100%, or disappears between polls.
- Poll ticks are delayed or skipped.
- Multiple trackers are active and must not open independent screen-capture sessions.
- Test fixture paths must not become runtime configuration fields.
- State changes must not synthesize input, consume input, or trigger callbacks in this feature.

## Requirements

### Functional Requirements

- **FR-001**: System MUST support Lua registration of screen state trackers with id, scope, capabilities, poll_ms, and a detector kind with kind-specific configuration.
- **FR-002**: System MUST NOT require or accept user-declared emitted fields; each detector kind MUST define its own state schema.
- **FR-003**: System MUST provide a `radial_cooldown` detector kind whose state includes ready status, cooldown fraction, cooldown remaining milliseconds, total estimated cooldown milliseconds, confidence, and sample freshness.
- **FR-004**: System MUST calculate Refutation cooldown estimates in Rust from observed radial progression, poll timing, and tracker history.
- **FR-005**: System MUST provide a `horizontal_progress_bar` detector kind whose state includes visible status, progress percentage from 0 to 100, confidence, and sample freshness.
- **FR-006**: System MUST poll Heavy Stun at the configured 50 ms cadence when the scoped target is trusted and screen-read permission is available.
- **FR-007**: System MUST treat `examples/poe2/refutation_cooldown.webm` and `examples/poe2/progress_heavy_stun.webm` as automated test fixtures only, not runtime tracker sources.
- **FR-008**: System MUST require explicit current-run `screen_read` capability and fail closed when permission, compositor support, or focus trust is unavailable.
- **FR-009**: System MUST batch active trackers against one current screen sample or capture stream rather than opening per-tracker capture sessions.
- **FR-010**: System MUST expose latest tracker state for diagnostics and future consumers without invoking gameplay reactions.
- **FR-011**: System MUST preserve Lua sandbox boundaries; scripts must not receive raw screen buffers, filesystem access, portal handles, or compositor handles.
- **FR-012**: System MUST provide privacy-bounded diagnostics containing tracker id, state summary, confidence, stale/denied reasons, and timing metrics without storing or logging screenshots.

### Key Entities

- **State Tracker**: A scoped Lua-declared observation rule with id, polling cadence, ROI, detector kind, and latest state.
- **Detector Kind**: A Rust-owned classifier type that defines required config, validation, and emitted state schema.
- **Tracker State**: Latest typed output from a detector, including confidence and freshness.
- **Screen Sample**: Permissioned, current-run visual input used internally by Rust detector logic.
- **Test Fixture**: Repository media file used by automated tests to validate detector behavior.

## Success Criteria

### Measurable Outcomes

- **SC-001**: A Lua example with Refutation and Heavy Stun trackers validates without registering hotkeys, motions, synthesized input, or reaction callbacks for the trackers.
- **SC-002**: Refutation fixture tests show cooldown remaining decreases monotonically during cooldown and reaches ready at the expected fixture transition.
- **SC-003**: Refutation total cooldown estimate stabilizes within 10% of fixture expectation after enough progression samples are available.
- **SC-004**: Heavy Stun fixture tests report progress within 5 percentage points of labeled fixture expectations at 50 ms sampling intervals.
- **SC-005**: Denied screen-read permission, unsupported compositor capture, and inactive focus produce no screen samples and diagnosable tracker states.
- **SC-006**: Simulated two-tracker polling at 50 ms keeps classifier work bounded and shares capture work across trackers.
- **SC-007**: Feature verification passes with documented Nix commands for formatting, linting, tests, and flake checks where feasible.

## Assumptions

- V1 uses explicit, resolution-bound ROIs; automatic UI element discovery is out of scope.
- The initial target is the provided 3840x2160 PoE2 fullscreen UI, with ROI values user-tunable.
- Polling every 50 ms is acceptable when using one persistent permissioned capture stream and small ROI classification.
- Runtime config declares detector kind and ROI only; fixture media is referenced by tests.
- This feature is observation-only. Input synthesis, Space suppression, cooldown-triggered casting, and other reactions are future work.
