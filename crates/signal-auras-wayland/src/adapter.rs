use signal_auras_core::{
    ActiveProcessContext, ActiveProcessProvider, Capability, CapabilityKind, CapabilityReport,
    CapabilitySet, CleanupReport, DiagnosableError, ErrorPhase, HotkeyBinding, HotkeyRegistrar,
    InputEmission, MacroAction, MacroExecutor, ProcessName, RegistrationId,
    SynthesizedInputRequest,
};
use std::collections::BTreeSet;

use crate::capability::{environment_probe, KdeEnvironment};
use crate::diagnostics::unsupported_protocol;

#[derive(Default)]
pub struct MockableWaylandAdapter {
    registrations: Vec<RegistrationId>,
    active_process: Option<ProcessName>,
    capability_report: CapabilityReport,
    emitted_inputs: Vec<MacroAction>,
}

impl MockableWaylandAdapter {
    pub fn with_active_process(active_process: Option<ProcessName>) -> Self {
        Self {
            registrations: Vec::new(),
            active_process,
            capability_report: CapabilityReport::default(),
            emitted_inputs: Vec::new(),
        }
    }

    pub fn with_capability_report(mut self, capability_report: CapabilityReport) -> Self {
        self.capability_report = capability_report;
        self
    }

    pub fn probe_capabilities(&self, _required: &CapabilitySet) -> CapabilityReport {
        self.capability_report.clone()
    }

    pub fn emitted_inputs(&self) -> &[MacroAction] {
        &self.emitted_inputs
    }
}

impl ActiveProcessProvider for MockableWaylandAdapter {
    fn active_process_name(&self) -> Result<Option<ProcessName>, DiagnosableError> {
        Ok(self.active_process.clone())
    }

    fn active_process_context(&self) -> Result<ActiveProcessContext, DiagnosableError> {
        match self.active_process.clone() {
            Some(process) => Ok(ActiveProcessContext::name_only(process)),
            None => Ok(ActiveProcessContext::unavailable(
                "active process metadata is unavailable",
            )),
        }
    }
}

impl HotkeyRegistrar for MockableWaylandAdapter {
    fn register(&mut self, binding: HotkeyBinding) -> Result<RegistrationId, DiagnosableError> {
        let id = RegistrationId::new(format!("mock-{}", binding.trigger_label()));
        self.registrations.push(id.clone());
        Ok(id)
    }

    fn unregister_all(&mut self) -> Result<(), DiagnosableError> {
        self.registrations.clear();
        Ok(())
    }
}

impl MacroExecutor for MockableWaylandAdapter {
    fn execute_action(&mut self, action: &MacroAction) -> Result<(), DiagnosableError> {
        self.emitted_inputs.push(action.clone());
        Ok(())
    }

    fn execute_input_request(
        &mut self,
        request: SynthesizedInputRequest,
    ) -> Result<InputEmission, DiagnosableError> {
        self.execute_action(&request.action)?;
        Ok(InputEmission::Emitted)
    }

    fn cancel_pending(&mut self) -> Result<(), DiagnosableError> {
        Ok(())
    }
}

pub fn real_registration_unavailable() -> DiagnosableError {
    unsupported_protocol(Capability::GlobalShortcut)
}

#[derive(Default)]
pub struct RealWaylandAdapter {
    registrations: Vec<RegistrationId>,
    environment: Option<KdeEnvironment>,
    rejected_hotkeys: BTreeSet<String>,
    portal_session: Option<crate::portal::PortalInputSession>,
    shortcut_bridge: Option<crate::kde_bridge::KwinShortcutBridge>,
}

impl RealWaylandAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_environment(environment: KdeEnvironment) -> Self {
        Self {
            registrations: Vec::new(),
            environment: Some(environment),
            rejected_hotkeys: BTreeSet::new(),
            portal_session: None,
            shortcut_bridge: None,
        }
    }

    pub fn reject_hotkey_for_test(&mut self, hotkey: impl Into<String>) {
        self.rejected_hotkeys.insert(hotkey.into());
    }

    // This is the side-effect boundary for live Wayland session probing. It
    // intentionally fails closed until a compositor-specific provider is wired
    // behind the adapter contracts.
    pub fn probe_capabilities(&self, required: &CapabilitySet) -> CapabilityReport {
        match &self.environment {
            Some(environment) => crate::capability::kde_capability_report(required, environment),
            None => environment_probe(required),
        }
    }

    pub fn cleanup_report(&self) -> CleanupReport {
        CleanupReport::all_succeeded(self.registrations.len())
    }

    pub fn next_shortcut_event(&mut self) -> Option<signal_auras_core::HotkeyId> {
        self.shortcut_bridge
            .as_mut()
            .and_then(crate::kde_bridge::KwinShortcutBridge::next_shortcut_event)
    }
}

impl ActiveProcessProvider for RealWaylandAdapter {
    fn active_process_name(&self) -> Result<Option<ProcessName>, DiagnosableError> {
        Ok(None)
    }

    fn active_process_context(&self) -> Result<ActiveProcessContext, DiagnosableError> {
        if let Some(bridge) = &self.shortcut_bridge {
            return Ok(bridge.active_process_context());
        }
        Ok(ActiveProcessContext::unavailable(
            "active process metadata provider is unsupported",
        ))
    }
}

impl HotkeyRegistrar for RealWaylandAdapter {
    fn register(&mut self, binding: HotkeyBinding) -> Result<RegistrationId, DiagnosableError> {
        let required = CapabilitySet::for_bindings([&binding]);
        if let Some(error) = self
            .probe_capabilities(&required)
            .first_blocking_error(&required)
        {
            return Err(error);
        }
        if !binding.trigger.is_keyboard() {
            return Err(DiagnosableError::new(
                ErrorPhase::Registration,
                "composite pointer registration provider is unsupported",
            )
            .with_capability(Capability::CompositePointerObservation));
        }
        let signal_auras_core::BindingTrigger::Keyboard(hotkey) = &binding.trigger else {
            unreachable!("composite triggers returned above")
        };
        if self.rejected_hotkeys.contains(hotkey.as_str()) {
            return Err(crate::diagnostics::reserved_shortcut(hotkey.as_str()));
        }
        let id = if self.environment.is_some() {
            RegistrationId::new(format!("kde-{}", hotkey.as_str()))
        } else {
            if self.shortcut_bridge.is_none() {
                self.shortcut_bridge = Some(crate::kde_bridge::KwinShortcutBridge::connect()?);
            }
            RegistrationId::new(
                self.shortcut_bridge
                    .as_mut()
                    .expect("shortcut bridge was initialized")
                    .register_shortcut(&binding)?,
            )
        };
        self.registrations.push(id.clone());
        Ok(id)
    }

    fn unregister_all(&mut self) -> Result<(), DiagnosableError> {
        if let Some(bridge) = &mut self.shortcut_bridge {
            let _ = bridge.unload()?;
        }
        self.shortcut_bridge = None;
        self.registrations.clear();
        Ok(())
    }
}

impl MacroExecutor for RealWaylandAdapter {
    fn execute_action(&mut self, _action: &MacroAction) -> Result<(), DiagnosableError> {
        Err(DiagnosableError::new(
            ErrorPhase::MacroExecution,
            "synthesized input provider is unsupported",
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
            self.portal_session = Some(if self.environment.is_some() {
                crate::portal::PortalInputSession::open()
            } else {
                crate::portal::PortalInputSession::open_live()?
            });
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
