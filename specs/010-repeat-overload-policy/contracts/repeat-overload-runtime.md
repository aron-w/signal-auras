# Repeat Overload Runtime Contract

## Scope

This contract covers the live runner and testable Rust runtime behavior for held repeat macros. It does not change the Lua configuration contract.

## Scheduling Rules

- A repeat tick may start macro output only if the held repeat is still active and there is no pending or active repeat macro run for the same binding.
- If a tick is due while output for the same held repeat is pending or active, the tick is skipped/coalesced and counted.
- Skipped/coalesced ticks are never replayed after overload clears, after cancellation, or during shutdown.
- Distinct held repeat bindings keep independent pending/active state and counters.

## Cancellation Rules

- Runtime input processing has priority over due repeat ticks after release input is observed by the event loop.
- Once a cancellation release is processed, no later tick for that held repeat may start output.
- Already-started output may finish through the existing output adapter semantics, but queued repeat macro runs that have not started are cancelled.

## Diagnostics Rules

- Verbose diagnostics identify repeat lifecycle events using trigger labels and bounded counters.
- Final summaries include counts for executed repeat ticks, skipped/coalesced repeat ticks, repeat cancellations, and cancelled queued macro runs.
- Diagnostics must not include macro text payloads or unrelated desktop metadata.

## Failure Rules

- Permission denial or revoked input/output capabilities fail closed using existing diagnosable errors.
- Scope/process denial prevents repeat output and increments existing denial diagnostics without weakening overload invariants.
