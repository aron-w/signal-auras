# Rust Library Contract: KDE Plasma Wayland Adapter

## Core Responsibilities

`signal-auras-core` continues to own pure automation semantics:

- capability requirement modeling
- current-run consent decisions
- registration lifecycle state validation
- active-process scope matching
- macro scheduling and no-overlap enforcement
- runtime stats accounting
- diagnosable error shape and phase mapping

Core code must not call Wayland, KDE, portal, D-Bus, process inspection, terminal, filesystem, or Lua APIs directly.

## Wayland Crate Responsibilities

`signal-auras-wayland` owns desktop-session side effects:

- detect KDE Plasma Wayland sessions
- reject non-KDE and non-Wayland sessions explicitly
- probe KWin, KDE global shortcut, active-process metadata, and xdg-desktop-portal-kde synthesized-input capabilities
- request or observe permission outcomes when KDE or portals require them
- create and remove current-run KDE bridge state when required
- register, deliver, and unregister global shortcut handles
- read focused application/process metadata from KDE/KWin
- synthesize approved key and text input through KDE/portal paths
- translate unsupported provider behavior into core diagnosable errors

Any unsafe Rust, FFI, D-Bus object lifetime, KWin script lifetime, portal session lifetime, thread/event-loop ownership, or permission bridge must be isolated in this crate and documented near the boundary.

## CLI Responsibilities

`signal-auras-cli` composes the existing Lua configuration loader, scope prompt, core decisions, and selected KDE adapter:

- load and validate the Lua script before probing or registration
- resolve current-run scope and consent
- select the real KDE provider only for KDE Plasma Wayland
- probe required capabilities before registration
- print session, capability, bridge, registration, event, input, and cleanup diagnostics
- unregister shortcuts, close portal sessions, unload KDE bridge state, and cancel pending input on Ctrl-C or runtime error
- print final stats

## Lua Responsibilities

`signal-auras-lua` remains configuration-only:

- scripts can declare scopes, hotkeys, and macro actions
- scripts cannot query KDE session state
- scripts cannot inspect active-process metadata directly
- scripts cannot call synthesized-input APIs directly
- script API stability is preserved for this feature

## Required Test Contracts

- Core tests model capability requirement sets without compositor dependencies.
- Core tests deny process-scoped execution when active-process context is missing, stale, ambiguous, privileged, or denied.
- Core tests ensure synthesized-input denial does not count as emitted input or macro success.
- Adapter mock tests cover every KDE capability state and registration lifecycle transition.
- KDE provider selection tests cover KDE Wayland, non-KDE Wayland, X11, missing KWin, and missing portal cases.
- KDE bridge tests cover setup, callback/event mapping, unload, idempotent cleanup, and partial failure cleanup.
- Portal/input tests cover ordered key emission, text-to-key translation, denied permission, provider failure, cancellation, and no partial text emission.
- CLI contract tests verify startup fails before registration when required KDE capabilities are unavailable.
- Integration tests verify cleanup is attempted after registration, bridge setup, portal session creation, or runtime failure.
- Lua sandbox tests continue to verify scripts cannot access active-process metadata or synthesized input directly.

