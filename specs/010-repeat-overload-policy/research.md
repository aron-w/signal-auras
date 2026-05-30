# Research: Repeat Overload Policy

## Decision: Default Policy Skips Overlapping Repeat Ticks

Due ticks for a held repeat are skipped when the same repeat already has pending or active output. The runtime records the skip/coalesce count and advances the repeat schedule without replaying missed ticks.

**Rationale**: Queueing missed ticks is unsafe for desktop automation because it can emit stale input after the user releases the hold. Skipping preserves bounded resource use and matches the spec's default lossy policy.

**Alternatives considered**:
- Queue every missed tick: rejected because it can create unbounded pending macro work.
- Keep one coalesced pending tick per repeat: rejected for the first increment because it can still emit after cancellation unless additional generation tracking is introduced.

## Decision: Cancellation Is a Processed-Release Boundary

After the runtime processes a configured cancellation release, no later repeat tick for that hold may schedule output. Output that already started before cancellation may complete through the existing macro queue cancellation semantics.

**Rationale**: This preserves the existing safety contract from the input performance and event-loop work while making overload behavior explicit.

**Alternatives considered**:
- Cancel already-started kernel writes: rejected because lower-level output may already be in progress.
- Let due ticks race cancellation after release observation: rejected because the spec requires processed release priority.

## Decision: Diagnostics Are Counters and Bounded Events

Verbose mode may emit per-skip summary events, but final runtime summaries must include executed, skipped/coalesced, cancelled, and cancelled-run counters. Diagnostics identify the binding trigger label and reason without macro text payloads.

**Rationale**: Operators need tuning data, but diagnostics cannot become their own overload source or leak private input payloads.

**Alternatives considered**:
- Log every missed tick individually: rejected because it can create log overload.
- Only report final totals: rejected because verbose operators need enough lifecycle context to understand active overload.

## Decision: No Lua or Permission Surface Change

The feature changes runtime scheduling behavior only. Existing repeat syntax, Lua validation, consent prompts, unsafe input opt-in, and fail-closed permission behavior remain unchanged.

**Rationale**: The specification requires reliability hardening without migration.

**Alternatives considered**:
- Add a Lua policy knob: rejected as scope expansion and a public API change.
