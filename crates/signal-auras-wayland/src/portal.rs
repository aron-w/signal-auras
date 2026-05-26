use signal_auras_core::{
    ActiveProcessContext, Capability, CapabilityReport, CapabilitySet, DiagnosableError,
    InputEmission, SynthesizedInputRequest,
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

pub fn active_process_context() -> Result<ActiveProcessContext, DiagnosableError> {
    Ok(ActiveProcessContext::unavailable(
        "active process metadata provider is unsupported",
    ))
}

pub fn synthesize_input(
    _request: SynthesizedInputRequest,
) -> Result<InputEmission, DiagnosableError> {
    Err(unsupported_protocol(Capability::SynthesizedInput))
}
