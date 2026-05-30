# Contract: Callback Wakeup Runtime

## Queue Contract

- Accepted callback events are delivered FIFO.
- Queue capacity is fixed at implementation level and documented in verbose
  diagnostics.
- When full, the newest callback is dropped and the drop count is observable.
- Drops never silently turn into macro execution.

## Wake Contract

- A newly accepted or dropped callback writes to a runtime wake fd.
- The live runner includes that fd in the same wait set as shutdown, repeat
  timers, macro timers, input, and hotplug readiness.
- Draining the wake fd does not lose queued callback events.

## Dispatch Contract

- Callback dispatch uses existing hotkey binding lookup, process scope
  decisions, macro queue scheduling, and input consent checks.
- Unknown callback action names are ignored with observable diagnostics.
- Callback receipt-to-dispatch latency is recorded for accepted callbacks that
  reach a runner decision.
- After shutdown begins, callbacks are ignored and cannot start new macro work.

## Failure Contract

- Missing KWin scripting, KGlobalAccel, D-Bus callback registration, or callback
  listener setup fails closed with a diagnosable error.
- Callback diagnostics do not include private input payloads, raw window titles,
  or unrelated desktop metadata.
