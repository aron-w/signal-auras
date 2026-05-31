use crate::{DiagnosableError, ErrorPhase, KeyToken, MacroDefinition, MouseButton, WheelDirection};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

pub const DEFAULT_MOTION_DURATION: Duration = Duration::from_millis(500);

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

    pub fn can_be_held(&self) -> bool {
        !matches!(self, Self::Wheel(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldCondition(Vec<MotionToken>);

impl HeldCondition {
    pub fn new(tokens: Vec<MotionToken>) -> Result<Self, DiagnosableError> {
        for token in &tokens {
            if !token.can_be_held() {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!("requires_held token '{}' cannot be held", token.describe()),
                ));
            }
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

    pub fn is_satisfied_by(&self, held: &BTreeSet<MotionToken>) -> bool {
        self.0.iter().all(|token| held.contains(token))
    }

    pub fn contains(&self, token: &MotionToken) -> bool {
        self.0.contains(token)
    }

    pub fn contains_leader(&self) -> bool {
        self.0
            .iter()
            .any(|token| matches!(token, MotionToken::Leader))
    }

    pub fn requires_pointer_observation(&self) -> bool {
        self.0.iter().any(MotionToken::requires_pointer_observation)
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
pub struct LoopInterval {
    pub every_ms: u64,
}

impl LoopInterval {
    pub fn new(every_ms: u64) -> Result<Self, DiagnosableError> {
        if every_ms == 0 {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "loop repeat every_ms must be a positive integer",
            ));
        }
        Ok(Self { every_ms })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopRepeat {
    pub interval: LoopInterval,
    pub macro_definition: MacroDefinition,
}

impl LoopRepeat {
    pub fn new(interval: LoopInterval, macro_definition: MacroDefinition) -> Self {
        Self {
            interval,
            macro_definition,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopBody {
    Once(MacroDefinition),
    Repeat(LoopRepeat),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopDefinition {
    pub while_held: MotionTrigger,
    pub before: Option<MacroDefinition>,
    pub body: LoopBody,
    pub after: Option<MacroDefinition>,
}

impl LoopDefinition {
    pub fn new(
        while_held: MotionTrigger,
        before: Option<MacroDefinition>,
        body: LoopBody,
        after: Option<MacroDefinition>,
    ) -> Self {
        Self {
            while_held,
            before,
            body,
            after,
        }
    }

    pub fn repeat(&self) -> Option<&LoopRepeat> {
        match &self.body {
            LoopBody::Once(_) => None,
            LoopBody::Repeat(repeat) => Some(repeat),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionDefinition {
    pub requires_held: HeldCondition,
    pub trigger: MotionTrigger,
    pub mode: crate::BindingMode,
    pub macro_definition: Option<MacroDefinition>,
    pub loop_definition: Option<LoopDefinition>,
    pub within_ms: u64,
    pub inter_action_delay_ms: u64,
}

impl MotionDefinition {
    pub fn new(
        trigger: MotionTrigger,
        mode: crate::BindingMode,
        macro_definition: Option<MacroDefinition>,
        loop_definition: Option<LoopDefinition>,
        within_ms: u64,
        inter_action_delay_ms: u64,
    ) -> Result<Self, DiagnosableError> {
        Self::with_requires_held(
            HeldCondition::new(Vec::new())?,
            trigger,
            mode,
            macro_definition,
            loop_definition,
            within_ms,
            inter_action_delay_ms,
        )
    }

    pub fn with_requires_held(
        requires_held: HeldCondition,
        trigger: MotionTrigger,
        mode: crate::BindingMode,
        macro_definition: Option<MacroDefinition>,
        loop_definition: Option<LoopDefinition>,
        within_ms: u64,
        inter_action_delay_ms: u64,
    ) -> Result<Self, DiagnosableError> {
        if within_ms == 0 {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "motion within_ms must be a positive integer",
            ));
        }
        if macro_definition.is_none() && loop_definition.is_none() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "motion must define macro or loop",
            ));
        }
        Ok(Self {
            requires_held,
            trigger,
            mode,
            macro_definition,
            loop_definition,
            within_ms,
            inter_action_delay_ms,
        })
    }

    pub fn requires_pointer_observation(&self) -> bool {
        self.requires_held.requires_pointer_observation()
            || self.trigger.requires_pointer_observation()
            || self
                .loop_definition
                .as_ref()
                .is_some_and(|loop_definition| {
                    loop_definition.while_held.requires_pointer_observation()
                })
    }

    fn requires_held_satisfied(&self, held: &BTreeSet<MotionToken>) -> bool {
        self.requires_held.is_satisfied_by(held)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressDefinition {
    pub requires_held: HeldCondition,
    pub trigger: MotionToken,
    pub mode: crate::BindingMode,
    pub macro_definition: MacroDefinition,
    pub inter_action_delay_ms: u64,
}

impl PressDefinition {
    pub fn new(
        requires_held: HeldCondition,
        trigger: MotionToken,
        mode: crate::BindingMode,
        macro_definition: MacroDefinition,
        inter_action_delay_ms: u64,
    ) -> Self {
        Self {
            requires_held,
            trigger,
            mode,
            macro_definition,
            inter_action_delay_ms,
        }
    }

    pub fn guard_is_satisfied_by(&self, held: &BTreeSet<MotionToken>) -> bool {
        self.requires_held.is_satisfied_by(held)
    }

    pub fn requires_pointer_observation(&self) -> bool {
        self.requires_held.requires_pointer_observation()
            || self.trigger.requires_pointer_observation()
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
        starts_loop: bool,
    },
    LoopCancelled {
        trigger: MotionTrigger,
    },
    MotionDiscarded {
        reason: MotionDiscardReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionDiscardReason {
    Timeout,
    UnrelatedPress,
    PreconditionReleased,
}

#[derive(Debug, Clone)]
pub struct MotionRuntime {
    motions: BTreeMap<MotionTrigger, MotionDefinition>,
    active_attempt: Option<MotionAttempt>,
    held: BTreeSet<MotionToken>,
    active_loops: BTreeSet<MotionTrigger>,
}

#[derive(Debug, Clone)]
struct MotionAttempt {
    started_at: Duration,
    progress: usize,
    candidates: BTreeSet<MotionTrigger>,
}

impl MotionRuntime {
    pub fn new(motions: impl IntoIterator<Item = MotionDefinition>) -> Self {
        let motions = motions
            .into_iter()
            .map(|motion| (motion.trigger.clone(), motion))
            .collect::<BTreeMap<_, _>>();
        Self {
            motions,
            active_attempt: None,
            held: BTreeSet::new(),
            active_loops: BTreeSet::new(),
        }
    }

    pub fn motion(&self, trigger: &MotionTrigger) -> Option<&MotionDefinition> {
        self.motions.get(trigger)
    }

    pub fn handle_input(&mut self, event: MotionInputEvent) -> Vec<MotionRuntimeEvent> {
        self.handle_input_at(event, Duration::ZERO)
    }

    pub fn handle_input_at(
        &mut self,
        event: MotionInputEvent,
        event_time: Duration,
    ) -> Vec<MotionRuntimeEvent> {
        match event.state {
            MotionInputState::Pressed => self.handle_press(event.token, event_time),
            MotionInputState::Released => self.handle_release(event.token),
        }
    }

    pub fn loop_is_active(&self, trigger: &MotionTrigger) -> bool {
        self.active_loops.contains(trigger)
    }

    pub fn cancel_active_loops(&mut self) -> Vec<MotionTrigger> {
        let cancelled = self.active_loops.iter().cloned().collect::<Vec<_>>();
        self.active_loops.clear();
        cancelled
    }

    fn handle_press(
        &mut self,
        token: MotionToken,
        event_time: Duration,
    ) -> Vec<MotionRuntimeEvent> {
        self.held.insert(token.clone());
        let mut events = Vec::new();

        if self.active_attempt_expired(event_time) {
            self.active_attempt = None;
            events.push(MotionRuntimeEvent::MotionDiscarded {
                reason: MotionDiscardReason::Timeout,
            });
        }

        if let Some(attempt) = &mut self.active_attempt {
            let next_progress = attempt.progress + 1;
            let elapsed = event_time.saturating_sub(attempt.started_at);
            let candidates = attempt
                .candidates
                .iter()
                .filter(|trigger| trigger.tokens().get(attempt.progress) == Some(&token))
                .filter(|trigger| {
                    self.motions.get(*trigger).is_some_and(|motion| {
                        elapsed <= Duration::from_millis(motion.within_ms)
                            && motion.requires_held_satisfied(&self.held)
                    })
                })
                .cloned()
                .collect::<BTreeSet<_>>();
            if candidates.is_empty() {
                self.active_attempt = None;
                events.push(MotionRuntimeEvent::MotionDiscarded {
                    reason: MotionDiscardReason::UnrelatedPress,
                });
            } else {
                attempt.progress = next_progress;
                attempt.candidates = candidates;
                let completed = attempt
                    .candidates
                    .iter()
                    .find(|trigger| trigger.tokens().len() == attempt.progress)
                    .cloned();
                if let Some(trigger) = completed {
                    self.active_attempt = None;
                    if let Some(event) = self.trigger_event(trigger) {
                        events.push(event);
                    }
                }
                return events;
            }
        }

        let candidates = self
            .motions
            .iter()
            .filter(|(trigger, motion)| {
                trigger.tokens().first() == Some(&token)
                    && motion.requires_held_satisfied(&self.held)
            })
            .map(|(trigger, _)| trigger)
            .cloned()
            .collect::<BTreeSet<_>>();
        if candidates.is_empty() {
            return events;
        }
        if let Some(trigger) = candidates
            .iter()
            .find(|trigger| trigger.tokens().len() == 1)
            .cloned()
        {
            if let Some(event) = self.trigger_event(trigger) {
                events.push(event);
            }
            return events;
        }
        self.active_attempt = Some(MotionAttempt {
            started_at: event_time,
            progress: 1,
            candidates,
        });
        events
    }

    fn trigger_event(&mut self, trigger: MotionTrigger) -> Option<MotionRuntimeEvent> {
        let motion = self.motions.get(&trigger)?;
        let starts_loop = motion
            .loop_definition
            .as_ref()
            .is_some_and(|loop_definition| self.while_held_satisfied(&loop_definition.while_held));
        if starts_loop {
            self.active_loops.insert(trigger.clone());
        }
        Some(MotionRuntimeEvent::Triggered {
            trigger,
            starts_loop,
        })
    }

    fn handle_release(&mut self, token: MotionToken) -> Vec<MotionRuntimeEvent> {
        self.held.remove(&token);
        let mut events = Vec::new();
        if self.active_attempt.as_ref().is_some_and(|attempt| {
            attempt.candidates.iter().any(|trigger| {
                self.motions
                    .get(trigger)
                    .is_some_and(|motion| motion.requires_held.contains(&token))
            })
        }) {
            self.active_attempt = None;
            events.push(MotionRuntimeEvent::MotionDiscarded {
                reason: MotionDiscardReason::PreconditionReleased,
            });
        }
        let cancelled = self
            .active_loops
            .iter()
            .filter(|trigger| {
                self.motions
                    .get(*trigger)
                    .and_then(|motion| motion.loop_definition.as_ref())
                    .is_some_and(|loop_definition| {
                        !self.while_held_satisfied(&loop_definition.while_held)
                    })
                    || self
                        .motions
                        .get(*trigger)
                        .is_some_and(|motion| !motion.requires_held_satisfied(&self.held))
            })
            .cloned()
            .collect::<Vec<_>>();
        for trigger in &cancelled {
            self.active_loops.remove(trigger);
        }
        events.extend(
            cancelled
                .into_iter()
                .map(|trigger| MotionRuntimeEvent::LoopCancelled { trigger }),
        );
        events
    }

    fn active_attempt_expired(&self, event_time: Duration) -> bool {
        let Some(attempt) = &self.active_attempt else {
            return false;
        };
        let elapsed = event_time.saturating_sub(attempt.started_at);
        attempt.candidates.iter().all(|trigger| {
            self.motions
                .get(trigger)
                .is_none_or(|motion| elapsed > Duration::from_millis(motion.within_ms))
        })
    }

    fn while_held_satisfied(&self, trigger: &MotionTrigger) -> bool {
        trigger
            .tokens()
            .iter()
            .all(|token| self.held.contains(token))
    }

    pub fn held_satisfies(&self, condition: &HeldCondition) -> bool {
        condition.is_satisfied_by(&self.held)
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
            DEFAULT_MOTION_DURATION.as_millis() as u64,
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
                starts_loop: false,
            }]
        );
    }

    #[test]
    fn motion_sequence_must_complete_within_duration_window() {
        let trigger = MotionTrigger::parse(["<Leader>", "f", "f"]).unwrap();
        let motion = MotionDefinition::new(
            trigger.clone(),
            BindingMode::Consume,
            Some(macro_def()),
            None,
            DEFAULT_MOTION_DURATION.as_millis() as u64,
            0,
        )
        .unwrap();
        let mut runtime = MotionRuntime::new([motion.clone()]);

        assert!(runtime
            .handle_input_at(
                MotionInputEvent::pressed(MotionToken::Leader),
                Duration::from_millis(100),
            )
            .is_empty());
        assert!(runtime
            .handle_input_at(
                MotionInputEvent::pressed(MotionToken::Key("f".into())),
                Duration::from_millis(400),
            )
            .is_empty());
        let events = runtime.handle_input_at(
            MotionInputEvent::pressed(MotionToken::Key("f".into())),
            Duration::from_millis(600),
        );

        assert_eq!(
            events,
            vec![MotionRuntimeEvent::Triggered {
                trigger: trigger.clone(),
                starts_loop: false,
            }]
        );

        let mut runtime = MotionRuntime::new([motion]);
        runtime.handle_input_at(
            MotionInputEvent::pressed(MotionToken::Leader),
            Duration::from_millis(100),
        );
        runtime.handle_input_at(
            MotionInputEvent::pressed(MotionToken::Key("f".into())),
            Duration::from_millis(400),
        );
        let events = runtime.handle_input_at(
            MotionInputEvent::pressed(MotionToken::Key("f".into())),
            Duration::from_millis(601),
        );

        assert_eq!(
            events,
            vec![MotionRuntimeEvent::MotionDiscarded {
                reason: MotionDiscardReason::Timeout,
            }]
        );
    }

    #[test]
    fn unrelated_press_discards_pending_motion_and_can_start_new_one() {
        let trigger = MotionTrigger::parse(["<Leader>", "f"]).unwrap();
        let motion = MotionDefinition::new(
            trigger.clone(),
            BindingMode::Consume,
            Some(macro_def()),
            None,
            DEFAULT_MOTION_DURATION.as_millis() as u64,
            0,
        )
        .unwrap();
        let mut runtime = MotionRuntime::new([motion]);

        runtime.handle_input_at(
            MotionInputEvent::pressed(MotionToken::Leader),
            Duration::from_millis(0),
        );
        let discarded = runtime.handle_input_at(
            MotionInputEvent::pressed(MotionToken::Key("x".into())),
            Duration::from_millis(10),
        );
        assert_eq!(
            discarded,
            vec![MotionRuntimeEvent::MotionDiscarded {
                reason: MotionDiscardReason::UnrelatedPress,
            }]
        );
        assert!(runtime
            .handle_input_at(
                MotionInputEvent::pressed(MotionToken::Leader),
                Duration::from_millis(20),
            )
            .is_empty());
        let triggered = runtime.handle_input_at(
            MotionInputEvent::pressed(MotionToken::Key("f".into())),
            Duration::from_millis(30),
        );

        assert_eq!(
            triggered,
            vec![MotionRuntimeEvent::Triggered {
                trigger,
                starts_loop: false,
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
            Some(LoopDefinition::new(
                MotionTrigger::parse(["<Leader>", "<LClick>"]).unwrap(),
                None,
                LoopBody::Repeat(LoopRepeat::new(LoopInterval::new(50).unwrap(), macro_def())),
                None,
            )),
            DEFAULT_MOTION_DURATION.as_millis() as u64,
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
                starts_loop: true,
            }]
        );
        assert!(runtime.loop_is_active(&trigger));

        let cancelled = runtime.handle_input(MotionInputEvent::released(MotionToken::Leader));

        assert_eq!(
            cancelled,
            vec![MotionRuntimeEvent::LoopCancelled {
                trigger: trigger.clone(),
            }]
        );
        assert!(!runtime.loop_is_active(&trigger));
    }

    #[test]
    fn repeat_cancellation_is_idempotent() {
        let trigger = MotionTrigger::parse(["<Leader>", "<LClick>", "<LClick>"]).unwrap();
        let motion = MotionDefinition::new(
            trigger.clone(),
            BindingMode::Passthrough,
            None,
            Some(LoopDefinition::new(
                MotionTrigger::parse(["<Leader>", "<LClick>"]).unwrap(),
                None,
                LoopBody::Repeat(LoopRepeat::new(LoopInterval::new(50).unwrap(), macro_def())),
                None,
            )),
            DEFAULT_MOTION_DURATION.as_millis() as u64,
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

        assert!(runtime.loop_is_active(&trigger));
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
        assert!(!runtime.loop_is_active(&trigger));
    }

    #[test]
    fn guarded_motion_requires_hold_before_first_press() {
        let trigger = MotionTrigger::parse(["<LClick>", "<LClick>"]).unwrap();
        let motion = MotionDefinition::with_requires_held(
            HeldCondition::parse(["<Leader>"]).unwrap(),
            trigger,
            BindingMode::Consume,
            Some(macro_def()),
            None,
            DEFAULT_MOTION_DURATION.as_millis() as u64,
            0,
        )
        .unwrap();
        let mut runtime = MotionRuntime::new([motion]);

        assert!(runtime
            .handle_input(MotionInputEvent::pressed(MotionToken::MouseButton(
                MouseButton::Left,
            )))
            .is_empty());
        assert!(runtime
            .handle_input(MotionInputEvent::pressed(MotionToken::Leader))
            .is_empty());
        assert!(runtime
            .handle_input(MotionInputEvent::pressed(MotionToken::MouseButton(
                MouseButton::Left,
            )))
            .is_empty());
    }

    #[test]
    fn releasing_guard_discards_active_motion_attempt() {
        let trigger = MotionTrigger::parse(["<LClick>", "<LClick>"]).unwrap();
        let motion = MotionDefinition::with_requires_held(
            HeldCondition::parse(["<Leader>"]).unwrap(),
            trigger,
            BindingMode::Consume,
            Some(macro_def()),
            None,
            DEFAULT_MOTION_DURATION.as_millis() as u64,
            0,
        )
        .unwrap();
        let mut runtime = MotionRuntime::new([motion]);

        runtime.handle_input(MotionInputEvent::pressed(MotionToken::Leader));
        runtime.handle_input(MotionInputEvent::pressed(MotionToken::MouseButton(
            MouseButton::Left,
        )));
        let discarded = runtime.handle_input(MotionInputEvent::released(MotionToken::Leader));
        runtime.handle_input(MotionInputEvent::pressed(MotionToken::Leader));
        let second_click = runtime.handle_input(MotionInputEvent::pressed(
            MotionToken::MouseButton(MouseButton::Left),
        ));

        assert_eq!(
            discarded,
            vec![MotionRuntimeEvent::MotionDiscarded {
                reason: MotionDiscardReason::PreconditionReleased,
            }]
        );
        assert!(second_click.is_empty());
    }

    #[test]
    fn guard_release_cancels_active_loop() {
        let trigger = MotionTrigger::parse(["<LClick>", "<LClick>"]).unwrap();
        let motion = MotionDefinition::with_requires_held(
            HeldCondition::parse(["<Leader>"]).unwrap(),
            trigger.clone(),
            BindingMode::Passthrough,
            None,
            Some(LoopDefinition::new(
                MotionTrigger::parse(["<LClick>"]).unwrap(),
                None,
                LoopBody::Repeat(LoopRepeat::new(LoopInterval::new(50).unwrap(), macro_def())),
                Some(macro_def()),
            )),
            DEFAULT_MOTION_DURATION.as_millis() as u64,
            0,
        )
        .unwrap();
        let mut runtime = MotionRuntime::new([motion]);

        runtime.handle_input(MotionInputEvent::pressed(MotionToken::Leader));
        runtime.handle_input(MotionInputEvent::pressed(MotionToken::MouseButton(
            MouseButton::Left,
        )));
        let triggered = runtime.handle_input(MotionInputEvent::pressed(MotionToken::MouseButton(
            MouseButton::Left,
        )));
        let cancelled = runtime.handle_input(MotionInputEvent::released(MotionToken::Leader));

        assert_eq!(
            triggered,
            vec![MotionRuntimeEvent::Triggered {
                trigger: trigger.clone(),
                starts_loop: true,
            }]
        );
        assert_eq!(
            cancelled,
            vec![MotionRuntimeEvent::LoopCancelled { trigger }]
        );
    }
}
