use crate::{
    BindingTrigger, DiagnosableError, ErrorPhase, HotkeyId, MacroDefinition, ScopeSelection,
    ScriptScope,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuaAutomationConfiguration {
    pub scope: Option<ScriptScope>,
    bindings: BTreeMap<BindingTrigger, BindingDefinition>,
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
        if bindings.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "configuration must contain at least one binding",
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

        Ok(Self {
            scope,
            bindings: normalized,
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
    use crate::{CompositeTrigger, MacroAction, ModifierSet, MouseTrigger, WheelDirection};

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
}
