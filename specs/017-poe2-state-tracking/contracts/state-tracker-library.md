# Contract: State Tracker Rust Library

## Tracker Registration

Rust accepts `StateTrackerDefinition` values with:
- `id`
- `scope`
- `capabilities`
- `poll_ms`
- `detector`

Validation returns `ScriptValidation` diagnostics for duplicate ids, missing `screen_read`, invalid ROIs, invalid poll intervals, unknown detector kinds, or runtime-only fixture fields.

## Detector Output

`radial_cooldown` emits:
- `ready: bool`
- `cooldown_fraction: u8`
- `remaining_ms: Option<u64>`
- `total_estimated_ms: Option<u64>`
- `confidence: u8`
- `freshness_ms: u64`

`horizontal_progress_bar` emits:
- `visible: bool`
- `progress_percent: u8`
- `confidence: u8`
- `freshness_ms: u64`

## Polling

`StateTrackerPoller::poll_due(...)` must:
- skip screen sampling when `screen_read` is denied or unavailable
- evaluate each tracker scope against current active-process context and skip screen sampling when focus is denied, stale, or non-matching for process-scoped trackers
- acquire at most one screen sample for all due trackers in a tick
- update tracker diagnostics without calling Lua callbacks or input APIs
