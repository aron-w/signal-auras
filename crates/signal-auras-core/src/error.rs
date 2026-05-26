use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorPhase {
    ArgumentValidation,
    ScriptLoad,
    ScriptValidation,
    ScopePrompt,
    CapabilityProbe,
    Registration,
    Trigger,
    MacroExecution,
    Shutdown,
}

impl fmt::Display for ErrorPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::ArgumentValidation => "argument_validation",
            Self::ScriptLoad => "script_load",
            Self::ScriptValidation => "script_validation",
            Self::ScopePrompt => "scope_prompt",
            Self::CapabilityProbe => "capability_probe",
            Self::Registration => "registration",
            Self::Trigger => "trigger",
            Self::MacroExecution => "macro_execution",
            Self::Shutdown => "shutdown",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    GlobalShortcut,
    CompositePointerObservation,
    CompositePointerConsumption,
    ActiveProcess,
    SynthesizedInput,
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::GlobalShortcut => "global_shortcut",
            Self::CompositePointerObservation => "composite_pointer_observation",
            Self::CompositePointerConsumption => "composite_pointer_consumption",
            Self::ActiveProcess => "active_process",
            Self::SynthesizedInput => "synthesized_input",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CapabilityKind {
    GlobalShortcut,
    CompositePointerObservation,
    CompositePointerConsumption,
    ActiveProcessMetadata,
    SynthesizedInput,
}

impl CapabilityKind {
    pub fn legacy_capability(self) -> Capability {
        match self {
            Self::GlobalShortcut => Capability::GlobalShortcut,
            Self::CompositePointerObservation => Capability::CompositePointerObservation,
            Self::CompositePointerConsumption => Capability::CompositePointerConsumption,
            Self::ActiveProcessMetadata => Capability::ActiveProcess,
            Self::SynthesizedInput => Capability::SynthesizedInput,
        }
    }
}

impl fmt::Display for CapabilityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::GlobalShortcut => "global_shortcut",
            Self::CompositePointerObservation => "composite_pointer_observation",
            Self::CompositePointerConsumption => "composite_pointer_consumption",
            Self::ActiveProcessMetadata => "active_process_metadata",
            Self::SynthesizedInput => "synthesized_input",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityAvailability {
    Available,
    Unsupported,
    PermissionRequired,
    Denied,
    Revoked,
    Invalidated,
    ProviderError,
}

impl CapabilityAvailability {
    pub fn allows_activation(self) -> bool {
        matches!(self, Self::Available)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterDiagnostic {
    pub phase: ErrorPhase,
    pub capability: Option<CapabilityKind>,
    pub message: String,
    pub remediation: Option<String>,
    pub source: Option<String>,
}

impl AdapterDiagnostic {
    pub fn new(phase: ErrorPhase, message: impl Into<String>) -> Self {
        Self {
            phase,
            capability: None,
            message: message.into(),
            remediation: None,
            source: None,
        }
    }

    pub fn with_capability(mut self, capability: CapabilityKind) -> Self {
        self.capability = Some(capability);
        self
    }

    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityStatus {
    pub kind: CapabilityKind,
    pub availability: CapabilityAvailability,
    pub source: Option<String>,
    pub diagnostic: Option<AdapterDiagnostic>,
}

impl CapabilityStatus {
    pub fn available(kind: CapabilityKind, source: impl Into<String>) -> Self {
        Self {
            kind,
            availability: CapabilityAvailability::Available,
            source: Some(source.into()),
            diagnostic: None,
        }
    }

    pub fn unavailable(
        kind: CapabilityKind,
        availability: CapabilityAvailability,
        diagnostic: AdapterDiagnostic,
    ) -> Self {
        Self {
            kind,
            availability,
            source: diagnostic.source.clone(),
            diagnostic: Some(diagnostic),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapabilitySet {
    required: Vec<CapabilityKind>,
}

impl CapabilitySet {
    pub fn new(required: impl IntoIterator<Item = CapabilityKind>) -> Self {
        let mut required = required.into_iter().collect::<Vec<_>>();
        required.sort();
        required.dedup();
        Self { required }
    }

    pub fn for_bindings<'a>(
        bindings: impl IntoIterator<Item = &'a crate::config::HotkeyBinding>,
    ) -> Self {
        let mut required = Vec::new();
        for binding in bindings {
            match binding.trigger {
                crate::hotkey::BindingTrigger::Keyboard(_) => {
                    required.push(CapabilityKind::GlobalShortcut);
                }
                crate::hotkey::BindingTrigger::Composite(_) => {
                    required.push(CapabilityKind::CompositePointerObservation);
                    if binding.mode == crate::config::BindingMode::Consume {
                        required.push(CapabilityKind::CompositePointerConsumption);
                    }
                }
            }
            if matches!(
                binding.scope,
                crate::scope::ScopeSelection::ProcessList { .. }
            ) {
                required.push(CapabilityKind::ActiveProcessMetadata);
            }
            if binding.macro_definition.actions().iter().any(|action| {
                matches!(
                    action,
                    crate::macro_plan::MacroAction::KeyPress { .. }
                        | crate::macro_plan::MacroAction::TextInput { .. }
                )
            }) {
                required.push(CapabilityKind::SynthesizedInput);
            }
        }
        Self::new(required)
    }

    pub fn iter(&self) -> impl Iterator<Item = CapabilityKind> + '_ {
        self.required.iter().copied()
    }

    pub fn contains(&self, kind: CapabilityKind) -> bool {
        self.required.contains(&kind)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapabilityReport {
    statuses: BTreeMap<CapabilityKind, CapabilityStatus>,
}

impl CapabilityReport {
    pub fn from_statuses(statuses: impl IntoIterator<Item = CapabilityStatus>) -> Self {
        Self {
            statuses: statuses
                .into_iter()
                .map(|status| (status.kind, status))
                .collect(),
        }
    }

    pub fn status(&self, kind: CapabilityKind) -> Option<&CapabilityStatus> {
        self.statuses.get(&kind)
    }

    pub fn all_available(&self, required: &CapabilitySet) -> bool {
        required.iter().all(|kind| {
            self.status(kind)
                .is_some_and(|status| status.availability.allows_activation())
        })
    }

    pub fn first_blocking_error(&self, required: &CapabilitySet) -> Option<DiagnosableError> {
        for kind in required.iter() {
            let Some(status) = self.status(kind) else {
                return Some(
                    DiagnosableError::new(
                        ErrorPhase::CapabilityProbe,
                        format!("required capability '{kind}' was not probed"),
                    )
                    .with_capability(kind.legacy_capability()),
                );
            };
            if !status.availability.allows_activation() {
                return Some(
                    DiagnosableError::new(
                        ErrorPhase::CapabilityProbe,
                        status
                            .diagnostic
                            .as_ref()
                            .map(|diagnostic| diagnostic.message.clone())
                            .unwrap_or_else(|| {
                                format!("required capability '{kind}' is unavailable")
                            }),
                    )
                    .with_capability(kind.legacy_capability())
                    .with_optional_remediation(
                        status
                            .diagnostic
                            .as_ref()
                            .and_then(|diagnostic| diagnostic.remediation.clone()),
                    )
                    .with_optional_source(status.source.clone()),
                );
            }
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosableError {
    pub phase: ErrorPhase,
    pub capability: Option<Capability>,
    pub message: String,
    pub remediation: Option<String>,
    pub source: Option<String>,
}

impl DiagnosableError {
    pub fn new(phase: ErrorPhase, message: impl Into<String>) -> Self {
        Self {
            phase,
            capability: None,
            message: message.into(),
            remediation: None,
            source: None,
        }
    }

    pub fn with_capability(mut self, capability: Capability) -> Self {
        self.capability = Some(capability);
        self
    }

    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }

    pub fn with_optional_remediation(mut self, remediation: Option<String>) -> Self {
        self.remediation = remediation;
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_optional_source(mut self, source: Option<String>) -> Self {
        self.source = source;
        self
    }
}

impl fmt::Display for DiagnosableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.phase, self.message)?;
        if let Some(capability) = self.capability {
            write!(f, " (capability: {capability})")?;
        }
        if let Some(remediation) = &self.remediation {
            write!(f, " remediation: {remediation}")?;
        }
        if let Some(source) = &self.source {
            write!(f, " source: {source}")?;
        }
        Ok(())
    }
}

impl std::error::Error for DiagnosableError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_phase_capability_and_remediation() {
        let error = DiagnosableError::new(ErrorPhase::CapabilityProbe, "missing protocol")
            .with_capability(Capability::GlobalShortcut)
            .with_remediation("enable compositor global shortcut protocol");

        assert!(error.to_string().contains("capability_probe"));
        assert!(error.to_string().contains("global_shortcut"));
        assert!(error.to_string().contains("enable compositor"));
    }
}
