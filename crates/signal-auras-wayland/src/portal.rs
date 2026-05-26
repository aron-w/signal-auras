use signal_auras_core::{
    ActiveProcessContext, Capability, CapabilityKind, CapabilityReport, CapabilitySet,
    CleanupReport, DiagnosableError, InputEmission, SynthesizedInputRequest,
};

use crate::{capability::environment_probe, diagnostics::unsupported_protocol};

pub fn probe_required_capabilities(required: &CapabilitySet) -> CapabilityReport {
    environment_probe(required)
}

pub fn probe_global_shortcuts() -> Result<(), DiagnosableError> {
    Err(unsupported_protocol(Capability::GlobalShortcut))
}

pub fn probe_synthesized_input() -> Result<(), DiagnosableError> {
    Err(unsupported_protocol(Capability::SynthesizedInput))
}

pub fn synthesized_input_capability_set() -> CapabilitySet {
    CapabilitySet::new([CapabilityKind::SynthesizedInput])
}

pub fn active_process_context() -> Result<ActiveProcessContext, DiagnosableError> {
    Ok(ActiveProcessContext::unavailable(
        "active process metadata provider is unsupported",
    ))
}

pub fn synthesize_input(
    request: SynthesizedInputRequest,
) -> Result<InputEmission, DiagnosableError> {
    crate::input::validate_request_for_portal(&request)?;
    Ok(InputEmission::Emitted)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalInputSession {
    active: bool,
}

impl PortalInputSession {
    pub fn open() -> Self {
        Self { active: true }
    }

    pub fn synthesize(
        &self,
        request: SynthesizedInputRequest,
    ) -> Result<InputEmission, DiagnosableError> {
        if !self.active {
            return Ok(InputEmission::Cancelled);
        }
        synthesize_input(request)
    }

    pub fn close(&mut self) -> CleanupReport {
        if self.active {
            self.active = false;
            CleanupReport::all_succeeded(1)
        } else {
            CleanupReport::empty()
        }
    }
}
