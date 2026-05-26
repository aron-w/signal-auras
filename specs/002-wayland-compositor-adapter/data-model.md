# Data Model: KDE Plasma Wayland Adapter

## KdeSession

Represents the current desktop session as observed by the adapter.

**Fields**:
- `wayland_display`: Wayland display name from the environment
- `desktop_session`: observed desktop/session value
- `kde_services`: availability of KWin, KGlobalAccel, and xdg-desktop-portal-kde paths
- `state`: `unsupported`, `available`, `permission_required`, `invalidated`, or `provider_error`
- `diagnostic`: optional `AdapterDiagnostic`

**Validation Rules**:
- Session state must be known before any shortcut registration, metadata read, or input synthesis.
- Non-Wayland, non-KDE, missing KWin, missing portal, and invalidated sessions fail closed.
- Session diagnostics must identify the missing KDE or portal component when known.

## CompositorCapability

Represents one sensitive desktop-session capability.

**Fields**:
- `kind`: `global_shortcut`, `active_process_metadata`, or `synthesized_input`
- `availability`: `available`, `unsupported`, `permission_required`, `denied`, `revoked`, `invalidated`, or `provider_error`
- `source`: provider description, such as `kde-plasma`, `kwin`, `kglobalaccel`, or `xdg-desktop-portal-kde`
- `reason`: optional diagnosable explanation
- `remediation`: optional user action

**Validation Rules**:
- A capability must be probed before dependent behavior activates.
- Unsupported, denied, revoked, invalidated, and provider-error capabilities fail closed.
- Capability diagnostics must be safe to print in terminal output.

## CapabilityGrant

Represents current-run user consent or session permission for a capability.

**Fields**:
- `capability`: associated `CompositorCapability`
- `scope`: current-run scope covered by the grant
- `provider_session`: optional KDE or portal session identifier
- `granted_at`: monotonic timestamp or event sequence
- `revoked_at`: optional monotonic timestamp or event sequence

**Validation Rules**:
- Grants are current-run only.
- Global shortcut, KWin bridge, metadata, and synthesized-input grants must be explicit and visible.
- A revoked grant cannot be used for registrations, metadata reads, or input emission.

## KdeBridgeState

Represents current-run KDE bridge state used to receive events or query KWin when direct in-process APIs are unavailable.

**Fields**:
- `dbus_name`: current-run D-Bus service or unique name
- `object_path`: current-run object path for callback/event delivery
- `kwin_script_id`: optional temporary KWin script identifier
- `state`: `not_installed`, `installing`, `active`, `unloading`, `unloaded`, or `failed`
- `diagnostic`: optional `AdapterDiagnostic`

**Validation Rules**:
- Bridge state is created only after visible current-run consent.
- Bridge state must never be autostarted or persisted across runs.
- Active bridge state must transition to `unloaded` or report cleanup failure on shutdown.

## ShortcutRegistration

Represents an owned registration with KDE global shortcut infrastructure.

**Fields**:
- `hotkey`: normalized hotkey identifier
- `scope`: effective current-run scope
- `provider_handle`: KDE registration action, object path, or provider token
- `state`: `pending`, `registered`, `rejected`, `unregistering`, or `unregistered`
- `diagnostic`: optional `AdapterDiagnostic`

**Validation Rules**:
- Registration cannot enter `registered` until KDE capability probing and consent are complete.
- Rejected registrations must include a reason, such as reserved shortcut, unsupported key, denied permission, missing provider, or session error.
- Every registered handle must transition to `unregistered` during normal or error cleanup.

## ShortcutEvent

Represents a desktop shortcut event delivered by the KDE provider.

**Fields**:
- `hotkey`: normalized hotkey identifier
- `registration_handle`: source registration handle
- `received_at`: monotonic timestamp or event sequence
- `active_context`: optional `ActiveProcessContext`

**Validation Rules**:
- Events for unknown or unregistered handles are ignored and counted.
- Process-scoped eligibility is evaluated when the event is handled.
- Events after shutdown begins must not trigger macro execution.

## ActiveProcessContext

Represents focused application or process metadata available from KDE/KWin.

**Fields**:
- `visible_name`: optional user-visible application or process name
- `process_id`: optional process identifier if KDE exposes it
- `app_id`: optional desktop file id or app id
- `window_class`: optional KWin window resource class
- `confidence`: `exact`, `name_only`, `ambiguous`, `unavailable`, or `denied`
- `captured_at`: monotonic timestamp or event sequence
- `diagnostic`: optional `AdapterDiagnostic`

**Validation Rules**:
- `exact` or `name_only` context may be used for configured process-name matching.
- `ambiguous`, `unavailable`, and `denied` contexts are non-matches for process-scoped shortcuts.
- Stale context must not be reused silently when a fresh KDE read fails.
- Privileged, compositor-owned, launcher, or lock-screen surfaces must become non-match contexts.

## SynthesizedInputRequest

Represents a key or text macro action submitted to the KDE/portal input path.

**Fields**:
- `action`: key press or text input action
- `sequence`: declared macro action order
- `portal_session`: current-run RemoteDesktop session identifier
- `target_context`: optional active process context at emission time
- `state`: `pending`, `emitted`, `denied`, `failed`, or `cancelled`
- `diagnostic`: optional `AdapterDiagnostic`

**Validation Rules**:
- Requests require synthesized-input capability and current-run consent.
- Requests execute in macro order.
- Text requests must translate to supported ordered key events or fail before emitting partial text.
- Requests after shutdown begins are cancelled.
- Denied or failed requests emit no further input in that macro.

## AdapterDiagnostic

Represents user-visible adapter feedback.

**Fields**:
- `phase`: `session_probe`, `capability_probe`, `bridge_setup`, `registration`, `event_delivery`, `active_process`, `synthesized_input`, or `shutdown`
- `capability`: optional capability kind
- `message`: concise user-facing explanation
- `remediation`: optional user action
- `source`: optional provider description

**Validation Rules**:
- Diagnostics must identify the affected KDE/portal capability when applicable.
- Diagnostics must not expose sensitive data beyond what is needed for user action.
- Every failed adapter operation must produce or map to a diagnosable error.

## RuntimeStats Extensions

Adds KDE adapter-specific counters to the existing one-run stats model.

**Fields**:
- `capability_probe_success_count`
- `capability_probe_failure_count`
- `shortcut_event_ignored_count`
- `active_process_match_count`
- `active_process_non_match_count`
- `metadata_unavailable_count`
- `synthesized_input_emitted_count`
- `synthesized_input_denied_count`
- `kde_bridge_setup_count`
- `kde_bridge_cleanup_count`
- `cleanup_success_count`
- `cleanup_failure_count`

**Validation Rules**:
- Counters start at zero for each run.
- Denied or unavailable synthesized input must not increment emitted count.
- Cleanup counts must be printed even after runtime errors when terminal output is available.

## State Transitions

```text
adapter_created
  -> kde_session_probed
  -> capabilities_probed
  -> permissions_resolved
  -> kde_bridge_active
  -> shortcuts_registered
  -> event_loop_running
  -> shutting_down
  -> shortcuts_unregistered
  -> portal_session_closed
  -> kde_bridge_unloaded
  -> adapter_stopped
```

Failure transitions may occur during session probing, capability probing, bridge setup, permission resolution, registration, event handling, synthesized input, or shutdown. Any failure after registration or bridge setup must attempt cleanup before returning control to the user.

