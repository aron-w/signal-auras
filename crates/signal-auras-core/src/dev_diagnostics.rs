use crate::{DetectorDefinition, DiagnosableError, HotkeyId, Roi, StateTrackerDefinitionSet};

pub const DEV_MODE_TOGGLE_HOTKEY: &str = "Ctrl+Alt+]";
pub const POINTER_DIAGNOSTIC_HOTKEY: &str = "Num1";
pub const TRACKER_GHOST_HOTKEY: &str = "Num2";

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

    pub fn tracker_ghost_enabled(&self) -> bool {
        self.enabled
    }

    pub fn toggle_hotkey() -> Result<HotkeyId, DiagnosableError> {
        HotkeyId::parse(DEV_MODE_TOGGLE_HOTKEY)
    }

    pub fn pointer_diagnostic_hotkey() -> Result<HotkeyId, DiagnosableError> {
        HotkeyId::parse(POINTER_DIAGNOSTIC_HOTKEY)
    }

    pub fn tracker_ghost_hotkey() -> Result<HotkeyId, DiagnosableError> {
        HotkeyId::parse(TRACKER_GHOST_HOTKEY)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeveloperDiagnosticShortcut {
    ToggleDevMode,
    PointerUnderMouse,
    TrackerGhostAuras,
}

impl DeveloperDiagnosticShortcut {
    pub fn from_hotkey(hotkey: &HotkeyId) -> Result<Option<Self>, DiagnosableError> {
        if hotkey == &DeveloperDiagnosticState::toggle_hotkey()? {
            return Ok(Some(Self::ToggleDevMode));
        }
        if hotkey == &DeveloperDiagnosticState::pointer_diagnostic_hotkey()? {
            return Ok(Some(Self::PointerUnderMouse));
        }
        if hotkey == &DeveloperDiagnosticState::tracker_ghost_hotkey()? {
            return Ok(Some(Self::TrackerGhostAuras));
        }
        Ok(None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackerGhostAuraKind {
    Circle { mark_center: bool },
    Rect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackerGhostAura {
    pub tracker_id: String,
    pub detector_kind: String,
    pub roi: Roi,
    pub kind: TrackerGhostAuraKind,
}

pub fn tracker_ghost_auras(trackers: &StateTrackerDefinitionSet) -> Vec<TrackerGhostAura> {
    trackers
        .trackers()
        .iter()
        .map(|tracker| {
            let (roi, kind) = match &tracker.detector {
                DetectorDefinition::RadialCooldown { roi, .. } => (
                    roi.clone(),
                    TrackerGhostAuraKind::Circle { mark_center: true },
                ),
                DetectorDefinition::HorizontalProgressBar { roi, .. } => {
                    (roi.clone(), TrackerGhostAuraKind::Rect)
                }
            };
            TrackerGhostAura {
                tracker_id: tracker.id.clone(),
                detector_kind: tracker.detector.kind().to_string(),
                roi,
                kind,
            }
        })
        .collect()
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
    use crate::{
        CapabilityKind, CapabilitySet, ProgressFillDirection, ScopeSelection,
        StateTrackerDefinition,
    };

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
        assert_eq!(
            DeveloperDiagnosticState::tracker_ghost_hotkey()
                .unwrap()
                .as_str(),
            "Num2"
        );
    }

    #[test]
    fn tracker_ghost_auras_describe_detector_rois() {
        let trackers = StateTrackerDefinitionSet::new([
            tracker(
                "refutation",
                DetectorDefinition::RadialCooldown {
                    roi: Roi::new(1540, 1560, 82, 82).unwrap(),
                    mask: None,
                },
            ),
            tracker(
                "heavy_stun",
                DetectorDefinition::HorizontalProgressBar {
                    roi: Roi::new(312, 1255, 300, 5).unwrap(),
                    fill_direction: ProgressFillDirection::LeftToRight,
                },
            ),
        ])
        .unwrap();

        let ghosts = tracker_ghost_auras(&trackers);

        assert_eq!(ghosts.len(), 2);
        assert_eq!(ghosts[0].tracker_id, "refutation");
        assert_eq!(ghosts[0].detector_kind, "radial_cooldown");
        assert_eq!(
            ghosts[0].kind,
            TrackerGhostAuraKind::Circle { mark_center: true }
        );
        assert_eq!(ghosts[0].roi, Roi::new(1540, 1560, 82, 82).unwrap());
        assert_eq!(ghosts[1].tracker_id, "heavy_stun");
        assert_eq!(ghosts[1].detector_kind, "horizontal_progress_bar");
        assert_eq!(ghosts[1].kind, TrackerGhostAuraKind::Rect);
        assert_eq!(ghosts[1].roi, Roi::new(312, 1255, 300, 5).unwrap());
    }

    fn tracker(id: &str, detector: DetectorDefinition) -> StateTrackerDefinition {
        StateTrackerDefinition::new(
            id,
            ScopeSelection::ExplicitGlobal,
            CapabilitySet::new([CapabilityKind::ScreenRead]),
            50,
            detector,
        )
        .unwrap()
    }
}
