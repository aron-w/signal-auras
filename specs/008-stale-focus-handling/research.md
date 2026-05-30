# Research: Stale Focus Handling

## Decision: Evaluate focus freshness in the core scope model

Freshness affects whether a process-scoped binding may emit actions, so it belongs beside `ScopeSelection` and `ActiveProcessContext` in `signal-auras-core`. CLI and Wayland code consume the outcome instead of duplicating safety decisions.

## Decision: Treat the exact threshold boundary as fresh

Metadata is stale only when its age is greater than the active threshold. This makes the boundary deterministic and lets tests cover below, exactly at, and above the default 2 second threshold.

## Decision: Represent unavailable, denied, stale, untrusted timestamp, and process mismatch as distinct denial kinds

Distinct denial kinds satisfy diagnosability without expanding the Lua API. They also let runtime counters treat stale/unavailable metadata differently from a true process mismatch.

## Decision: Keep diagnostics privacy-bounded to configured rule names and freshness fields

Stale-focus diagnostics may include the configured process rule, denial kind, metadata age, and threshold. They must not include command-line arguments, window title text, unrelated process data, or hidden compositor metadata.

## Decision: Reuse the existing active-process provider boundary

The KDE bridge already produces `ActiveProcessContext` values and the runner already asks for the current context per trigger. This feature does not add a persistent global cache or new compositor query path.
