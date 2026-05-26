use signal_auras_core::{
    AdapterDiagnostic, CapabilityAvailability, CapabilityKind, CapabilityReport, CapabilitySet,
    CapabilityStatus, ErrorPhase,
};

pub fn unsupported_report(required: &CapabilitySet, source: &str) -> CapabilityReport {
    CapabilityReport::from_statuses(required.iter().map(|kind| {
        CapabilityStatus::unavailable(
            kind,
            CapabilityAvailability::Unsupported,
            AdapterDiagnostic::new(
                ErrorPhase::CapabilityProbe,
                format!("required Wayland capability '{kind}' is unavailable"),
            )
            .with_capability(kind)
            .with_source(source)
            .with_remediation(
                "use a supported Wayland compositor/session or disable this capability",
            ),
        )
    }))
}

pub fn environment_probe(required: &CapabilitySet) -> CapabilityReport {
    let source = std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown Wayland session".to_string());

    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return unsupported_report(required, "no WAYLAND_DISPLAY");
    }

    CapabilityReport::from_statuses(required.iter().map(|kind| {
        match kind {
            CapabilityKind::GlobalShortcut
            | CapabilityKind::ActiveProcessMetadata
            | CapabilityKind::SynthesizedInput => CapabilityStatus::unavailable(
                kind,
                CapabilityAvailability::Unsupported,
                AdapterDiagnostic::new(
                    ErrorPhase::CapabilityProbe,
                    format!("no supported provider is configured for '{kind}'"),
                )
                .with_capability(kind)
                .with_source(source.clone())
                .with_remediation("add a compositor-specific backend for this capability"),
            ),
        }
    }))
}
