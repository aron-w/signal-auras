# Research: Input Motion Performance and Consistency

## Decision: Use fd readiness polling for live evdev waits

**Rationale**: The existing fixed sleep loop can delay input by one or more loop intervals and wastes wakeups while idle. `libc::poll` is already available through the wayland crate dependency and lets the provider block until an evdev fd is readable or the next runtime deadline arrives.

**Alternatives considered**: Add an async runtime; rejected because this project currently uses a simple synchronous runner and a new runtime would add unnecessary architecture. Keep shorter sleeps; rejected because it still polls and makes latency dependent on arbitrary sleep intervals.

## Decision: Drain bounded bursts fairly across devices

**Rationale**: Reading one event per loop can delay queued events, while draining one busy device completely can starve other devices. A round-robin device cursor with bounded per-wake reads preserves fairness and keeps dispatch deterministic enough for tests.

**Alternatives considered**: Read only the first ready device; rejected for fairness. Read all bytes from one fd before checking others; rejected because pointer noise can delay keyboard events.

## Decision: Treat repeat cancellation as higher priority than due repeat ticks

**Rationale**: A repeat tick and release event can be due in the same loop. Once release has been observed and processed, the repeat must stop before another macro is emitted. The loop should process ready input before evaluating repeat deadlines after each wait.

**Alternatives considered**: Keep current repeat-first/after-input timing; rejected because scheduling jitter can emit a click after release.

## Decision: Keep macro execution synchronous for tests but remove long fixed input sleeps in the live loop

**Rationale**: The primary observed latency bug is the fixed live-loop sleep and one-event polling. This increment improves live responsiveness without changing the public macro API. Longer-term fully interruptible macro execution can be added behind the same scheduler contract if needed.

**Alternatives considered**: Replace all macro execution with a job queue immediately; rejected as broader than the minimum reliability increment and higher risk for existing macros.

## Decision: Rescan only for `devices = "all"`

**Rationale**: Explicit device lists are user-selected permission boundaries. `devices = "all"` already opts into broad current-run device discovery, so hotplug rescans are consistent with that consent.

**Alternatives considered**: Rescan for explicit lists; rejected because it could open paths the user did not explicitly configure.
