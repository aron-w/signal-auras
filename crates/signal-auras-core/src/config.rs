use crate::{
    AutomationDefaults, BindingTrigger, DiagnosableError, ErrorPhase, HotkeyId, MacroDefinition,
    MotionDefinition, MotionToken, PressDefinition, ScopeSelection, ScriptScope,
};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuaAutomationConfiguration {
    pub scope: Option<ScriptScope>,
    pub leader: Option<MotionToken>,
    pub defaults: AutomationDefaults,
    pub input_provider: Option<InputProviderConfig>,
    bindings: BTreeMap<BindingTrigger, BindingDefinition>,
    motions: BTreeMap<crate::MotionTrigger, MotionDefinition>,
    presses: BTreeMap<MotionToken, PressDefinition>,
}

impl LuaAutomationConfiguration {
    pub fn new(
        scope: Option<ScriptScope>,
        hotkeys: Vec<(HotkeyId, MacroDefinition)>,
    ) -> Result<Self, DiagnosableError> {
        if hotkeys.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "hotkeys must contain at least one binding",
            ));
        }
        Self::with_bindings(
            scope,
            hotkeys
                .into_iter()
                .map(|(hotkey, macro_definition)| {
                    BindingDefinition::new(
                        BindingTrigger::keyboard(hotkey),
                        BindingMode::Consume,
                        macro_definition,
                    )
                })
                .collect(),
        )
    }

    pub fn with_bindings(
        scope: Option<ScriptScope>,
        bindings: Vec<BindingDefinition>,
    ) -> Result<Self, DiagnosableError> {
        Self::with_bindings_and_motions(
            scope,
            None,
            AutomationDefaults::default(),
            None,
            bindings,
            Vec::new(),
            Vec::new(),
        )
    }

    pub fn with_bindings_and_motions(
        scope: Option<ScriptScope>,
        leader: Option<MotionToken>,
        defaults: AutomationDefaults,
        input_provider: Option<InputProviderConfig>,
        bindings: Vec<BindingDefinition>,
        motions: Vec<MotionDefinition>,
        presses: Vec<PressDefinition>,
    ) -> Result<Self, DiagnosableError> {
        if bindings.is_empty() && motions.is_empty() && presses.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "configuration must contain at least one binding, motion, or press",
            ));
        }

        let mut normalized = BTreeMap::new();
        for binding in bindings {
            if normalized
                .insert(binding.trigger.clone(), binding)
                .is_some()
            {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    "duplicate binding trigger after normalization",
                ));
            }
        }
        let mut normalized_motions = BTreeMap::new();
        for motion in motions {
            if normalized_motions
                .insert(motion.trigger.clone(), motion)
                .is_some()
            {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    "duplicate motion trigger after normalization",
                ));
            }
        }
        reject_prefix_overlapping_motion_triggers(normalized_motions.keys())?;
        let mut normalized_presses = BTreeMap::new();
        for press in presses {
            if normalized_presses
                .insert(press.trigger.clone(), press)
                .is_some()
            {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    "duplicate press trigger after normalization",
                ));
            }
        }

        Ok(Self {
            scope,
            leader,
            defaults,
            input_provider,
            bindings: normalized,
            motions: normalized_motions,
            presses: normalized_presses,
        })
    }

    pub fn hotkeys(&self) -> BTreeMap<HotkeyId, MacroDefinition> {
        self.bindings
            .iter()
            .filter_map(|(trigger, binding)| match trigger {
                BindingTrigger::Keyboard(hotkey) => {
                    Some((hotkey.clone(), binding.macro_definition.clone()))
                }
                BindingTrigger::Composite(_) => None,
            })
            .collect()
    }

    pub fn bindings(&self) -> &BTreeMap<BindingTrigger, BindingDefinition> {
        &self.bindings
    }

    pub fn motions(&self) -> &BTreeMap<crate::MotionTrigger, MotionDefinition> {
        &self.motions
    }

    pub fn presses(&self) -> &BTreeMap<MotionToken, PressDefinition> {
        &self.presses
    }

    pub fn motions_for_scope(&self, scope: ScopeSelection) -> Vec<RuntimeMotion> {
        self.motions
            .values()
            .map(|motion| RuntimeMotion {
                definition: motion.clone(),
                scope: scope.clone(),
            })
            .collect()
    }

    pub fn presses_for_scope(&self, scope: ScopeSelection) -> Vec<RuntimePress> {
        self.presses
            .values()
            .map(|press| RuntimePress {
                definition: press.clone(),
                scope: scope.clone(),
            })
            .collect()
    }

    pub fn bindings_for_scope(&self, scope: ScopeSelection) -> Vec<HotkeyBinding> {
        self.bindings
            .values()
            .map(|binding| HotkeyBinding {
                trigger: binding.trigger.clone(),
                scope: scope.clone(),
                mode: binding.mode,
                macro_definition: binding.macro_definition.clone(),
                registration_state: RegistrationState::Pending,
            })
            .collect()
    }
}

fn reject_prefix_overlapping_motion_triggers<'a>(
    triggers: impl IntoIterator<Item = &'a crate::MotionTrigger>,
) -> Result<(), DiagnosableError> {
    let triggers = triggers.into_iter().collect::<Vec<_>>();
    for (index, left) in triggers.iter().enumerate() {
        for right in triggers.iter().skip(index + 1) {
            if trigger_is_prefix(left, right) || trigger_is_prefix(right, left) {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!(
                        "prefix-overlapping motion triggers are ambiguous: '{}' and '{}'",
                        left.describe(),
                        right.describe()
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn trigger_is_prefix(left: &crate::MotionTrigger, right: &crate::MotionTrigger) -> bool {
    left.tokens().len() < right.tokens().len() && right.tokens().starts_with(left.tokens())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputProviderConfig {
    pub backend: InputProviderBackend,
    pub mode: InputProviderMode,
    pub output: InputProviderOutput,
    pub devices: Vec<PathBuf>,
    pub all_devices: bool,
}

impl InputProviderConfig {
    pub fn evdev(
        devices: Vec<PathBuf>,
        mode: InputProviderMode,
        output: InputProviderOutput,
    ) -> Result<Self, DiagnosableError> {
        if devices.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "evdev input provider requires at least one device",
            ));
        }
        Ok(Self {
            backend: InputProviderBackend::Evdev,
            mode,
            output,
            devices,
            all_devices: false,
        })
    }

    pub fn evdev_all(
        mode: InputProviderMode,
        output: InputProviderOutput,
        acknowledge_risk: Option<&str>,
    ) -> Result<Self, DiagnosableError> {
        if mode == InputProviderMode::Grab && acknowledge_risk != Some("GRAB_ALL_INPUTS") {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "evdev grab with devices = \"all\" requires acknowledge_risk = \"GRAB_ALL_INPUTS\"",
            ));
        }
        Ok(Self {
            backend: InputProviderBackend::Evdev,
            mode,
            output,
            devices: Vec::new(),
            all_devices: true,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputProviderBackend {
    Evdev,
}

impl InputProviderBackend {
    pub fn parse(value: &str) -> Result<Self, DiagnosableError> {
        match value.trim() {
            "evdev" => Ok(Self::Evdev),
            value => Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("unsupported input provider backend '{value}'"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputProviderMode {
    Observe,
    Grab,
}

impl InputProviderMode {
    pub fn parse(value: Option<&str>) -> Result<Self, DiagnosableError> {
        match value.unwrap_or("observe").trim() {
            "observe" => Ok(Self::Observe),
            "grab" | "consume" => Ok(Self::Grab),
            value => Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("unsupported input provider mode '{value}'"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputProviderOutput {
    Portal,
    Uinput,
}

impl InputProviderOutput {
    pub fn parse(value: Option<&str>) -> Result<Self, DiagnosableError> {
        match value.unwrap_or("portal").trim() {
            "portal" => Ok(Self::Portal),
            "uinput" => Ok(Self::Uinput),
            value => Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("unsupported input provider output '{value}'"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationState {
    Pending,
    Registered,
    Failed,
    Unregistered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingMode {
    Consume,
    Passthrough,
}

impl BindingMode {
    pub fn parse(value: Option<&str>) -> Result<Self, DiagnosableError> {
        match value.unwrap_or("consume").trim() {
            "consume" => Ok(Self::Consume),
            "passthrough" => Ok(Self::Passthrough),
            value => Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("unsupported binding mode '{value}'"),
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Consume => "consume",
            Self::Passthrough => "passthrough",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingDefinition {
    pub trigger: BindingTrigger,
    pub mode: BindingMode,
    pub macro_definition: MacroDefinition,
}

impl BindingDefinition {
    pub fn new(
        trigger: BindingTrigger,
        mode: BindingMode,
        macro_definition: MacroDefinition,
    ) -> Self {
        Self {
            trigger,
            mode,
            macro_definition,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyBinding {
    pub trigger: BindingTrigger,
    pub scope: ScopeSelection,
    pub mode: BindingMode,
    pub macro_definition: MacroDefinition,
    pub registration_state: RegistrationState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMotion {
    pub definition: MotionDefinition,
    pub scope: ScopeSelection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePress {
    pub definition: PressDefinition,
    pub scope: ScopeSelection,
}

impl HotkeyBinding {
    pub fn trigger_label(&self) -> String {
        self.trigger.describe()
    }

    pub fn keyboard_hotkey(&self) -> Option<&HotkeyId> {
        match &self.trigger {
            BindingTrigger::Keyboard(hotkey) => Some(hotkey),
            BindingTrigger::Composite(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CompositeTrigger, MacroAction, ModifierSet, MotionDefinition, MotionTrigger, MouseTrigger,
        WheelDirection,
    };

    fn macro_def() -> MacroDefinition {
        MacroDefinition::new(vec![MacroAction::text("x").unwrap()]).unwrap()
    }

    #[test]
    fn rejects_empty_hotkey_collection() {
        assert!(LuaAutomationConfiguration::new(None, vec![]).is_err());
    }

    #[test]
    fn rejects_duplicate_hotkeys() {
        let f5 = HotkeyId::parse("F5").unwrap();
        assert!(LuaAutomationConfiguration::new(
            None,
            vec![(f5.clone(), macro_def()), (f5, macro_def())],
        )
        .is_err());
    }

    #[test]
    fn rejects_duplicate_keyboard_aliases_after_normalization() {
        assert!(LuaAutomationConfiguration::new(
            None,
            vec![
                (HotkeyId::parse("Return").unwrap(), macro_def()),
                (HotkeyId::parse("Enter").unwrap(), macro_def()),
            ],
        )
        .is_err());
    }

    #[test]
    fn converts_hotkeys_into_unified_bindings() {
        let config = LuaAutomationConfiguration::new(
            None,
            vec![(HotkeyId::parse("F5").unwrap(), macro_def())],
        )
        .unwrap();

        let binding = config.bindings().values().next().unwrap();
        assert_eq!(binding.mode, BindingMode::Consume);
        assert!(binding.trigger.is_keyboard());
    }

    #[test]
    fn rejects_duplicate_normalized_composite_bindings() {
        let trigger = BindingTrigger::Composite(CompositeTrigger::new(
            ModifierSet::parse(["Shift", "Ctrl"]).unwrap(),
            MouseTrigger::Wheel(WheelDirection::Up),
        ));

        assert!(LuaAutomationConfiguration::with_bindings(
            None,
            vec![
                BindingDefinition::new(trigger.clone(), BindingMode::Consume, macro_def()),
                BindingDefinition::new(trigger, BindingMode::Passthrough, macro_def()),
            ],
        )
        .is_err());
    }

    #[test]
    fn rejects_duplicate_motion_triggers() {
        let trigger = MotionTrigger::parse(["<Leader>", "f", "f"]).unwrap();
        let first = MotionDefinition::new(
            trigger.clone(),
            BindingMode::Consume,
            Some(macro_def()),
            None,
            crate::motion::DEFAULT_MOTION_DURATION.as_millis() as u64,
            0,
        )
        .unwrap();
        let second = MotionDefinition::new(
            trigger,
            BindingMode::Passthrough,
            Some(macro_def()),
            None,
            crate::motion::DEFAULT_MOTION_DURATION.as_millis() as u64,
            0,
        )
        .unwrap();

        assert!(LuaAutomationConfiguration::with_bindings_and_motions(
            None,
            None,
            AutomationDefaults::default(),
            None,
            Vec::new(),
            vec![first, second],
            Vec::new(),
        )
        .is_err());
    }

    #[test]
    fn rejects_duplicate_motion_aliases_after_normalization() {
        let first_trigger = MotionTrigger::parse(["<Leader>", "Return"]).unwrap();
        let second_trigger = MotionTrigger::parse(["<Leader>", "Enter"]).unwrap();
        let first = MotionDefinition::new(
            first_trigger,
            BindingMode::Consume,
            Some(macro_def()),
            None,
            crate::motion::DEFAULT_MOTION_DURATION.as_millis() as u64,
            0,
        )
        .unwrap();
        let second = MotionDefinition::new(
            second_trigger,
            BindingMode::Passthrough,
            Some(macro_def()),
            None,
            crate::motion::DEFAULT_MOTION_DURATION.as_millis() as u64,
            0,
        )
        .unwrap();

        assert!(LuaAutomationConfiguration::with_bindings_and_motions(
            None,
            None,
            AutomationDefaults::default(),
            None,
            Vec::new(),
            vec![first, second],
            Vec::new(),
        )
        .is_err());
    }
}
