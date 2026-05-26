# Data Model: Input Motion Performance and Consistency

## Observed Input Event

- **Fields**: motion token, input state, source device path, observed timestamp.
- **Validation**: Only supported key, mouse button, and wheel events are converted; auto-repeat key values are ignored.
- **Relationships**: Consumed by the motion runtime and latency diagnostics.

## Evdev Device State

- **Fields**: path, file descriptor, active flag, grabbed flag, last read status.
- **Validation**: A device is active only while its fd is readable and not removed; unreadable paths are skipped with diagnostics during `devices = "all"` rescans.
- **Relationships**: Owned by the evdev observation provider.

## Input Provider Runtime

- **Fields**: configured mode, leader token, all-devices flag, known device paths, active devices, next fair-read cursor, rescan interval.
- **State transitions**: configured -> active; active device -> removed; new readable path -> active; unreadable path -> skipped.
- **Relationships**: Exposes observed input events to the CLI runner.

## Repeat Runtime State

- **Fields**: trigger, while-held tokens, active flag, next tick deadline.
- **Validation**: Repeat is active only while all held tokens are satisfied.
- **State transitions**: inactive -> active on trigger completion; active -> cancelled on held-token release; active -> tick due on deadline.

## Latency Diagnostic

- **Fields**: source device, event label, observed timestamp, dispatched timestamp, elapsed duration.
- **Validation**: Diagnostics are emitted only when verbose logging is enabled.
- **Relationships**: Used by tests and runtime logs to explain input latency.
