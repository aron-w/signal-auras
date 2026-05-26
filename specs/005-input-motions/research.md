# Research: Unified Input Motions

## Decision: Add `motions` Beside Existing API Surfaces

Use `motions` as the preferred future model while preserving `hotkeys` and structured `bindings`.

**Rationale**: Existing scripts remain compatible, and repeat/sequence semantics can be introduced without breaking the current runner.

## Decision: Repeat Owns Held State

Represent repeat behavior with `repeat.while_held`, `repeat.interval_ms`, and `repeat.macro`.

**Rationale**: A separate `hold = true` flag duplicates the held-state list and makes cancellation less explicit.

## Decision: Allow Zero Inter-Action Defaults

`defaults.inter_action_delay_ms` and motion overrides accept `0` and higher. Explicit `delay(ms)` actions remain one millisecond or higher.

**Rationale**: Zero delay preserves current behavior; long generated-action waits must be interruptible when real scheduling is implemented.

## Decision: Fail Closed for Real Sequence Observation

The current implementation validates and stores motions, but real desktop-wide motion observation remains behind provider capabilities.

**Rationale**: Wayland input observation and consumption are security-sensitive compositor behavior. Unsupported providers must produce diagnosable failure rather than partial hidden behavior.
