use crate::{DiagnosableError, ErrorPhase, HotkeyId, MacroDefinition, ScopeSelection, ScriptScope};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuaAutomationConfiguration {
    pub scope: Option<ScriptScope>,
    hotkeys: BTreeMap<HotkeyId, MacroDefinition>,
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

        let mut normalized = BTreeMap::new();
        for (hotkey, macro_definition) in hotkeys {
            if normalized
                .insert(hotkey.clone(), macro_definition)
                .is_some()
            {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!("duplicate hotkey '{}'", hotkey.as_str()),
                ));
            }
        }

        Ok(Self {
            scope,
            hotkeys: normalized,
        })
    }

    pub fn hotkeys(&self) -> &BTreeMap<HotkeyId, MacroDefinition> {
        &self.hotkeys
    }

    pub fn bindings_for_scope(&self, scope: ScopeSelection) -> Vec<HotkeyBinding> {
        self.hotkeys
            .iter()
            .map(|(hotkey, macro_definition)| HotkeyBinding {
                hotkey: hotkey.clone(),
                scope: scope.clone(),
                macro_definition: macro_definition.clone(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyBinding {
    pub hotkey: HotkeyId,
    pub scope: ScopeSelection,
    pub macro_definition: MacroDefinition,
    pub registration_state: RegistrationState,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MacroAction;

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
}
