use signal_auras_core::{
    ActiveProcessContext, ActiveProcessProvider, Capability, CapabilityReport, CapabilitySet,
    CleanupReport, DiagnosableError, ErrorPhase, HotkeyBinding, HotkeyRegistrar, InputEmission,
    MacroAction, MacroExecutor, ProcessName, RegistrationId, SynthesizedInputRequest,
};

use crate::capability::environment_probe;
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
        let id = RegistrationId::new(format!("mock-{}", binding.hotkey.as_str()));
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
}

impl RealWaylandAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    // This is the side-effect boundary for live Wayland session probing. It
    // intentionally fails closed until a compositor-specific provider is wired
    // behind the adapter contracts.
    pub fn probe_capabilities(&self, required: &CapabilitySet) -> CapabilityReport {
        environment_probe(required)
    }

    pub fn cleanup_report(&self) -> CleanupReport {
        CleanupReport::all_succeeded(self.registrations.len())
    }
}

impl ActiveProcessProvider for RealWaylandAdapter {
    fn active_process_name(&self) -> Result<Option<ProcessName>, DiagnosableError> {
        Ok(None)
    }

    fn active_process_context(&self) -> Result<ActiveProcessContext, DiagnosableError> {
        Ok(ActiveProcessContext::unavailable(
            "active process metadata provider is unsupported",
        ))
    }
}

impl HotkeyRegistrar for RealWaylandAdapter {
    fn register(&mut self, _binding: HotkeyBinding) -> Result<RegistrationId, DiagnosableError> {
        Err(unsupported_protocol(Capability::GlobalShortcut))
    }

    fn unregister_all(&mut self) -> Result<(), DiagnosableError> {
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
        _request: SynthesizedInputRequest,
    ) -> Result<InputEmission, DiagnosableError> {
        Err(DiagnosableError::new(
            ErrorPhase::MacroExecution,
            "synthesized input provider is unsupported",
        )
        .with_capability(Capability::SynthesizedInput))
    }

    fn cancel_pending(&mut self) -> Result<(), DiagnosableError> {
        Ok(())
    }
}
