# Data Model: Callback Wakeups

## Callback Event

- **Fields**: configured hotkey id, receipt timestamp, active-process context.
- **Validation**: hotkey must map to a registered current-run shortcut action.
- **State**: received -> queued -> dispatched, denied, ignored, or dropped.

## Callback Queue

- **Fields**: FIFO accepted event list, fixed capacity, dropped-newest count.
- **Validation**: accepted callbacks preserve arrival order; full queue records
  a dropped disposition instead of growing.
- **State**: empty, accepting, full-with-drops, drained.

## Callback Dispatch Decision

- **Fields**: trigger label, latency from receipt to decision, disposition,
  optional denial classification.
- **Validation**: no callback-started macro may be scheduled after shutdown
  begins.
- **State**: accepted binding -> allowed macro schedule, denied by scope or
  permissions, ignored unknown callback, dropped by queue limit.

## Runner Wakeup Source

- **Fields**: callback wake fd, evdev/udev input readiness, repeat/macro timer,
  shutdown signal.
- **Validation**: no source can indefinitely starve another; cancellation and
  shutdown are handled before later repeat/macro output continues.

## Callback Diagnostic

- **Fields**: event name, trigger label where applicable, latency bucket or
  count, disposition, reason.
- **Validation**: diagnostics avoid private input payloads, window titles, and
  unrelated desktop metadata.
