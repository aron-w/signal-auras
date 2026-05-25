use signal_auras_core::{
    ActiveProcessProvider, Capability, DiagnosableError, ErrorPhase, HotkeyBinding,
    HotkeyRegistrar, MacroAction, MacroExecutor, ProcessName, RegistrationId,
};

use crate::diagnostics::unsupported_protocol;

#[derive(Default)]
pub struct MockableWaylandAdapter {
    registrations: Vec<RegistrationId>,
    active_process: Option<ProcessName>,
}

impl MockableWaylandAdapter {
    pub fn with_active_process(active_process: Option<ProcessName>) -> Self {
        Self {
            registrations: Vec::new(),
            active_process,
        }
    }
}

impl ActiveProcessProvider for MockableWaylandAdapter {
    fn active_process_name(&self) -> Result<Option<ProcessName>, DiagnosableError> {
        Ok(self.active_process.clone())
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
    fn execute_action(&mut self, _action: &MacroAction) -> Result<(), DiagnosableError> {
        Err(DiagnosableError::new(
            ErrorPhase::MacroExecution,
            "synthesized input adapter is not implemented for this compositor",
        )
        .with_capability(Capability::SynthesizedInput))
    }
}

pub fn real_registration_unavailable() -> DiagnosableError {
    unsupported_protocol(Capability::GlobalShortcut)
}
