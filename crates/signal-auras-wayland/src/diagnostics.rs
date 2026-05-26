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

pub fn reserved_shortcut(hotkey: &str) -> DiagnosableError {
    DiagnosableError::new(
        ErrorPhase::Registration,
        format!("hotkey '{hotkey}' is reserved or already owned by the session"),
    )
    .with_capability(Capability::GlobalShortcut)
    .with_remediation("choose a different hotkey")
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
