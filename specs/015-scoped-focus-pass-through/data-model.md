# Data Model: Scoped Focus Pass-Through

## ScopedFocusState

- `active`: boolean state used by the runner to decide whether process-scoped automation may process input.
- `reason`: `active`, `process_mismatch`, `stale_focus`, `focus_unavailable`, `focus_permission_denied`, `ambiguous_focus`, or `untrusted_focus_timestamp`.
- `rule`: privacy-bounded configured process rule, for example `processes:kate`.
- `metadata_age`: optional age of the focus snapshot.
- `stale_threshold`: optional configured freshness threshold.

Validation:
- Explicit global scope is always active and does not emit scoped activation/deactivation transitions.
- Process-scoped state is inactive for every denial kind.
- Fields must not contain command-line arguments, window titles, macro payloads, or unrelated process data.

## InactivePassThroughDecision

- `trigger_label`: sanitized trigger or motion label.
- `input_kind`: callback, hotkey, composite trigger, motion input, repeat tick, or raw observed input.
- `action`: pass through, ignore callback, cancel repeat, cancel queued output, release grab.

Validation:
- Inactive decisions must not schedule macros.
- Inactive decisions must not record consumed/prevented input.
- Grabbed raw input is passed through at most once for each observed input event.

## ScopedQueuedWork

- `trigger_label`: trigger or repeat label associated with a pending macro run.
- `scope_kind`: explicit global or process scoped.
- `state`: pending, active, complete, or cancelled.

Validation:
- Process-scoped queued work is cancelled on active-to-inactive transition.
- Explicit global queued work is unaffected by process focus changes.

## FocusActivationLog

- `level`: info.
- `event`: `scoped_focus_transition`.
- `state`: active or inactive.
- `rule`: configured process rule.
- `reason`: transition reason.
- `metadata_age_ms`: optional.
- `stale_threshold_ms`: optional.

Validation:
- Emit only when the state changes.
- Omit private command-line arguments, window titles, text payloads, and macro payloads.
