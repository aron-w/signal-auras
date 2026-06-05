# Contract: Runner Boundaries

## Lifecycle Boundary

- Accepts `LifecycleConfig`.
- Returns `RuntimeSession` or a diagnosable startup error.
- On startup error after partial acquisition, releases all acquired resources through `RuntimeSession` cleanup behavior where practical.
- Cleanup may be called multiple times and returns stable, diagnosable results.

## Runtime Loop Boundary

- Accepts a mutable session, policy/configuration values, and wake source adapters.
- Drains ready work without blocking on Lua callbacks or host sleeps.
- Stops scheduling new macro/controller work once shutdown starts.
- Emits privacy-bounded diagnostics for wake ordering and dropped/denied work.

## Controller Boundary

- Executes pending Lua controller work through Rust-owned budgets and capabilities.
- Represents sleep/yield as pending scheduled work rather than blocking the runtime loop.
- Reports completed, pending, slow, failed, denied, skipped, cancelled, and dropped dispositions.

## Compatibility

- Public Lua declarations and controller APIs remain source-compatible.
- Consent prompts and fail-closed permission behavior remain unchanged.
- Existing verification commands must pass after each boundary extraction.
