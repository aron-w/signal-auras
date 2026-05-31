# Contract: Scoped Focus Runtime

## Core Contract

`ScopeSelection` exposes a process-scoped activity decision:

```rust
fn scoped_focus_state_at(
    &self,
    active_context: &ActiveProcessContext,
    now: Instant,
    policy: FocusFreshnessPolicy,
) -> ScopedFocusState;
```

Required behavior:
- `ExplicitGlobal` returns active and does not require focus metadata.
- Process scope returns active only for fresh trusted metadata matching the configured process rule.
- Process scope returns inactive for process mismatch, stale focus, unavailable focus, permission-denied focus, ambiguous focus, and untrusted timestamps.
- Inactive states carry privacy-bounded denial fields matching existing `ScopeDenial::render_fields()`.

## Runner Contract

Before process-scoped automation can consume/prevent input or schedule macro output, the runner evaluates `ScopedFocusState`.

Required behavior:
- Inactive hotkey callbacks are ignored without macro scheduling.
- Inactive composite triggers are not recorded as consumed/prevented and do not schedule macros.
- Inactive motion triggers do not schedule macros or repeats.
- Inactive repeat ticks do not execute and cancel active scoped repeat/queued output when caused by a focus transition.
- Grabbed raw input observed while scoped focus is inactive is passed through exactly once when a pass-through guarantee exists.
- Providers that cannot guarantee inactive pass-through fail closed before activation.

## Logging Contract

Info-level transition logs use this field shape:

```text
event=scoped_focus_transition state=<active|inactive> configured_rule=<rule> reason=<reason> [metadata_age_ms=<age>] [stale_threshold_ms=<threshold>]
```

Required behavior:
- Emit one activation log on inactive-to-active transitions.
- Emit one deactivation log on active-to-inactive transitions.
- Do not emit per-event logs when the state is unchanged.
- Do not include command-line arguments, window titles, text payloads, macro payloads, or unrelated process metadata.

## Compatibility Contract

Existing Lua shapes remain valid:

```lua
return {
  scope = { processes = { "kate" } },
  hotkeys = { ["F5"] = macro { text "hello" } },
}
```

No migration, new Lua fields, or ambient capability is required.
