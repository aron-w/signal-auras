# Focus Freshness Contract

The core scope API evaluates process-aware bindings from a configured scope, an active-process context, and a focus freshness policy.

## Inputs

- `ScopeSelection`: either explicit global scope or a configured process list.
- `ActiveProcessContext`: the latest metadata snapshot from the active-process provider.
- `FocusFreshnessPolicy`: stale threshold, defaulting to 2 seconds.
- Evaluation time: supplied by the caller or runtime clock for deterministic tests.

## Outputs

- `ScopeDecision::Allowed`: a process-scoped or global binding may continue to macro scheduling.
- `ScopeDecision::Denied`: macro scheduling must not occur. The denial includes a stable denial kind, a user-facing reason, the configured rule, and freshness fields when relevant.

## Required Denial Kinds

- stale focus metadata
- unavailable focus metadata
- permission-denied focus metadata
- ambiguous or untrusted focus metadata
- process mismatch

## Timestamp Rules

- `ActiveProcessContext.captured_at` represents when the compositor/KWin callback was received and converted into a focus snapshot.
- Reading cached focus state from the live KDE bridge must return the cached timestamp unchanged.
- A cached matching process snapshot must become stale and deny process-scoped matching if no newer focus callback refreshes the cache before the stale threshold.

## Privacy Rules

Diagnostics may include configured process names, denial kind, metadata age, and stale threshold. Diagnostics must not include command-line arguments, window titles, unrelated process names, or unconfigured process details beyond the already consented active process name used for a mismatch reason.
