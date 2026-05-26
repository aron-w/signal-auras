use signal_auras_core::{
    ActiveProcessContext, ActiveProcessProvider, Capability, CapabilityKind, CapabilityReport,
    CapabilitySet, CleanupReport, DiagnosableError, ErrorPhase, HotkeyBinding, HotkeyRegistrar,
    InputEmission, MacroAction, MacroExecutor, ProcessName, RegistrationId,
    SynthesizedInputRequest,
};
use std::collections::BTreeSet;

use crate::capability::{kde_capability_report, KdeEnvironment};

#[derive(Debug)]
pub struct KdePlasmaAdapter {
    environment: KdeEnvironment,
    // Current-run bridge state only. This must never represent persistent KWin
    // scripts, autostart entries, or global registrations across process runs.
    bridge: crate::kde_bridge::KdeBridgeState,
    registrations: Vec<RegistrationId>,
    rejected_hotkeys: BTreeSet<String>,
    active_process: Option<ActiveProcessContext>,
    // Portal sessions are opened lazily for approved input and closed on
    // cancellation/shutdown so input capability cannot outlive the runner.
    portal_session: Option<crate::portal::PortalInputSession>,
}

impl KdePlasmaAdapter {
    pub fn new() -> Self {
        Self::from_environment(KdeEnvironment::from_process_env())
    }

    pub fn from_environment(environment: KdeEnvironment) -> Self {
        Self {
            environment,
            bridge: crate::kde_bridge::KdeBridgeState::default(),
            registrations: Vec::new(),
            rejected_hotkeys: BTreeSet::new(),
            active_process: None,
            portal_session: None,
        }
    }

    pub fn reject_hotkey_for_test(&mut self, hotkey: impl Into<String>) {
        self.rejected_hotkeys.insert(hotkey.into());
    }

    pub fn with_active_process_context(mut self, context: ActiveProcessContext) -> Self {
        self.active_process = Some(context);
        self
    }

    pub fn probe_capabilities(&self, required: &CapabilitySet) -> CapabilityReport {
        kde_capability_report(required, &self.environment)
    }

    pub fn cleanup_report(&self) -> CleanupReport {
        let bridge = self.bridge.cleanup_report();
        CleanupReport::all_succeeded(self.registrations.len() + bridge.attempted)
    }
}

impl Default for KdePlasmaAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ActiveProcessProvider for KdePlasmaAdapter {
    fn active_process_name(&self) -> Result<Option<ProcessName>, DiagnosableError> {
        Ok(self
            .active_process
            .as_ref()
            .and_then(|context| context.matchable_name().cloned()))
    }

    fn active_process_context(&self) -> Result<ActiveProcessContext, DiagnosableError> {
        Ok(self.active_process.clone().unwrap_or_else(|| {
            ActiveProcessContext::unavailable(
                "KDE active-process metadata provider is not connected yet",
            )
        }))
    }
}

impl HotkeyRegistrar for KdePlasmaAdapter {
    fn register(&mut self, binding: HotkeyBinding) -> Result<RegistrationId, DiagnosableError> {
        let required = CapabilitySet::for_bindings([&binding]);
        if let Some(error) = self
            .probe_capabilities(&required)
            .first_blocking_error(&required)
        {
            return Err(error);
        }
        let Some(hotkey) = binding.keyboard_hotkey() else {
            return Err(DiagnosableError::new(
                ErrorPhase::Registration,
                "KDE composite pointer registration provider is unsupported",
            )
            .with_capability(Capability::CompositePointerObservation));
        };
        if self.rejected_hotkeys.contains(hotkey.as_str()) {
            return Err(crate::diagnostics::reserved_shortcut(hotkey.as_str()));
        }
        let id = RegistrationId::new(format!("kde-{}", hotkey.as_str()));
        self.registrations.push(id.clone());
        Ok(id)
    }

    fn unregister_all(&mut self) -> Result<(), DiagnosableError> {
        self.registrations.clear();
        Ok(())
    }
}

impl MacroExecutor for KdePlasmaAdapter {
    fn execute_action(&mut self, _action: &MacroAction) -> Result<(), DiagnosableError> {
        Err(DiagnosableError::new(
            ErrorPhase::MacroExecution,
            "KDE synthesized input provider is not connected yet",
        )
        .with_capability(Capability::SynthesizedInput))
    }

    fn execute_input_request(
        &mut self,
        request: SynthesizedInputRequest,
    ) -> Result<InputEmission, DiagnosableError> {
        let required = CapabilitySet::new([CapabilityKind::SynthesizedInput]);
        if let Some(error) = self
            .probe_capabilities(&required)
            .first_blocking_error(&required)
        {
            return Err(error);
        }
        if self.portal_session.is_none() {
            self.portal_session = Some(crate::portal::PortalInputSession::open());
        }
        self.portal_session.as_ref().unwrap().synthesize(request)
    }

    fn cancel_pending(&mut self) -> Result<(), DiagnosableError> {
        if let Some(session) = &mut self.portal_session {
            let _ = session.close();
        }
        self.portal_session = None;
        Ok(())
    }
}
