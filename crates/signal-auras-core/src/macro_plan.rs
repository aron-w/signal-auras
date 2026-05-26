use crate::{AdapterDiagnostic, DiagnosableError, ErrorPhase, MouseButton};
use std::collections::BTreeSet;
use std::thread;
use std::time::{Duration, Instant};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MacroRunId(u64);

impl MacroRunId {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroRunPoll {
    Ready(SynthesizedInputRequest),
    Pending(Duration),
    Complete,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct MacroRunState {
    id: MacroRunId,
    actions: Vec<MacroAction>,
    next_action: usize,
    generated_actions: usize,
    inter_action_delay_ms: u64,
    inter_action_delay_armed: bool,
    ready_at: Instant,
    cancelled: bool,
}

impl MacroRunState {
    pub fn new(
        id: MacroRunId,
        definition: &MacroDefinition,
        inter_action_delay_ms: u64,
        ready_at: Instant,
    ) -> Self {
        Self {
            id,
            actions: definition.actions().to_vec(),
            next_action: 0,
            generated_actions: 0,
            inter_action_delay_ms,
            inter_action_delay_armed: true,
            ready_at,
            cancelled: false,
        }
    }

    pub fn id(&self) -> MacroRunId {
        self.id
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        if self.cancelled || self.next_action >= self.actions.len() {
            None
        } else {
            Some(self.ready_at)
        }
    }

    pub fn poll(&mut self, now: Instant) -> MacroRunPoll {
        if self.cancelled {
            return MacroRunPoll::Cancelled;
        }
        if self.next_action >= self.actions.len() {
            return MacroRunPoll::Complete;
        }
        if now < self.ready_at {
            return MacroRunPoll::Pending(self.ready_at.duration_since(now));
        }
        let Some(action) = self.actions.get(self.next_action).cloned() else {
            return MacroRunPoll::Complete;
        };
        self.next_action += 1;
        match action {
            MacroAction::Delay { duration_ms } => {
                self.ready_at = now + Duration::from_millis(duration_ms);
                MacroRunPoll::Pending(Duration::from_millis(duration_ms))
            }
            action => {
                if self.generated_actions > 0
                    && self.inter_action_delay_ms > 0
                    && self.inter_action_delay_armed
                {
                    self.next_action -= 1;
                    let delay = Duration::from_millis(self.inter_action_delay_ms);
                    self.ready_at = now + delay;
                    self.inter_action_delay_armed = false;
                    MacroRunPoll::Pending(delay)
                } else {
                    self.generated_actions += 1;
                    self.inter_action_delay_armed = true;
                    self.ready_at = now;
                    MacroRunPoll::Ready(SynthesizedInputRequest::new(
                        action,
                        self.generated_actions,
                    ))
                }
            }
        }
    }
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

    #[test]
    fn incremental_macro_run_waits_without_blocking() {
        let definition = MacroDefinition::new(vec![
            MacroAction::key("Enter").unwrap(),
            MacroAction::delay(25).unwrap(),
            MacroAction::text("x").unwrap(),
        ])
        .unwrap();
        let now = Instant::now();
        let mut run = MacroRunState::new(MacroRunId::new(1), &definition, 0, now);

        assert!(matches!(run.poll(now), MacroRunPoll::Ready(_)));
        assert!(matches!(
            run.poll(now),
            MacroRunPoll::Pending(duration) if duration == Duration::from_millis(25)
        ));
        assert!(matches!(
            run.poll(now + Duration::from_millis(25)),
            MacroRunPoll::Ready(_)
        ));
        assert_eq!(
            run.poll(now + Duration::from_millis(25)),
            MacroRunPoll::Complete
        );
    }

    #[test]
    fn incremental_macro_run_can_be_cancelled_before_output() {
        let definition = MacroDefinition::new(vec![MacroAction::text("x").unwrap()]).unwrap();
        let now = Instant::now();
        let mut run = MacroRunState::new(MacroRunId::new(7), &definition, 0, now);

        run.cancel();

        assert_eq!(run.poll(now), MacroRunPoll::Cancelled);
    }
}
