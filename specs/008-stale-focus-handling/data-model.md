# Data Model: Stale Focus Handling

## Focus Snapshot

- **Fields**: visible process name, optional process id, optional app id, optional window class, confidence, captured timestamp, optional adapter diagnostic.
- **Validation**: exact/name-only confidence can be matched only when the snapshot timestamp is trusted and fresh; unavailable, denied, ambiguous, stale, or future/unordered timestamps are unknown for process-scoped matching.

## Focus Freshness Policy

- **Fields**: stale threshold.
- **Validation**: default threshold is 2 seconds; metadata is fresh when age is less than or equal to the threshold and stale when age is greater than the threshold.

## Process Match Rule

- **Fields**: configured process names from Lua scope or current-run prompt.
- **Validation**: process list must be non-empty and printable. Diagnostics may include these configured values because the user supplied them.

## Stale-Focus Denial

- **Fields**: denial kind, user-facing reason, configured rule description, optional metadata age, optional stale threshold.
- **Validation**: denial is produced before macro scheduling or input emission. Diagnostic fields must not include private command-line arguments, window text, or unrelated process details.

## State Transitions

- Fresh matching snapshot -> process-scoped macro allowed.
- Fresh non-matching snapshot -> process mismatch denial.
- Fresh unavailable/denied/ambiguous snapshot -> metadata denial.
- Fresh snapshot ages beyond threshold -> stale-focus denial.
- New fresh matching snapshot after a denial -> process-scoped matching resumes on the next trigger.
