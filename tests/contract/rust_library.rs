use signal_auras_core::{
    ActiveProcessProvider, Capability, DiagnosableError, ErrorPhase, HotkeyBinding,
    HotkeyRegistrar, MacroAction, MacroDefinition, ProcessName, RegistrationId,
};

struct FailingRegistrar;

impl HotkeyRegistrar for FailingRegistrar {
    fn register(&mut self, _binding: HotkeyBinding) -> Result<RegistrationId, DiagnosableError> {
        Err(
            DiagnosableError::new(ErrorPhase::Registration, "unsupported protocol")
                .with_capability(Capability::GlobalShortcut),
        )
    }

    fn unregister_all(&mut self) -> Result<(), DiagnosableError> {
        Ok(())
    }
}

#[test]
fn adapter_contract_can_report_unsupported_protocol() {
    let macro_definition = MacroDefinition::new(vec![MacroAction::text("x").unwrap()]).unwrap();
    let binding = HotkeyBinding {
        hotkey: signal_auras_core::HotkeyId::parse("F5").unwrap(),
        scope: signal_auras_core::ScopeSelection::ExplicitGlobal,
        macro_definition,
        registration_state: signal_auras_core::RegistrationState::Pending,
    };
    let mut registrar = FailingRegistrar;
    let error = registrar.register(binding).unwrap_err();
    assert_eq!(error.capability, Some(Capability::GlobalShortcut));
}

#[test]
fn adapter_contract_can_report_denied_permission() {
    let error = signal_auras_wayland::diagnostics::denied_permission(Capability::SynthesizedInput);

    assert_eq!(error.phase, ErrorPhase::CapabilityProbe);
    assert_eq!(error.capability, Some(Capability::SynthesizedInput));
    assert!(error.message.contains("permission"));
}

#[test]
fn active_process_provider_can_report_unavailable_metadata() {
    struct UnavailableActiveProcess;

    impl ActiveProcessProvider for UnavailableActiveProcess {
        fn active_process_name(&self) -> Result<Option<ProcessName>, DiagnosableError> {
            Err(DiagnosableError::new(
                ErrorPhase::CapabilityProbe,
                "active process metadata unavailable",
            )
            .with_capability(Capability::ActiveProcess))
        }
    }

    let error = UnavailableActiveProcess.active_process_name().unwrap_err();

    assert_eq!(error.phase, ErrorPhase::CapabilityProbe);
    assert_eq!(error.capability, Some(Capability::ActiveProcess));
}
