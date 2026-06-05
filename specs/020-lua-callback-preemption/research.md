# Research: Lua Callback Preemption

## Decision: Use `mlua` Coroutine Instruction Hooks for the First Increment

Use `mlua::Thread::set_hook` with `HookTriggers::every_nth_instruction` around each active callback resume. The hook checks a Rust-owned deadline and returns a runtime error when the callback exceeds its active execution budget. The hook is installed before each resume and removed or replaced after the callback yields, completes, errors, or is preempted.

**Rationale**: The repository already uses `mlua` 0.10.5 with vendored Lua 5.4. Local crate source documents `Thread::set_hook` for coroutine-specific hooks and `HookTriggers::every_nth_instruction` for execution limits. This directly addresses non-yielding pure Lua loops without adding a worker thread, async runtime, daemon, or new dependency.

**Alternatives considered**:

- Worker thread isolation: stronger wall-clock isolation, but larger architecture change, harder ownership model for `mlua`, and not needed for the first enforceable increment.
- Static source scanning: cannot reliably prove runtime loop bounds and would duplicate sandbox/parser semantics.
- Cooperative-only checks through host APIs: already implemented for `sa.sleep`, but does not protect pure Lua loops.

## Decision: Keep Budget Policy Library-Owned

Define callback execution budget policy in the core/Lua runtime contract, with defaults owned by library code and passed explicitly by the runner.

**Rationale**: The previous architecture review called out CLI-local timing constants as a source of policy drift. Budget behavior affects scheduler state, diagnostics, and testability, so it belongs in reusable Rust boundaries.

**Alternatives considered**:

- CLI-only timeout constants: simpler initially, but repeats the focus freshness policy problem and makes tests less reusable.
- Per-script user configuration in this increment: useful later, but unnecessary for MVP and requires a broader Lua API contract.

## Decision: Add a Distinct Preempted Disposition

Extend callback outcomes with a distinct preempted/timeout disposition instead of reusing slow, failed, or cancelled.

**Rationale**: Slow means completed after budget; failed means callback error; cancelled means shutdown or explicit cancellation. A callback interrupted by budget enforcement is operationally different and should be visible in stats and diagnostics.

**Alternatives considered**:

- Reuse `Slow`: hides the fact that the callback did not complete.
- Reuse `Failed`: makes script bugs and runtime safety interruption indistinguishable.
- Reuse `Cancelled`: hides whether cancellation was shutdown-driven or budget-driven.

## Decision: Preserve Host-Yielding Continuation Semantics

Budget applies to active Lua execution time during each resume. Time spent pending on `sa.sleep`, timer wakeups, window host requests, or scheduler queueing does not count as active Lua execution.

**Rationale**: This preserves the behavior introduced by callback responsiveness work and prevents long sleeps from being misclassified as runaway CPU execution.

**Alternatives considered**:

- Wall-clock lifetime budget from callback acceptance: simpler accounting, but incorrectly penalizes legitimate host-yielding callbacks.
- No budget on resumed callbacks: leaves post-resume tight loops unprotected.
