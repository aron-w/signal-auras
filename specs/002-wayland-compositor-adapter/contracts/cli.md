# CLI Contract: KDE Plasma Wayland Adapter

## Command

```text
signal-auras run <lua-file>
```

No new user-facing command is introduced for this feature.

## Startup Output

The runner must emit user-visible terminal output for:

- script path and validation result
- effective current-run scope
- detected session and selected provider
- explicit unsupported message for non-KDE or non-Wayland sessions
- required capability set
- KDE global shortcut capability probe result
- KDE/KWin active-process metadata capability probe result when process-scoped shortcuts are configured
- xdg-desktop-portal-kde synthesized-input capability probe result when key or text macro actions are configured
- current-run bridge setup when KDE event or metadata integration requires it
- permission denial or unsupported provider details
- registration result per hotkey

## Runtime Output

The runner must emit user-visible terminal output for:

- delivered shortcut events
- active-process match and non-match decisions when available
- ignored events with reasons
- synthesized-input completion, denial, translation failure, or provider failure
- capability revocation or provider invalidation
- shutdown cleanup result for shortcuts, portal sessions, and KDE bridge state
- final summary stats

## Exit Behavior

- Script or argument validation failure: non-zero exit; no capability probing required.
- Non-KDE or non-Wayland session: non-zero exit; no KDE bridge, portal session, or shortcut registration created.
- Capability unsupported or permission denied before registration: non-zero exit; no shortcuts registered.
- KDE bridge setup failure: non-zero exit; no shortcuts registered.
- Partial registration failure: non-zero exit after unregistering any previously registered handles and unloading bridge state.
- Runtime provider invalidation: non-zero exit after cleanup and final stats.
- Synthesized-input failure during macro execution: non-zero exit after macro cancellation policy is applied and cleanup completes.
- Ctrl-C during a successful run: zero exit after unregistering shortcuts, cancelling pending input, closing portal sessions, unloading bridge state, and printing stats.

## Diagnostics Format

Diagnostic output must identify:

- phase
- capability when applicable
- provider/source, such as KDE Plasma, KWin, KGlobalAccel, or xdg-desktop-portal-kde
- hotkey when applicable
- active-process decision when applicable
- concise reason
- remediation when available

Exact formatting may evolve during implementation, but tests must be able to assert the fields above.

