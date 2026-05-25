use signal_auras_core::{Capability, DiagnosableError};

use crate::diagnostics::unsupported_protocol;

pub fn probe_global_shortcuts() -> Result<(), DiagnosableError> {
    Err(unsupported_protocol(Capability::GlobalShortcut))
}

pub fn probe_synthesized_input() -> Result<(), DiagnosableError> {
    Err(unsupported_protocol(Capability::SynthesizedInput))
}
