mod config;
mod consent;
mod error;
mod hotkey;
mod macro_plan;
mod scope;
mod stats;

pub use config::{HotkeyBinding, LuaAutomationConfiguration, RegistrationState};
pub use consent::ConsentDecision;
pub use error::{Capability, DiagnosableError, ErrorPhase};
pub use hotkey::{HotkeyId, RegistrationId};
pub use macro_plan::{execute_plan, MacroAction, MacroDefinition, MacroScheduler};
pub use scope::{ProcessName, ScopeDecision, ScopeSelection, ScriptScope};
pub use stats::{RuntimeStats, ShutdownReason};

pub trait ActiveProcessProvider {
    fn active_process_name(&self) -> Result<Option<ProcessName>, DiagnosableError>;
}

pub trait HotkeyRegistrar {
    fn register(&mut self, binding: HotkeyBinding) -> Result<RegistrationId, DiagnosableError>;
    fn unregister_all(&mut self) -> Result<(), DiagnosableError>;
}

pub trait MacroExecutor {
    fn execute_action(&mut self, action: &MacroAction) -> Result<(), DiagnosableError>;
}
