use crate::{AdapterDiagnostic, DiagnosableError, ErrorPhase, MouseButton};
use std::collections::BTreeSet;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroAction {
    KeyPress { key: String },
    TextInput { text: String },
    MouseClick { button: MouseButton },
    Delay { duration_ms: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynthesizedInputState {
    Pending,
    Emitted,
    Denied,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynthesizedInputRequest {
    pub action: MacroAction,
    pub sequence: usize,
    pub state: SynthesizedInputState,
    pub diagnostic: Option<AdapterDiagnostic>,
}

impl SynthesizedInputRequest {
    pub fn new(action: MacroAction, sequence: usize) -> Self {
        Self {
            action,
            sequence,
            state: SynthesizedInputState::Pending,
            diagnostic: None,
        }
    }

    pub fn denied(mut self, diagnostic: AdapterDiagnostic) -> Self {
        self.state = SynthesizedInputState::Denied;
        self.diagnostic = Some(diagnostic);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEmission {
    Emitted,
    Denied,
    Failed,
    Cancelled,
}

impl MacroAction {
    pub fn key(key: impl Into<String>) -> Result<Self, DiagnosableError> {
        let key = key.into();
        if key.trim().is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "key action requires a non-empty key",
            ));
        }
        Ok(Self::KeyPress { key })
    }

    pub fn text(text: impl Into<String>) -> Result<Self, DiagnosableError> {
        let text = text.into();
        if text.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "text action requires non-empty text",
            ));
        }
        Ok(Self::TextInput { text })
    }

    pub fn mouse_click(button: MouseButton) -> Self {
        Self::MouseClick { button }
    }

    pub fn delay(duration_ms: u64) -> Result<Self, DiagnosableError> {
        if duration_ms == 0 {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "delay must be 1 ms or greater",
            ));
        }
        Ok(Self::Delay { duration_ms })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroDefinition {
    actions: Vec<MacroAction>,
}

impl MacroDefinition {
    pub fn new(actions: Vec<MacroAction>) -> Result<Self, DiagnosableError> {
        if actions.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "macro must contain at least one action",
            ));
        }
        Ok(Self { actions })
    }

    pub fn actions(&self) -> &[MacroAction] {
        &self.actions
    }
}

#[derive(Default)]
pub struct MacroScheduler {
    running: BTreeSet<String>,
}

impl MacroScheduler {
    pub fn begin(&mut self, hotkey: &str) -> Result<MacroRunGuard, DiagnosableError> {
        if !self.running.insert(hotkey.to_string()) {
            return Err(DiagnosableError::new(
                ErrorPhase::Trigger,
                format!("macro for '{hotkey}' is already running"),
            ));
        }
        Ok(MacroRunGuard {
            hotkey: hotkey.to_string(),
        })
    }

    pub fn finish(&mut self, guard: MacroRunGuard) {
        self.running.remove(&guard.hotkey);
    }
}

pub struct MacroRunGuard {
    hotkey: String,
}

pub fn execute_plan<F>(
    definition: &MacroDefinition,
    mut execute_action: F,
) -> Result<(), DiagnosableError>
where
    F: FnMut(&MacroAction) -> Result<(), DiagnosableError>,
{
    execute_plan_with_inter_action_delay(definition, 0, &mut execute_action)
}

pub fn execute_plan_with_inter_action_delay<F>(
    definition: &MacroDefinition,
    inter_action_delay_ms: u64,
    mut execute_action: F,
) -> Result<(), DiagnosableError>
where
    F: FnMut(&MacroAction) -> Result<(), DiagnosableError>,
{
    let mut generated_actions = 0usize;
    for action in definition.actions() {
        match action {
            MacroAction::Delay { duration_ms } => {
                thread::sleep(Duration::from_millis(*duration_ms))
            }
            _ => {
                if generated_actions > 0 && inter_action_delay_ms > 0 {
                    thread::sleep(Duration::from_millis(inter_action_delay_ms));
                }
                execute_action(action)?;
                generated_actions += 1;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_action_order() {
        let macro_def = MacroDefinition::new(vec![
            MacroAction::key("Enter").unwrap(),
            MacroAction::text("/hideout").unwrap(),
        ])
        .unwrap();

        assert!(matches!(
            macro_def.actions()[0],
            MacroAction::KeyPress { .. }
        ));
        assert!(matches!(
            macro_def.actions()[1],
            MacroAction::TextInput { .. }
        ));
    }

    #[test]
    fn denies_repeated_running_hotkey() {
        let mut scheduler = MacroScheduler::default();
        let _guard = scheduler.begin("F5").unwrap();
        assert!(scheduler.begin("F5").is_err());
    }
}
