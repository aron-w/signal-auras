# Data Model: Repeat Overload Policy

## Held Repeat

- **Identity**: Runtime motion trigger plus repeat trigger label.
- **Fields**: active/cancelled state, repeat interval, last tick instant, pending/active output status, executed count, skipped/coalesced count, cancelled count.
- **Validation**: A held repeat is active only while its configured hold tokens remain satisfied and before its cancellation release is processed.
- **Relationships**: Owns repeat tick scheduling decisions and contributes to repeat overload diagnostics.

## Repeat Tick

- **Identity**: Scheduled opportunity for a held repeat.
- **Fields**: trigger, due time, interval, decision (`executed`, `skipped_overloaded`, `cancelled`, `denied`).
- **Validation**: A tick may schedule output only when the held repeat is active and no output for the same repeat is pending or active.
- **State transitions**: Due -> executed, due -> skipped/coalesced, due -> cancelled, or due -> denied by scope/permission.

## Repeat Overload Policy

- **Identity**: Built-in default policy for all repeat bindings.
- **Fields**: one-output-at-a-time invariant per held repeat, bounded skip/coalesce counter, no replay flag.
- **Validation**: Must not accumulate unbounded pending macro runs for a held repeat.

## Cancellation Release

- **Identity**: Configured release event for a held repeat input token.
- **Fields**: trigger, processed time, cancelled repeat count, cancelled queued macro run count.
- **Validation**: Once processed, later repeat ticks for the same hold must not emit output.

## Repeat Overload Diagnostic

- **Identity**: Runtime summary or verbose event for repeat lifecycle behavior.
- **Fields**: trigger label, reason, executed count, skipped/coalesced count, cancelled count, optional bounded time range.
- **Privacy**: Must not include macro text payloads, unrelated desktop metadata, or raw private input payloads.
