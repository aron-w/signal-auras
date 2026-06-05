# Data Model: Lua Callback Preemption

## CallbackExecutionBudget

- `max_active_duration`: maximum active Lua execution time allowed for one resume.
- `hook_instruction_interval`: number of Lua VM instructions between deadline checks.
- `tolerance`: documented enforcement allowance caused by hook granularity and scheduler timing.

Validation:

- Durations must be greater than zero.
- Instruction interval must be greater than zero.
- Defaults are owned by library/runtime code, not CLI-local constants.

## LuaCallbackInvocation

- `registration_label`: stable trigger label used by scheduler overload protection.
- `callback`: Lua callback name.
- `accepted_at`: time the task was accepted.
- `budget`: active execution budget for this invocation.
- `execution_elapsed`: accumulated active Lua execution across resumes.
- `state`: pending, active, yielded, completed, failed, cancelled, or preempted.

State transitions:

- `pending -> active` when the runner starts or resumes the callback.
- `active -> yielded` when Lua yields a host request.
- `yielded -> active` when the host response resumes Lua.
- `active -> completed` when Lua finishes within budget.
- `active -> failed` when Lua raises a non-budget error.
- `active -> preempted` when the budget hook interrupts execution.
- `pending|yielded|active -> cancelled` when shutdown cancels work.

## PreemptionDisposition

- Represents callback interruption due to active execution budget.
- Releases scheduler active state.
- Prevents post-timeout output from that callback.
- Records elapsed time, callback name, trigger label, and diagnostic remediation.

## CallbackDiagnosticEvent

- `event`: callback lifecycle diagnostic category.
- `callback`: callback name.
- `trigger`: trigger/registration label.
- `disposition`: accepted, skipped, denied, dropped, completed, slow, failed, cancelled, or preempted.
- `elapsed_bucket`: privacy-safe elapsed timing value or bucket.
- `queue_depth`: current scheduler pending length.

Privacy:

- Must not include active window title, process metadata, input text, or synthesized payload unless those values were already explicitly requested and logged by the script through approved APIs.
