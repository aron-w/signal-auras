mod config;
mod consent;
mod error;
mod hotkey;
mod macro_plan;
mod scope;
mod stats;

pub use config::{HotkeyBinding, LuaAutomationConfiguration, RegistrationState};
pub use consent::ConsentDecision;
pub use error::{
    AdapterDiagnostic, Capability, CapabilityAvailability, CapabilityKind, CapabilityReport,
    CapabilitySet, CapabilityStatus, DiagnosableError, ErrorPhase,
};
pub use hotkey::{
    CleanupReport, HotkeyId, RegistrationId, ShortcutRegistrationHandle, ShortcutRegistrationState,
};
pub use macro_plan::{
    execute_plan, InputEmission, MacroAction, MacroDefinition, MacroScheduler,
    SynthesizedInputRequest, SynthesizedInputState,
};
pub use scope::{
    ActiveProcessConfidence, ActiveProcessContext, ProcessName, ScopeDecision, ScopeSelection,
    ScriptScope,
};
pub use stats::{RuntimeStats, ShutdownReason};

pub trait ActiveProcessProvider {
    fn active_process_name(&self) -> Result<Option<ProcessName>, DiagnosableError>;

    fn active_process_context(&self) -> Result<ActiveProcessContext, DiagnosableError> {
        match self.active_process_name()? {
            Some(name) => Ok(ActiveProcessContext::name_only(name)),
            None => Ok(ActiveProcessContext::unavailable(
                "active process metadata is unavailable",
            )),
        }
    }
}

pub trait HotkeyRegistrar {
    fn register(&mut self, binding: HotkeyBinding) -> Result<RegistrationId, DiagnosableError>;
    fn unregister_all(&mut self) -> Result<(), DiagnosableError>;
}

pub trait MacroExecutor {
    fn execute_action(&mut self, action: &MacroAction) -> Result<(), DiagnosableError>;

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
