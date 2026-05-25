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
    ActiveProcess,
    SynthesizedInput,
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::GlobalShortcut => "global_shortcut",
            Self::ActiveProcess => "active_process",
            Self::SynthesizedInput => "synthesized_input",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosableError {
    pub phase: ErrorPhase,
    pub capability: Option<Capability>,
    pub message: String,
    pub remediation: Option<String>,
}

impl DiagnosableError {
    pub fn new(phase: ErrorPhase, message: impl Into<String>) -> Self {
        Self {
            phase,
            capability: None,
            message: message.into(),
            remediation: None,
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
