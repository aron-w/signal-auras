use signal_auras_core::{CleanupReport, DiagnosableError, ErrorPhase, HotkeyId};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum KdeBridgeLifecycle {
    #[default]
    NotInstalled,
    Installing,
    Active,
    Unloading,
    Unloaded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KdeBridgeState {
    lifecycle: KdeBridgeLifecycle,
    registered_handles: usize,
    events: VecDeque<HotkeyId>,
}

impl KdeBridgeState {
    pub fn active_for_test(registered_handles: usize) -> Self {
        Self {
            lifecycle: KdeBridgeLifecycle::Active,
            registered_handles,
            events: VecDeque::new(),
        }
    }

    pub fn lifecycle(&self) -> &KdeBridgeLifecycle {
        &self.lifecycle
    }

    pub fn unload(&mut self) -> Result<CleanupReport, signal_auras_core::DiagnosableError> {
        if matches!(self.lifecycle, KdeBridgeLifecycle::Unloaded) {
            return Ok(CleanupReport::empty());
        }
        let report = self.cleanup_report();
        self.registered_handles = 0;
        self.events.clear();
        self.lifecycle = KdeBridgeLifecycle::Unloaded;
        Ok(report)
    }

    pub fn push_shortcut_event(&mut self, hotkey: HotkeyId) -> Result<(), DiagnosableError> {
        if !matches!(self.lifecycle, KdeBridgeLifecycle::Active) {
            return Err(DiagnosableError::new(
                ErrorPhase::Trigger,
                "KDE bridge is not active",
            ));
        }
        self.events.push_back(hotkey);
        Ok(())
    }

    pub fn next_shortcut_event(&mut self) -> Result<Option<HotkeyId>, DiagnosableError> {
        if !matches!(
            self.lifecycle,
            KdeBridgeLifecycle::Active | KdeBridgeLifecycle::Unloaded
        ) {
            return Err(DiagnosableError::new(
                ErrorPhase::Trigger,
                "KDE bridge cannot deliver events in its current state",
            ));
        }
        Ok(self.events.pop_front())
    }

    pub fn cleanup_report(&self) -> CleanupReport {
        CleanupReport::all_succeeded(self.registered_handles)
    }
}
