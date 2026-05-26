use signal_auras_core::{CapabilityKind, CapabilitySet, HotkeyId, RegistrationId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutEvent {
    pub hotkey: HotkeyId,
    pub registration_id: RegistrationId,
}

pub fn global_shortcut_capability_set() -> CapabilitySet {
    CapabilitySet::new([CapabilityKind::GlobalShortcut])
}
