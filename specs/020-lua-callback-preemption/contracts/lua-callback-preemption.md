# Contract: Lua Callback Preemption

## Rust/Core Callback Scheduler

The core callback scheduler MUST expose or support:

- A library-owned default callback execution budget.
- A distinct `Preempted` callback disposition.
- State release for a preempted task equivalent to completed, failed, or cancelled tasks.
- Tests showing a trigger can be scheduled again after its previous invocation is preempted.

## Lua Runtime

The imperative Lua runtime MUST:

- Install a budget enforcement hook around each callback start/resume.
- Count only active Lua execution time, not time spent waiting on host requests.
- Interrupt non-yielding callbacks that exceed the configured budget.
- Convert budget hook interruption into a diagnosable error or runtime result that the runner can classify as preempted.
- Remove or replace budget hooks after callback yield, completion, failure, or preemption.
- Preserve existing behavior for callbacks that complete within budget or yield approved host requests.

## Runner Integration

The live runner MUST:

- Pass each task's budget to the Lua runtime when starting or resuming a callback.
- Mark over-budget callbacks as preempted and release scheduler active state.
- Cancel pending continuations on shutdown without emitting post-cancellation output.
- Log privacy-bounded diagnostics with callback name, trigger label, disposition, elapsed time, and queue depth.
- Continue processing shutdown, timer wakeups, callback wakeups, repeat cancellation, and input/event polling after a callback is preempted.

## Lua API Compatibility

No Lua-facing API names, callback registration syntax, import syntax, or host request syntax change in this feature. The safety boundary is runtime behavior for callbacks that exceed budget.

## Verification Contract

Automated tests MUST cover:

- Infinite loop before first host request.
- Infinite loop after resuming from `sa.sleep`.
- Bounded loop completing within budget.
- Capability denial remains classified as denial, not preemption.
- Scheduler active state release after preemption.
- Live runner shutdown while runaway callback work exists.
