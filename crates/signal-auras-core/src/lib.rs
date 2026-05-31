mod config;
mod consent;
mod error;
mod hotkey;
mod key;
mod macro_plan;
mod motion;
mod scope;
mod stats;

pub use config::{
    BindingDefinition, BindingMode, HotkeyBinding, InputProviderBackend, InputProviderConfig,
    InputProviderMode, InputProviderOutput, LuaAutomationConfiguration, RegistrationState,
    RuntimeMotion,
};
pub use consent::ConsentDecision;
pub use error::{
    AdapterDiagnostic, Capability, CapabilityAvailability, CapabilityKind, CapabilityReport,
    CapabilitySet, CapabilityStatus, DiagnosableError, ErrorPhase,
};
pub use hotkey::{
    BindingTrigger, CleanupReport, CompositeTrigger, HotkeyId, Modifier, ModifierSet, MouseButton,
    MouseTrigger, RegistrationId, ShortcutRegistrationHandle, ShortcutRegistrationState,
    WheelDirection,
};
pub use key::{KeyCategory, KeyToken};
pub use macro_plan::{
    execute_plan, execute_plan_with_inter_action_delay, InputEmission, MacroAction,
    MacroDefinition, MacroRunId, MacroRunPoll, MacroRunState, MacroScheduler,
    SynthesizedInputRequest, SynthesizedInputState,
};
pub use motion::{
    AutomationDefaults, MotionDefinition, MotionDiscardReason, MotionInputEvent, MotionInputState,
    MotionRuntime, MotionRuntimeEvent, MotionToken, MotionTrigger, RepeatDefinition,
    RepeatInterval, DEFAULT_MOTION_DURATION,
};
pub use scope::{
    ActiveProcessConfidence, ActiveProcessContext, FocusFreshness, FocusFreshnessPolicy,
    ProcessName, ScopeDecision, ScopeDenial, ScopeDenialKind, ScopeSelection, ScriptScope,
    DEFAULT_FOCUS_STALE_THRESHOLD,
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
