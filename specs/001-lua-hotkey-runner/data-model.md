# Data Model: Lua Hotkey Runner

## RunnerInvocation

Represents one terminal-started run.

**Fields**:
- `lua_file_path`: path supplied as the single CLI argument
- `stdin_is_interactive`: whether terminal prompting is possible
- `started_at`: monotonic start time
- `shutdown_reason`: `ctrl_c`, `startup_error`, or `runtime_error`
- `effective_scope`: `ScopeSelection` after script validation and optional prompt

**Validation Rules**:
- Exactly one Lua file path is required.
- The file must be readable by the host before Lua evaluation.
- No hotkey registration may start until `effective_scope` is available.

## LuaAutomationConfiguration

Validated data returned by the Lua script.

**Fields**:
- `api_version`: implicit v1 script contract for this feature
- `scope`: optional `ScriptScope`
- `hotkeys`: non-empty map of `HotkeyId` to `MacroDefinition`

**Validation Rules**:
- Script must return a table/object shaped as the v1 contract.
- `hotkeys` must be present and non-empty.
- Unsupported fields may be ignored only if documented; fields that look like capabilities must be rejected.
- Script evaluation must occur in the sandbox environment.

## ScriptScope

Scope declared inside Lua.

**Fields**:
- `processes`: non-empty list of user-visible executable/process names

**Validation Rules**:
- Empty scope is invalid.
- Process names must be non-empty printable strings.
- Script-declared global scope is not accepted in v1; global behavior requires terminal selection.

## ScopeSelection

Effective scope for the current run.

**Variants**:
- `ProcessList { processes }`
- `ExplicitGlobal`

**Validation Rules**:
- `ProcessList` requires at least one process name.
- `ExplicitGlobal` can only be created from terminal prompt confirmation.
- Prompt-created scope is current-run memory only.

## HotkeyBinding

Connects a hotkey to an effective scope and macro.

**Fields**:
- `hotkey`: normalized hotkey identifier
- `scope`: `ScopeSelection`
- `macro_definition`: `MacroDefinition`
- `registration_state`: `pending`, `registered`, `failed`, or `unregistered`

**Validation Rules**:
- Duplicate hotkey identifiers are invalid before registration.
- Unsupported hotkey identifiers are invalid before registration.
- Registration failure must include a diagnosable error.

## MacroDefinition

Ordered list of supported macro actions.

**Fields**:
- `actions`: ordered non-empty list of `MacroAction`
- `concurrency_policy`: `deny_while_running` for v1

**Validation Rules**:
- Empty macros are invalid.
- Actions are executed in declared order.
- Re-triggering the same running macro is denied and counted.

## MacroAction

Single macro operation.

**Variants**:
- `KeyPress { key }`
- `TextInput { text }`
- `Delay { duration_ms }`

**Validation Rules**:
- `key` must be supported by the selected adapter.
- `text` must be non-empty.
- `duration_ms` must be finite, positive, and within the maximum chosen during implementation.

## RuntimeStats

Auditable counters and timing for one runner process.

**Fields**:
- `started_at`
- `elapsed_runtime`
- `registration_attempts`
- `registration_successes`
- `registration_failures`
- `trigger_count_by_hotkey`
- `macro_success_count`
- `macro_failure_count`
- `denied_action_count`
- `permission_failure_count`
- `scope_mismatch_count`

**Validation Rules**:
- Counters start at zero.
- Denied triggers must not increment macro success.
- Failed macro execution must include the action and reason.
- Final summary must be printable after Ctrl-C and startup/runtime errors.

## DiagnosableError

User-visible failure with phase and remediation.

**Fields**:
- `phase`: `argument_validation`, `script_load`, `script_validation`, `scope_prompt`, `capability_probe`, `registration`, `trigger`, `macro_execution`, or `shutdown`
- `capability`: optional capability name, such as `global_shortcut`, `active_process`, or `synthesized_input`
- `message`: concise user-facing explanation
- `remediation`: optional next action or missing permission/protocol hint

**Validation Rules**:
- Unsupported compositor/protocol/permission cases must use this shape.
- Errors must not expose sensitive script internals beyond what is needed to diagnose the run.

## State Transitions

```text
created
  -> script_loaded
  -> config_validated
  -> scope_resolved
  -> capabilities_checked
  -> hotkeys_registered
  -> running
  -> shutting_down
  -> stopped
```

Failure transitions may occur before registration or during runtime. Startup failures stop before `hotkeys_registered`. Ctrl-C transitions from prompt, running, or macro execution into `shutting_down`.
