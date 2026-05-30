use crate::{DiagnosableError, ErrorPhase, KeyToken, MacroDefinition, MouseButton, WheelDirection};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationDefaults {
    pub inter_action_delay_ms: u64,
}

impl AutomationDefaults {
    pub fn new(inter_action_delay_ms: u64) -> Self {
        Self {
            inter_action_delay_ms,
        }
    }
}

impl Default for AutomationDefaults {
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MotionToken {
    Leader,
    Key(String),
    MouseButton(MouseButton),
    Wheel(WheelDirection),
}

impl MotionToken {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, DiagnosableError> {
        let value = value.as_ref().trim();
        if value.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "motion token cannot be empty",
            ));
        }
        match value {
            "<Leader>" => Ok(Self::Leader),
            "<LClick>" => Ok(Self::MouseButton(MouseButton::Left)),
            "<RClick>" => Ok(Self::MouseButton(MouseButton::Right)),
            "<MClick>" => Ok(Self::MouseButton(MouseButton::Middle)),
            "<WheelUp>" => Ok(Self::Wheel(WheelDirection::Up)),
            "<WheelDown>" => Ok(Self::Wheel(WheelDirection::Down)),
            value => KeyToken::parse(value).map(|key| Self::Key(key.name().to_string())),
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Self::Leader => "<Leader>".to_string(),
            Self::Key(key) => key.clone(),
            Self::MouseButton(MouseButton::Left) => "<LClick>".to_string(),
            Self::MouseButton(MouseButton::Right) => "<RClick>".to_string(),
            Self::MouseButton(MouseButton::Middle) => "<MClick>".to_string(),
            Self::Wheel(WheelDirection::Up) => "<WheelUp>".to_string(),
            Self::Wheel(WheelDirection::Down) => "<WheelDown>".to_string(),
        }
    }

    pub fn requires_pointer_observation(&self) -> bool {
        matches!(self, Self::MouseButton(_) | Self::Wheel(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MotionTrigger(Vec<MotionToken>);

impl MotionTrigger {
    pub fn new(tokens: Vec<MotionToken>) -> Result<Self, DiagnosableError> {
        if tokens.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "motion trigger must contain at least one token",
            ));
        }
        Ok(Self(tokens))
    }

    pub fn parse(
        tokens: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, DiagnosableError> {
        Self::new(
            tokens
                .into_iter()
                .map(MotionToken::parse)
                .collect::<Result<Vec<_>, _>>()?,
        )
    }

    pub fn tokens(&self) -> &[MotionToken] {
        &self.0
    }

    pub fn describe(&self) -> String {
        self.0
            .iter()
            .map(MotionToken::describe)
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn requires_pointer_observation(&self) -> bool {
        self.0.iter().any(MotionToken::requires_pointer_observation)
    }

    pub fn contains_leader(&self) -> bool {
        self.0
            .iter()
            .any(|token| matches!(token, MotionToken::Leader))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepeatInterval {
    pub min_ms: u64,
    pub max_ms: u64,
}

impl RepeatInterval {
    pub fn new(min_ms: u64, max_ms: u64) -> Result<Self, DiagnosableError> {
        if min_ms == 0 || max_ms == 0 || min_ms > max_ms {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "repeat interval must have positive min and max with min <= max",
            ));
        }
        Ok(Self { min_ms, max_ms })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepeatDefinition {
    pub while_held: MotionTrigger,
    pub interval: RepeatInterval,
    pub macro_definition: MacroDefinition,
}

impl RepeatDefinition {
    pub fn new(
        while_held: MotionTrigger,
        interval: RepeatInterval,
        macro_definition: MacroDefinition,
    ) -> Self {
        Self {
            while_held,
            interval,
            macro_definition,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionDefinition {
    pub trigger: MotionTrigger,
    pub mode: crate::BindingMode,
    pub macro_definition: Option<MacroDefinition>,
    pub repeat: Option<RepeatDefinition>,
    pub inter_action_delay_ms: u64,
}

impl MotionDefinition {
    pub fn new(
        trigger: MotionTrigger,
        mode: crate::BindingMode,
        macro_definition: Option<MacroDefinition>,
        repeat: Option<RepeatDefinition>,
        inter_action_delay_ms: u64,
    ) -> Result<Self, DiagnosableError> {
        if macro_definition.is_none() && repeat.is_none() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "motion must define macro or repeat",
            ));
        }
        Ok(Self {
            trigger,
            mode,
            macro_definition,
            repeat,
            inter_action_delay_ms,
        })
    }

    pub fn requires_pointer_observation(&self) -> bool {
        self.trigger.requires_pointer_observation()
            || self
                .repeat
                .as_ref()
                .is_some_and(|repeat| repeat.while_held.requires_pointer_observation())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionInputState {
    Pressed,
    Released,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionInputEvent {
    pub token: MotionToken,
    pub state: MotionInputState,
}

impl MotionInputEvent {
    pub fn pressed(token: MotionToken) -> Self {
        Self {
            token,
            state: MotionInputState::Pressed,
        }
    }

    pub fn released(token: MotionToken) -> Self {
        Self {
            token,
            state: MotionInputState::Released,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MotionRuntimeEvent {
    Triggered {
        trigger: MotionTrigger,
        starts_repeat: bool,
    },
    RepeatCancelled {
        trigger: MotionTrigger,
    },
}

#[derive(Debug, Clone)]
pub struct MotionRuntime {
    motions: BTreeMap<MotionTrigger, MotionDefinition>,
    progress: BTreeMap<MotionTrigger, usize>,
    held: BTreeSet<MotionToken>,
    active_repeats: BTreeSet<MotionTrigger>,
}

impl MotionRuntime {
    pub fn new(motions: impl IntoIterator<Item = MotionDefinition>) -> Self {
        let motions = motions
            .into_iter()
            .map(|motion| (motion.trigger.clone(), motion))
            .collect::<BTreeMap<_, _>>();
        let progress = motions
            .keys()
            .cloned()
            .map(|trigger| (trigger, 0))
            .collect();
        Self {
            motions,
            progress,
            held: BTreeSet::new(),
            active_repeats: BTreeSet::new(),
        }
    }

    pub fn motion(&self, trigger: &MotionTrigger) -> Option<&MotionDefinition> {
        self.motions.get(trigger)
    }

    pub fn handle_input(&mut self, event: MotionInputEvent) -> Vec<MotionRuntimeEvent> {
        match event.state {
            MotionInputState::Pressed => self.handle_press(event.token),
            MotionInputState::Released => self.handle_release(event.token),
        }
    }

    pub fn repeat_is_active(&self, trigger: &MotionTrigger) -> bool {
        self.active_repeats.contains(trigger)
    }

    fn handle_press(&mut self, token: MotionToken) -> Vec<MotionRuntimeEvent> {
        self.held.insert(token.clone());
        let mut events = Vec::new();
        for (trigger, motion) in &self.motions {
            let progress = self.progress.entry(trigger.clone()).or_default();
            let expected = &trigger.tokens()[*progress];
            if expected == &token {
                *progress += 1;
            } else if trigger.tokens().first() == Some(&token) {
                *progress = 1;
            } else if trigger.tokens().first() == Some(&MotionToken::Leader)
                && self.held.contains(&MotionToken::Leader)
                && trigger.tokens().get(1) == Some(&token)
            {
                *progress = 2;
            } else {
                *progress = 0;
            }
            if *progress == trigger.tokens().len() {
                *progress = 0;
                let starts_repeat = motion
                    .repeat
                    .as_ref()
                    .is_some_and(|repeat| self.while_held_satisfied(&repeat.while_held));
                if starts_repeat {
                    self.active_repeats.insert(trigger.clone());
                }
                events.push(MotionRuntimeEvent::Triggered {
                    trigger: trigger.clone(),
                    starts_repeat,
                });
            }
        }
        events
    }

    fn handle_release(&mut self, token: MotionToken) -> Vec<MotionRuntimeEvent> {
        self.held.remove(&token);
        let cancelled = self
            .active_repeats
            .iter()
            .filter(|trigger| {
                self.motions
                    .get(*trigger)
                    .and_then(|motion| motion.repeat.as_ref())
                    .is_some_and(|repeat| !self.while_held_satisfied(&repeat.while_held))
            })
            .cloned()
            .collect::<Vec<_>>();
        for trigger in &cancelled {
            self.active_repeats.remove(trigger);
        }
        cancelled
            .into_iter()
            .map(|trigger| MotionRuntimeEvent::RepeatCancelled { trigger })
            .collect()
    }

    fn while_held_satisfied(&self, trigger: &MotionTrigger) -> bool {
        trigger
            .tokens()
            .iter()
            .all(|token| self.held.contains(token))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BindingMode, MacroAction};

    fn macro_def() -> MacroDefinition {
        MacroDefinition::new(vec![MacroAction::text("x").unwrap()]).unwrap()
    }

    #[test]
    fn matches_uniform_keyboard_motion_sequence() {
        let trigger = MotionTrigger::parse(["<Leader>", "f", "f"]).unwrap();
        let motion = MotionDefinition::new(
            trigger.clone(),
            BindingMode::Consume,
            Some(macro_def()),
            None,
            0,
        )
        .unwrap();
        let mut runtime = MotionRuntime::new([motion]);

        assert!(runtime
            .handle_input(MotionInputEvent::pressed(MotionToken::Leader))
            .is_empty());
        assert!(runtime
            .handle_input(MotionInputEvent::pressed(MotionToken::Key("f".into())))
            .is_empty());
        let events = runtime.handle_input(MotionInputEvent::pressed(MotionToken::Key("f".into())));

        assert_eq!(
            events,
            vec![MotionRuntimeEvent::Triggered {
                trigger,
                starts_repeat: false,
            }]
        );
    }

    #[test]
    fn parses_wheel_motion_tokens() {
        assert_eq!(
            MotionToken::parse("<WheelUp>").unwrap(),
            MotionToken::Wheel(WheelDirection::Up)
        );
        assert_eq!(
            MotionToken::parse("<WheelDown>").unwrap().describe(),
            "<WheelDown>"
        );
    }

    #[test]
    fn parses_expanded_keyboard_motion_tokens() {
        assert_eq!(
            MotionToken::parse("PageDown").unwrap(),
            MotionToken::Key("PageDown".to_string())
        );
        assert_eq!(
            MotionToken::parse("Return").unwrap(),
            MotionToken::Key("Enter".to_string())
        );
        assert_eq!(
            MotionToken::parse("KPEnter").unwrap(),
            MotionToken::Key("KPEnter".to_string())
        );
        assert_eq!(
            MotionToken::parse("VolumeUp").unwrap(),
            MotionToken::Key("VolumeUp".to_string())
        );
    }

    #[test]
    fn held_final_mouse_press_can_start_and_cancel_repeat() {
        let trigger = MotionTrigger::parse(["<Leader>", "<LClick>", "<LClick>"]).unwrap();
        let motion = MotionDefinition::new(
            trigger.clone(),
            BindingMode::Passthrough,
            None,
            Some(RepeatDefinition::new(
                MotionTrigger::parse(["<Leader>", "<LClick>"]).unwrap(),
                RepeatInterval::new(50, 80).unwrap(),
                macro_def(),
            )),
            0,
        )
        .unwrap();
        let mut runtime = MotionRuntime::new([motion]);

        runtime.handle_input(MotionInputEvent::pressed(MotionToken::Leader));
        runtime.handle_input(MotionInputEvent::pressed(MotionToken::MouseButton(
            MouseButton::Left,
        )));
        runtime.handle_input(MotionInputEvent::released(MotionToken::MouseButton(
            MouseButton::Left,
        )));
        let events = runtime.handle_input(MotionInputEvent::pressed(MotionToken::MouseButton(
            MouseButton::Left,
        )));

        assert_eq!(
            events,
            vec![MotionRuntimeEvent::Triggered {
                trigger: trigger.clone(),
                starts_repeat: true,
            }]
        );
        assert!(runtime.repeat_is_active(&trigger));

        let cancelled = runtime.handle_input(MotionInputEvent::released(MotionToken::Leader));

        assert_eq!(
            cancelled,
            vec![MotionRuntimeEvent::RepeatCancelled {
                trigger: trigger.clone(),
            }]
        );
        assert!(!runtime.repeat_is_active(&trigger));
    }

    #[test]
    fn repeat_cancellation_is_idempotent() {
        let trigger = MotionTrigger::parse(["<Leader>", "<LClick>", "<LClick>"]).unwrap();
        let motion = MotionDefinition::new(
            trigger.clone(),
            BindingMode::Passthrough,
            None,
            Some(RepeatDefinition::new(
                MotionTrigger::parse(["<Leader>", "<LClick>"]).unwrap(),
                RepeatInterval::new(50, 80).unwrap(),
                macro_def(),
            )),
            0,
        )
        .unwrap();
        let mut runtime = MotionRuntime::new([motion]);

        runtime.handle_input(MotionInputEvent::pressed(MotionToken::Leader));
        runtime.handle_input(MotionInputEvent::pressed(MotionToken::MouseButton(
            MouseButton::Left,
        )));
        runtime.handle_input(MotionInputEvent::released(MotionToken::MouseButton(
            MouseButton::Left,
        )));
        runtime.handle_input(MotionInputEvent::pressed(MotionToken::MouseButton(
            MouseButton::Left,
        )));

        assert!(runtime.repeat_is_active(&trigger));
        assert_eq!(
            runtime
                .handle_input(MotionInputEvent::released(MotionToken::MouseButton(
                    MouseButton::Left,
                )))
                .len(),
            1
        );
        assert!(runtime
            .handle_input(MotionInputEvent::released(MotionToken::MouseButton(
                MouseButton::Left,
            )))
            .is_empty());
        assert!(!runtime.repeat_is_active(&trigger));
    }
}
