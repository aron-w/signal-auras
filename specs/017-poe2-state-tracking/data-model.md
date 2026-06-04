# Data Model: PoE2 Screen State Tracking

## StateTracker

- `id`: stable non-empty identifier unique within a controller program.
- `scope`: explicit global or process list scope; PoE2 examples use a process scope.
- `capabilities`: must include `screen_read`.
- `poll_ms`: positive polling cadence in milliseconds.
- `detector`: one Rust-owned detector definition.
- `latest_state`: optional latest `TrackerState`.

Validation:
- Duplicate ids fail script validation.
- `poll_ms = 0` fails script validation.
- Capabilities must not include input output requirements for tracker-only registration.
- Fixture file paths are not accepted as fields.

## DetectorDefinition

Kinds:
- `radial_cooldown`: requires ROI and optional circular mask inset.
- `horizontal_progress_bar`: requires ROI and fill direction.

Shared ROI fields:
- `x`, `y`, `w`, `h` are non-negative coordinates/sizes.
- `w` and `h` must be positive.

## TrackerState

Variants:
- `RadialCooldown`: ready, cooldown_fraction, remaining_ms, total_estimated_ms, confidence, freshness_ms.
- `HorizontalProgressBar`: visible, progress_percent, confidence, freshness_ms.
- `Inactive`: reason, confidence, freshness_ms.

## ScreenSample

- Rust-owned current-run visual input.
- Not exposed to Lua.
- May be simulated by tests.
- Runtime capture is skipped if capability, compositor, or focus gates fail.

## StateTrackerPoller

- Owns registered trackers and their history.
- Selects due trackers by elapsed time.
- Requests at most one screen sample for a polling tick.
- Updates latest states and diagnostics without executing callbacks.
