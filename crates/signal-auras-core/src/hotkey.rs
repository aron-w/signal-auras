use crate::{DiagnosableError, ErrorPhase, KeyToken};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Super,
}

impl Modifier {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, DiagnosableError> {
        match value.as_ref().trim() {
            "Ctrl" => Ok(Self::Ctrl),
            "Alt" => Ok(Self::Alt),
            "Shift" => Ok(Self::Shift),
            "Super" => Ok(Self::Super),
            value => Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("unsupported modifier '{value}'"),
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ctrl => "Ctrl",
            Self::Alt => "Alt",
            Self::Shift => "Shift",
            Self::Super => "Super",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ModifierSet(Vec<Modifier>);

impl ModifierSet {
    pub fn new(modifiers: impl IntoIterator<Item = Modifier>) -> Result<Self, DiagnosableError> {
        let mut normalized = Vec::new();
        for modifier in modifiers {
            if normalized.contains(&modifier) {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!("duplicate modifier '{}'", modifier.as_str()),
                ));
            }
            normalized.push(modifier);
        }
        normalized.sort();
        Ok(Self(normalized))
    }

    pub fn parse(
        modifiers: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, DiagnosableError> {
        Self::new(
            modifiers
                .into_iter()
                .map(Modifier::parse)
                .collect::<Result<Vec<_>, _>>()?,
        )
    }

    pub fn iter(&self) -> impl Iterator<Item = Modifier> + '_ {
        self.0.iter().copied()
    }

    pub fn describe(&self) -> String {
        self.iter()
            .map(Modifier::as_str)
            .collect::<Vec<_>>()
            .join("+")
    }
}

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
        let parts = value.split('+').map(str::trim).collect::<Vec<_>>();
        let mut modifiers = Vec::new();
        let mut key = None;
        for part in &parts {
            if part.is_empty() {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!("unsupported hotkey '{value}'"),
                ));
            }
            if parts.len() > 1 {
                if let Ok(modifier) = Modifier::parse(part) {
                    modifiers.push(modifier);
                    continue;
                }
            }
            if key.replace(KeyToken::parse(part)?).is_some() {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!("hotkey '{value}' must contain exactly one non-modifier key"),
                ));
            }
        }
        let Some(key) = key else {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("hotkey '{value}' must contain a key"),
            ));
        };
        let modifiers = ModifierSet::new(modifiers)?;
        let prefix = modifiers.describe();
        if prefix.is_empty() {
            Ok(Self(key.name().to_string()))
        } else {
            Ok(Self(format!("{prefix}+{}", key.name())))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, DiagnosableError> {
        match value.as_ref().trim() {
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            "middle" => Ok(Self::Middle),
            value => Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("unsupported mouse button '{value}'"),
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Middle => "middle",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WheelDirection {
    Up,
    Down,
}

impl WheelDirection {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, DiagnosableError> {
        match value.as_ref().trim() {
            "up" => Ok(Self::Up),
            "down" => Ok(Self::Down),
            value => Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("unsupported mouse wheel direction '{value}'"),
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Up => "wheel_up",
            Self::Down => "wheel_down",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MouseTrigger {
    Button(MouseButton),
    Wheel(WheelDirection),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompositeTrigger {
    modifiers: ModifierSet,
    primary: MouseTrigger,
}

impl CompositeTrigger {
    pub fn new(modifiers: ModifierSet, primary: MouseTrigger) -> Self {
        Self { modifiers, primary }
    }

    pub fn modifiers(&self) -> &ModifierSet {
        &self.modifiers
    }

    pub fn primary(&self) -> &MouseTrigger {
        &self.primary
    }

    pub fn describe(&self) -> String {
        let primary = match self.primary {
            MouseTrigger::Button(button) => format!("mouse_{}", button.as_str()),
            MouseTrigger::Wheel(direction) => direction.as_str().to_string(),
        };
        let modifiers = self.modifiers.describe();
        if modifiers.is_empty() {
            primary
        } else {
            format!("{modifiers}+{primary}")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BindingTrigger {
    Keyboard(HotkeyId),
    Composite(CompositeTrigger),
}

impl BindingTrigger {
    pub fn keyboard(hotkey: HotkeyId) -> Self {
        Self::Keyboard(hotkey)
    }

    pub fn is_keyboard(&self) -> bool {
        matches!(self, Self::Keyboard(_))
    }

    pub fn describe(&self) -> String {
        match self {
            Self::Keyboard(hotkey) => hotkey.as_str().to_string(),
            Self::Composite(trigger) => trigger.describe(),
        }
    }
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
        assert_eq!(HotkeyId::parse("F24").unwrap().as_str(), "F24");
        assert_eq!(HotkeyId::parse("Return").unwrap().as_str(), "Enter");
        assert_eq!(
            HotkeyId::parse("Ctrl+PageUp").unwrap().as_str(),
            "Ctrl+PageUp"
        );
    }

    #[test]
    fn rejects_empty_hotkey() {
        assert!(HotkeyId::parse(" ").is_err());
    }

    #[test]
    fn normalizes_modifier_order() {
        let modifiers = ModifierSet::parse(["Shift", "Ctrl", "Alt"]).unwrap();

        assert_eq!(modifiers.describe(), "Ctrl+Alt+Shift");
    }

    #[test]
    fn rejects_duplicate_and_unknown_modifiers() {
        assert!(ModifierSet::parse(["Ctrl", "Ctrl"]).is_err());
        assert!(ModifierSet::parse(["Meta"]).is_err());
    }

    #[test]
    fn validates_supported_mouse_triggers() {
        assert_eq!(MouseButton::parse("left").unwrap(), MouseButton::Left);
        assert_eq!(WheelDirection::parse("up").unwrap(), WheelDirection::Up);
        assert!(MouseButton::parse("back").is_err());
        assert!(WheelDirection::parse("sideways").is_err());
    }
}
