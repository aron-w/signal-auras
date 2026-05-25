use signal_auras_core::{Capability, DiagnosableError, ErrorPhase};

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
