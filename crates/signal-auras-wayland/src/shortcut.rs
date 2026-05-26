use signal_auras_core::{HotkeyId, RegistrationId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutEvent {
    pub hotkey: HotkeyId,
    pub registration_id: RegistrationId,
}
