# Research: Scoped Focus Pass-Through

## Decision: Model Process-Scoped Activity As Explicit Scoped Focus State

Use the existing `ScopeSelection::decide_context*` freshness and confidence checks as the source of truth, then wrap the result in a small active/inactive scoped-focus state that carries the configured rule, reason, metadata age, and stale threshold.

**Rationale**: The core crate already owns scope and stale-focus semantics. Extending that contract avoids duplicating focus rules in the CLI and keeps non-matching, stale, unavailable, denied, ambiguous, and untrusted metadata behavior consistent.

**Alternatives considered**: A runner-only boolean gate was rejected because it would duplicate denial classification and make transition logging less testable.

## Decision: Treat Inactive Scoped Focus As Pass-Through Before Consumption

For process-scoped triggers and motion events, evaluate trusted focus before recording consumption or scheduling macro/repeat work. In grabbed evdev paths, inactive decisions must release/pass through observed raw input exactly once and avoid arming long-lived prevention.

**Rationale**: The core safety requirement is that out-of-scope applications receive original input normally. Consumption counters and queued work are side effects and must happen only after an active scoped-focus decision.

**Alternatives considered**: Denying after trigger recognition was rejected because the existing behavior records denials and can still represent consumed/prevented input, which conflicts with pass-through requirements.

## Decision: Cancel Scoped Work On Active-To-Inactive Transitions

When scoped focus changes from active to inactive, cancel repeat state for scoped motions, cancel queued/pending scoped macro output, and release any armed input grab before more output can occur.

**Rationale**: Previously active scoped work may outlive the focus that authorized it. Cancellation on transition prevents delayed macro output from leaking into a different application.

**Alternatives considered**: Cancelling only future repeat ticks was rejected because queued macro output can still be pending under the incremental runtime model.

## Decision: Info Logs Are Transition-Only And Privacy Bounded

Emit info-level `scoped_focus_transition` logs only when the active/inactive state changes. Include state, configured rule, denial reason, metadata age, and stale threshold when available. Do not log command lines, window titles, macro payloads, or unrelated process data.

**Rationale**: Operators need to understand activation/deactivation without enabling verbose per-event logs or exposing private window/process details.

**Alternatives considered**: Debug-only per-event denial logs were rejected because the spec requires info-level activation/deactivation without log spam.
