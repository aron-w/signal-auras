# Data Model: Lua Controller Runtime

## Lua Controller Script

- `main_path`: main script file.
- `root`: directory that contains `main_path`; all `sa.import()` modules resolve beneath it.
- `modules`: imported local Lua modules loaded during startup.
- Validation: imports must not use absolute paths, parent traversal, ambient package paths, shell, debug, network, or unrestricted filesystem APIs.

## Controller Registration

- `kind`: `hotkey`, `motion`, `press`, `timer`, or `shutdown`.
- `trigger`: normalized trigger label.
- `scope`: explicit global or process list.
- `mode`: consume or passthrough.
- `callback`: callback name/reference collected during startup.
- `required_capabilities`: normalized capability set required before activation.
- `overload_policy`: default `skip_while_pending`.
- Validation: duplicate kind/scope/trigger combinations fail; prefix-overlapping motion triggers fail.

## Lua Callback Task

- `registration_label`: stable registration identity.
- `callback`: callback name/reference to invoke later.
- `accepted_at`: enqueue time for latency diagnostics.
- `budget`: maximum intended execution duration.
- States/dispositions: accepted, skipped, denied, dropped, completed, slow, failed, cancelled.

## Rust Operation Request

- `sequence`: monotonic order inside a callback/output batch.
- `action`: Rust-owned macro/input action.
- `state`: pending, emitted, denied, failed, or cancelled.
- Validation: synthesized input requests require current-run `SynthesizedInput` capability and bounded output queue capacity.

## Runtime Capability Grant

- `kind`: capability being probed.
- `availability`: available, unsupported, permission-required, denied, revoked, invalidated, or provider-error.
- `diagnostic`: privacy-bounded failure/remediation.
- Validation: any missing or unavailable required capability blocks activation or output enqueue.
