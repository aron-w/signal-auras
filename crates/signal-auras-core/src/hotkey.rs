use crate::{DiagnosableError, ErrorPhase};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HotkeyId(String);

impl HotkeyId {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, DiagnosableError> {
        let value = value.as_ref().trim();
        if value.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "hotkey identifier cannot be empty",
            ));
        }
        if is_supported_hotkey(value) {
            Ok(Self(value.to_string()))
        } else {
            Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("unsupported hotkey '{value}'"),
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_supported_hotkey(value: &str) -> bool {
    matches!(
        value,
        "F1" | "F2" | "F3" | "F4" | "F5" | "F6" | "F7" | "F8" | "F9" | "F10" | "F11" | "F12"
    ) || value
        .split('+')
        .all(|part| matches!(part, "Ctrl" | "Alt" | "Shift" | "Super") || part.len() == 1)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegistrationId(String);

impl RegistrationId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutRegistrationState {
    Pending,
    Registered,
    Rejected,
    Unregistering,
    Unregistered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutRegistrationHandle {
    id: RegistrationId,
}

impl ShortcutRegistrationHandle {
    pub fn new(id: RegistrationId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> &RegistrationId {
        &self.id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupReport {
    pub attempted: usize,
    pub succeeded: usize,
    pub failed: usize,
}

impl CleanupReport {
    pub fn empty() -> Self {
        Self {
            attempted: 0,
            succeeded: 0,
            failed: 0,
        }
    }

    pub fn all_succeeded(attempted: usize) -> Self {
        Self {
            attempted,
            succeeded: attempted,
            failed: 0,
        }
    }

    pub fn is_success(&self) -> bool {
        self.failed == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_supported_function_key() {
        assert_eq!(HotkeyId::parse("F5").unwrap().as_str(), "F5");
    }

    #[test]
    fn rejects_empty_hotkey() {
        assert!(HotkeyId::parse(" ").is_err());
    }
}
