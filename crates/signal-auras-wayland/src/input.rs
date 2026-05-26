use signal_auras_core::{InputEmission, SynthesizedInputRequest};

pub fn cancelled(_request: &SynthesizedInputRequest) -> InputEmission {
    InputEmission::Cancelled
}
