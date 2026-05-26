# Data Model: Runtime Event Loop Performance

## Macro Run State

- **Fields**: run id, action list, next action index, generated action count,
  next deadline, cancellation flag.
- **Validation**: cancelled runs produce no further output requests.

## Runtime Event Token

- **Fields**: stable numeric token assigned by the adapter.
- **Validation**: tokens identify readiness only and do not grant capability.

## Runtime Counters

- **Fields**: event-loop wakeups, hotplug additions/removals, output queue
  failures, cancelled macro runs, max output queue depth.
- **Validation**: counters are bounded scalar values and do not store per-event
  payload history.
