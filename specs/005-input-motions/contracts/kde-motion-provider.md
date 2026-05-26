# KDE Motion Provider Contract

Signal Auras cannot observe arbitrary global input as a normal Wayland client.
On KDE Plasma Wayland, full input motions require a user-visible KDE/KWin-side
provider that sends constrained events to the Rust runner.

## Provider Responsibilities

The provider MUST run as an explicitly installed and enabled KDE/KWin
integration. It MUST NOT be hidden, autostarted outside user configuration, or
installed by the runner without visible consent.

The provider MUST expose the following current-run capabilities:

- input observation for configured motion tokens only
- optional input consumption for `mode = "consume"`
- press and release events for held-state tracking
- provider shutdown or revocation notification

The provider MUST NOT send raw text, arbitrary unconfigured key streams, window
contents, or unrelated user input to the runner.

## Runner Registration

The runner sends the resolved current-run motion set to the provider:

```text
register_motions(run_id, motions)
```

Each motion contains:

- normalized trigger token list
- mode: `consume` or `passthrough`
- repeat `while_held` tokens, if present
- process scope description, if present

The provider returns:

- success with a provider handle
- unsupported observation
- unsupported consumption
- permission required
- permission denied
- provider error

## Event Delivery

The provider sends only these events:

```text
motion_input(run_id, token, pressed|released)
provider_stopped(run_id, reason)
```

The Rust core owns sequence matching, repeat activation, cancellation, scope
checks, macro execution, and stats. The provider does not execute macros.

## Consumption Semantics

For `consume`, the provider MUST guarantee the matched trigger event does not
reach the focused application. If it cannot guarantee that, it MUST reject
registration before activation.

For `passthrough`, the provider MUST allow the physical event through and still
report the configured token event.

## Repeat Semantics

The provider reports physical press/release state. The runner decides when
repeat is active and stops repeat when any `while_held` token is released.

If the provider later owns wall-clock repeat ticks, each tick MUST still be
scoped to the registered run and stop immediately after provider shutdown,
permission revocation, or held-state release.

## Diagnostics

Every failure maps to a `DiagnosableError` with:

- phase
- capability
- provider source
- remediation

No fallback to partial global observation is allowed.
