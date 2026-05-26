use signal_auras_core::{Capability, CapabilityKind, DiagnosableError, ErrorPhase};

pub fn unsupported_protocol(capability: Capability) -> DiagnosableError {
    DiagnosableError::new(
        ErrorPhase::CapabilityProbe,
        "required Wayland protocol or portal is unavailable",
    )
    .with_capability(capability)
    .with_remediation("use a supported Wayland compositor/session or disable this capability")
}

pub fn denied_permission(capability: Capability) -> DiagnosableError {
    DiagnosableError::new(
        ErrorPhase::CapabilityProbe,
        "required permission was denied",
    )
    .with_capability(capability)
    .with_remediation("grant the requested permission and restart the runner")
}

pub fn revoked_permission(capability: Capability) -> DiagnosableError {
    DiagnosableError::new(
        ErrorPhase::CapabilityProbe,
        "required permission was revoked",
    )
    .with_capability(capability)
    .with_remediation("restart the runner and grant the requested permission")
}

pub fn invalidated_provider(capability: Capability) -> DiagnosableError {
    DiagnosableError::new(
        ErrorPhase::CapabilityProbe,
        "Wayland provider was invalidated",
    )
    .with_capability(capability)
    .with_remediation("restart the runner after the compositor/session is stable")
}

pub fn unsupported_kde_session(source: impl Into<String>) -> DiagnosableError {
    DiagnosableError::new(
        ErrorPhase::CapabilityProbe,
        "KDE Plasma Wayland provider requires a KDE Plasma Wayland session",
    )
    .with_source(source)
    .with_remediation("start Signal Auras from a KDE Plasma Wayland session")
}

pub fn missing_kde_service(service: impl Into<String>) -> DiagnosableError {
    let service = service.into();
    DiagnosableError::new(
        ErrorPhase::CapabilityProbe,
        format!("required KDE service '{service}' is unavailable"),
    )
    .with_source(service)
    .with_remediation("enable the required KDE Plasma service and restart the runner")
}

pub fn reserved_shortcut(hotkey: &str) -> DiagnosableError {
    DiagnosableError::new(
        ErrorPhase::Registration,
        format!("hotkey '{hotkey}' is reserved or already owned by the session"),
    )
    .with_capability(Capability::GlobalShortcut)
    .with_remediation("choose a different hotkey")
}

pub fn unsupported_shortcut_key(hotkey: &str) -> DiagnosableError {
    DiagnosableError::new(
        ErrorPhase::Registration,
        format!("hotkey '{hotkey}' is not supported by the KDE shortcut provider"),
    )
    .with_capability(Capability::GlobalShortcut)
    .with_source("kglobalaccel")
    .with_remediation("choose a KDE-supported key combination")
}

pub fn unsupported_kind(kind: CapabilityKind, source: impl Into<String>) -> DiagnosableError {
    DiagnosableError::new(
        ErrorPhase::CapabilityProbe,
        format!("required capability '{kind}' is unsupported"),
    )
    .with_capability(kind.legacy_capability())
    .with_source(source)
    .with_remediation("use a supported Wayland compositor/session or disable this capability")
}
