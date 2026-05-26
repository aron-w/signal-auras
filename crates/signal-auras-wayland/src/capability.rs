use signal_auras_core::{
    AdapterDiagnostic, CapabilityAvailability, CapabilityKind, CapabilityReport, CapabilitySet,
    CapabilityStatus, DiagnosableError, ErrorPhase,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KdeEnvironment {
    pub wayland_display: Option<String>,
    pub session_type: Option<String>,
    pub current_desktop: Option<String>,
    pub services: KdeServiceAvailability,
}

impl KdeEnvironment {
    pub fn from_process_env() -> Self {
        Self {
            wayland_display: std::env::var("WAYLAND_DISPLAY").ok(),
            session_type: std::env::var("XDG_SESSION_TYPE").ok(),
            current_desktop: std::env::var("XDG_CURRENT_DESKTOP").ok(),
            services: KdeServiceAvailability::from_process_env(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KdeServiceAvailability {
    pub kwin: bool,
    pub kglobalaccel: bool,
    pub portal: bool,
}

impl KdeServiceAvailability {
    pub fn available() -> Self {
        Self {
            kwin: true,
            kglobalaccel: true,
            portal: true,
        }
    }

    pub fn from_process_env() -> Self {
        Self {
            kwin: env_flag("SIGNAL_AURAS_KDE_KWIN"),
            kglobalaccel: env_flag("SIGNAL_AURAS_KDE_GLOBAL_SHORTCUTS"),
            portal: env_flag("SIGNAL_AURAS_KDE_PORTAL"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KdeSessionState {
    Unsupported,
    Available,
    PermissionRequired,
    Invalidated,
    ProviderError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KdeSession {
    pub wayland_display: String,
    pub desktop_session: String,
    pub services: KdeServiceAvailability,
    pub state: KdeSessionState,
}

impl KdeSession {
    pub fn detect(environment: KdeEnvironment) -> Result<Self, DiagnosableError> {
        let wayland_display = environment
            .wayland_display
            .filter(|value| !value.is_empty());
        let session_type = environment
            .session_type
            .unwrap_or_else(|| "unknown".to_string());
        if wayland_display.is_none() || !session_type.eq_ignore_ascii_case("wayland") {
            return Err(session_error(
                "KDE Plasma Wayland requires a Wayland session",
                "kde-plasma",
            ));
        }

        let desktop = environment
            .current_desktop
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        if !desktop
            .split(':')
            .any(|part| part.eq_ignore_ascii_case("KDE"))
        {
            return Err(session_error(
                "KDE Plasma Wayland provider requires a KDE Plasma Wayland session",
                desktop,
            ));
        }

        if !environment.services.kwin {
            return Err(session_error(
                "required KDE service KWin is unavailable",
                "kwin",
            ));
        }
        if !environment.services.kglobalaccel {
            return Err(session_error(
                "required KDE global shortcut service KGlobalAccel is unavailable",
                "kglobalaccel",
            ));
        }
        if !environment.services.portal {
            return Err(session_error(
                "required KDE portal xdg-desktop-portal-kde is unavailable",
                "xdg-desktop-portal-kde",
            ));
        }

        Ok(Self {
            wayland_display: wayland_display.unwrap(),
            desktop_session: desktop,
            services: environment.services,
            state: KdeSessionState::Available,
        })
    }
}

pub fn unsupported_report(required: &CapabilitySet, source: &str) -> CapabilityReport {
    CapabilityReport::from_statuses(required.iter().map(|kind| {
        CapabilityStatus::unavailable(
            kind,
            CapabilityAvailability::Unsupported,
            AdapterDiagnostic::new(
                ErrorPhase::CapabilityProbe,
                format!("required KDE Plasma Wayland capability '{kind}' is unavailable"),
            )
            .with_capability(kind)
            .with_source(source)
            .with_remediation(
                "use a supported KDE Plasma Wayland session or disable this capability",
            ),
        )
    }))
}

pub fn environment_probe(required: &CapabilitySet) -> CapabilityReport {
    kde_capability_report(required, &KdeEnvironment::from_process_env())
}

pub fn kde_capability_report(
    required: &CapabilitySet,
    environment: &KdeEnvironment,
) -> CapabilityReport {
    if environment.wayland_display.is_none()
        || !environment
            .session_type
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("wayland"))
    {
        return unsupported_report(required, "no KDE Plasma Wayland session");
    }

    let desktop = environment
        .current_desktop
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    if !desktop
        .split(':')
        .any(|part| part.eq_ignore_ascii_case("KDE"))
    {
        return unsupported_report(required, "non-KDE Wayland session");
    }

    CapabilityReport::from_statuses(required.iter().map(|kind| match kind {
        CapabilityKind::GlobalShortcut if environment.services.kglobalaccel => {
            CapabilityStatus::available(kind, "kglobalaccel")
        }
        CapabilityKind::ActiveProcessMetadata if environment.services.kwin => {
            CapabilityStatus::available(kind, "kwin")
        }
        CapabilityKind::SynthesizedInput if environment.services.portal => {
            CapabilityStatus::available(kind, "xdg-desktop-portal-kde")
        }
        CapabilityKind::GlobalShortcut => unavailable_kind(
            kind,
            "KDE global shortcut service KGlobalAccel is unavailable",
            "kglobalaccel",
        ),
        CapabilityKind::ActiveProcessMetadata => unavailable_kind(
            kind,
            "KDE active-process metadata service KWin is unavailable",
            "kwin",
        ),
        CapabilityKind::SynthesizedInput => unavailable_kind(
            kind,
            "KDE RemoteDesktop portal is unavailable",
            "xdg-desktop-portal-kde",
        ),
    }))
}

fn unavailable_kind(
    kind: CapabilityKind,
    message: impl Into<String>,
    source: impl Into<String>,
) -> CapabilityStatus {
    CapabilityStatus::unavailable(
        kind,
        CapabilityAvailability::Unsupported,
        AdapterDiagnostic::new(ErrorPhase::CapabilityProbe, message)
            .with_capability(kind)
            .with_source(source)
            .with_remediation("start a KDE Plasma Wayland session with the required service"),
    )
}

fn session_error(message: impl Into<String>, source: impl Into<String>) -> DiagnosableError {
    DiagnosableError::new(ErrorPhase::CapabilityProbe, message)
        .with_source(source)
        .with_remediation("start Signal Auras from a KDE Plasma Wayland session")
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "available"))
}
