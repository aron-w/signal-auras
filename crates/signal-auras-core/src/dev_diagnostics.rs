use crate::{DiagnosableError, HotkeyId};

pub const DEV_MODE_TOGGLE_HOTKEY: &str = "Ctrl+Alt+]";
pub const POINTER_DIAGNOSTIC_HOTKEY: &str = "Num1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeveloperDiagnosticState {
    enabled: bool,
}

impl DeveloperDiagnosticState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn toggle(&mut self) -> bool {
        self.enabled = !self.enabled;
        self.enabled
    }

    pub fn pointer_diagnostic_enabled(&self) -> bool {
        self.enabled
    }

    pub fn toggle_hotkey() -> Result<HotkeyId, DiagnosableError> {
        HotkeyId::parse(DEV_MODE_TOGGLE_HOTKEY)
    }

    pub fn pointer_diagnostic_hotkey() -> Result<HotkeyId, DiagnosableError> {
        HotkeyId::parse(POINTER_DIAGNOSTIC_HOTKEY)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeveloperDiagnosticShortcut {
    ToggleDevMode,
    PointerUnderMouse,
}

impl DeveloperDiagnosticShortcut {
    pub fn from_hotkey(hotkey: &HotkeyId) -> Result<Option<Self>, DiagnosableError> {
        if hotkey == &DeveloperDiagnosticState::toggle_hotkey()? {
            return Ok(Some(Self::ToggleDevMode));
        }
        if hotkey == &DeveloperDiagnosticState::pointer_diagnostic_hotkey()? {
            return Ok(Some(Self::PointerUnderMouse));
        }
        Ok(None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenPixelColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ScreenPixelColor {
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn hex_rgb(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn developer_diagnostic_state_gates_pointer_hotkey() {
        let mut state = DeveloperDiagnosticState::new();

        assert!(!state.pointer_diagnostic_enabled());
        assert!(state.toggle());
        assert!(state.pointer_diagnostic_enabled());
        assert!(!state.toggle());
        assert!(!state.pointer_diagnostic_enabled());
    }

    #[test]
    fn developer_diagnostic_hotkeys_are_canonicalized() {
        assert_eq!(
            DeveloperDiagnosticState::toggle_hotkey().unwrap().as_str(),
            "Ctrl+Alt+]"
        );
        assert_eq!(
            DeveloperDiagnosticState::pointer_diagnostic_hotkey()
                .unwrap()
                .as_str(),
            "Num1"
        );
    }
}
