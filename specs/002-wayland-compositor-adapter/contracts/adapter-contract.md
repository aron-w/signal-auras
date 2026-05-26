# Adapter Contract: KDE Plasma Wayland Adapter

## Provider Selection

The real adapter must select the KDE provider only when the current session is KDE Plasma Wayland:

```rust
pub trait WaylandProviderSelector {
    fn select_provider(&mut self) -> Result<SelectedProvider, DiagnosableError>;
}
```

Contract requirements:

- Non-Wayland sessions return an unsupported-session diagnostic.
- Non-KDE sessions return an unsupported-provider diagnostic.
- Missing KWin, KDE global shortcut, or xdg-desktop-portal-kde paths return capability-specific diagnostics.
- Selection must not register shortcuts, install bridge state, or open input sessions.

## Capability Probe

The KDE provider must expose a probe step before registration:

```rust
pub trait KdeCapabilityProbe {
    fn probe_capabilities(&mut self, required: CapabilitySet) -> CapabilityReport;
}
```

`CapabilityReport` must distinguish:

- available capability
- unsupported KDE service, portal, protocol, or provider
- permission required
- permission denied
- capability revoked or invalidated
- provider error with user-visible remediation

Registration, active-process reads, and synthesized input must not proceed for capabilities reported as unsupported, denied, revoked, invalidated, or provider-error.

## Current-Run KDE Bridge

If KDE shortcut event delivery or active-window reads require a bridge, it must be visible and current-run only:

```rust
pub trait KdeBridge {
    fn install(&mut self, registrations: &[HotkeyBinding]) -> Result<KdeBridgeHandle, DiagnosableError>;
    fn unload(&mut self) -> Result<CleanupReport, DiagnosableError>;
}
```

Contract requirements:

- Bridge installation requires explicit current-run intent.
- Bridge state must not persist across process exit.
- Bridge setup failures must occur before shortcut activation.
- `unload` is idempotent and reports cleanup for D-Bus objects and any KWin script state.

## Global Shortcut Registration

```rust
pub trait KdeShortcutRegistrar {
    fn register_shortcut(
        &mut self,
        binding: HotkeyBinding,
    ) -> Result<ShortcutRegistrationHandle, DiagnosableError>;

    fn next_shortcut_event(&mut self) -> Result<ShortcutEvent, DiagnosableError>;

    fn unregister_all(&mut self) -> Result<CleanupReport, DiagnosableError>;
}
```

Contract requirements:

- Registration requires resolved current-run scope and granted shortcut capability.
- Reserved, unsupported, or already-owned shortcuts return a diagnosable registration error.
- Each successful registration returns an owned KDE provider handle.
- `unregister_all` is idempotent and reports cleanup status for every owned handle.
- No shortcut event may trigger macro execution after shutdown begins.

## Active Process Metadata

```rust
pub trait KdeActiveProcessProvider {
    fn active_process_context(&mut self) -> Result<ActiveProcessContext, DiagnosableError>;
}
```

Contract requirements:

- Context is read from KDE/KWin when a shortcut event is handled.
- Context includes a visible name when available and may include app id, window class, or PID.
- Missing, denied, ambiguous, privileged, compositor-owned, or stale metadata returns a non-match outcome for process-scoped shortcuts.
- The adapter must not cache stale metadata as if it were fresh.

## Synthesized Input

```rust
pub trait KdeInputSynthesizer {
    fn synthesize(&mut self, request: SynthesizedInputRequest) -> Result<InputEmission, DiagnosableError>;
    fn cancel_pending(&mut self) -> Result<(), DiagnosableError>;
}
```

Contract requirements:

- Synthesized input requires a granted current-run input capability separate from shortcut registration.
- Key and text actions are emitted through the KDE/portal input path in declared macro order.
- Text input must translate to supported key events before emission; unsupported text fails without partial emission.
- Denied or unavailable input emits zero input and returns a diagnosable error.
- Shutdown cancels pending input before adapter stop completes.
- The same hotkey's macro cannot overlap with itself.

## Diagnostics

Every adapter failure must map to `DiagnosableError` with:

- phase
- capability when applicable
- KDE/portal source when applicable
- user-facing message
- remediation when available

The adapter must avoid silent fallback behavior. Unsupported KDE, portal, and non-KDE provider paths must be visible to the user and to tests.

## Test Obligations

- Provider selection distinguishes KDE Wayland, non-KDE Wayland, X11, and missing session state.
- Probe reports each supported and unsupported capability state.
- Bridge installation and unload are current-run, visible, idempotent, and cleaned up after partial failure.
- Registration success returns handles and increments stats.
- Registration rejection cleans up prior successful handles.
- Event handling evaluates process context at handling time.
- Missing metadata denies process-scoped shortcuts without macro execution.
- Synthesized input denial emits no input and increments denial stats.
- Text input unsupported by portal translation emits no partial text.
- Shutdown unregisters handles, closes portal sessions, unloads bridge state, and cancels pending input.

