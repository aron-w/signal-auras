use signal_auras_core::{
    DiagnosableError, ErrorPhase, InputEmission, MacroAction, SynthesizedInputRequest,
};

pub fn cancelled(_request: &SynthesizedInputRequest) -> InputEmission {
    InputEmission::Cancelled
}

pub fn validate_text_for_key_translation(text: &str) -> Result<(), DiagnosableError> {
    if text
        .chars()
        .all(|character| character.is_ascii() && !character.is_control())
    {
        Ok(())
    } else {
        Err(DiagnosableError::new(
            ErrorPhase::MacroExecution,
            "text contains characters unsupported by the KDE portal key translation path",
        ))
    }
}

pub fn validate_request_for_portal(
    request: &SynthesizedInputRequest,
) -> Result<(), DiagnosableError> {
    match &request.action {
        MacroAction::TextInput { text } => validate_text_for_key_translation(text),
        MacroAction::KeyPress { key }
        | MacroAction::KeyDown { key }
        | MacroAction::KeyUp { key }
            if key.trim().is_empty() =>
        {
            Err(DiagnosableError::new(
                ErrorPhase::MacroExecution,
                "key input cannot be empty",
            ))
        }
        MacroAction::KeyPress { .. }
        | MacroAction::KeyDown { .. }
        | MacroAction::KeyUp { .. }
        | MacroAction::MouseClick { .. }
        | MacroAction::Delay { .. } => Ok(()),
    }
}
