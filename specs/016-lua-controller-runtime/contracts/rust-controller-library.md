# Contract: Rust Controller Library

## Registration

`ControllerRegistration::new(kind, trigger, scope, mode, callback, required_capabilities)` MUST:

- Trim and normalize trigger/callback labels.
- Reject empty trigger or callback labels.
- Preserve scope, mode, callback, capability set, and overload policy.

`ControllerRegistrationSet::new(registrations)` MUST:

- Reject empty registration collections.
- Reject duplicate normalized `(kind, scope, trigger)` registrations.
- Reject prefix-overlapping motion registrations within the same scope.
- Expose a deduplicated required capability set.
- Validate a `CapabilityReport` and return the first fail-closed diagnostic.

`ControllerProgram::new(registrations, callbacks)` MUST:

- Reject registrations whose callback names are not defined.
- Preserve validated registrations and callback output plans separately.
- Expose combined registration and callback output capability requirements.

## Callback Scheduling

`LuaCallbackScheduler::new(max_pending, default_budget)` MUST reject zero capacity or zero budget.

`schedule(registration, capability_report, accepted_at)` MUST:

- Return `Denied` when a required capability is missing or unavailable.
- Return `Accepted` and enqueue a task when capacity and per-trigger policy allow it.
- Return `Skipped` or `Dropped` for repeated triggers according to overload policy.
- Return `Dropped` when the pending queue is full.

`finish(task, elapsed)` MUST clear the trigger from active/pending state and return `Slow` when elapsed time exceeds the task budget.

`cancel_all()` MUST drop all pending work and clear active/pending trigger state.

## Output Batching

`RustOperationBatch::enqueue_input(action, capability_report)` MUST:

- Require `SynthesizedInput` capability.
- Fail closed without enqueueing when permission is denied, revoked, unsupported, unprobed, or unavailable.
- Preserve request order with monotonic sequence numbers.
- Reject requests after bounded queue capacity is reached.

`queue_controller_callback_outputs(callback, capability_report, batch)` MUST:

- Translate approved callback output actions into ordered Rust operation
  requests.
- Fail closed before enqueueing denied, revoked, unavailable, unsupported, or
  unprobed capabilities.

## Diagnostics

All denied or invalid operations MUST return `DiagnosableError` or `AdapterDiagnostic` values without recording private text payloads, command-line arguments, window titles, or unrelated process data.
