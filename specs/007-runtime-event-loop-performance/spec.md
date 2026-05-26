# Feature Specification: Runtime Event Loop Performance

**Feature Branch**: `007-runtime-event-loop-performance`
**Created**: 2026-05-27
**Status**: Draft

## User Stories

### User Story 1 - Cancellable Live Macro Execution

As a user running motion repeats, I need macro delays and repeat output to stay
cancellable so release input can stop repeat behavior before queued output
continues.

**Acceptance Criteria**

- Macro delays do not block live input observation.
- Repeat cancellation drops repeat macro runs that have not started output.
- Already-started kernel writes may complete, but no later queued repeat action
  may execute after cancellation is processed.

### User Story 2 - Event-Driven Runtime Readiness

As a user with multiple input devices, I need the live runner to sleep until
input readiness, timer deadlines, shutdown, or hotplug events require work.

**Acceptance Criteria**

- Runtime readiness is represented by a reusable Rust event-loop API.
- Evdev fd readiness can be registered and waited without fixed polling.
- Hotplug is planned as udev-driven for `devices = "all"` and remains
  current-run only.

### User Story 3 - Low-Overhead Diagnostics and Output

As an operator debugging latency, I need structured logs and bounded stats
without leaking macro text payloads or adding avoidable overhead.

**Acceptance Criteria**

- Verbose diagnostics use structured logging and write to stderr.
- Final summary includes event-loop, cancellation, hotplug, and output queue
  counters.
- Uinput writes preserve event order while batching logical actions.

## Requirements

- The Lua DSL and consent model MUST remain unchanged.
- No daemon, IPC endpoint, autostart entry, persistence, or hidden global
  behavior may be introduced.
- New runtime dependencies MUST be represented in Cargo metadata and the Nix
  dev shell.
- Sensitive input/output capabilities MUST remain explicit and fail closed.
