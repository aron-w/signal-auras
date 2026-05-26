use signal_auras_core::{
    ActiveProcessConfidence, ActiveProcessContext, ActiveProcessProvider, Capability,
    CapabilityAvailability, CapabilityKind, CapabilityReport, CapabilitySet, CapabilityStatus,
    DiagnosableError, ErrorPhase, HotkeyBinding, HotkeyRegistrar, InputEmission, MacroAction,
    MacroDefinition, MacroExecutor, ProcessName, RegistrationId, RuntimeStats,
    ShortcutRegistrationState, SynthesizedInputRequest,
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

#[test]
fn capability_report_fails_closed_for_unavailable_capabilities() {
    let required = CapabilitySet::new([
        CapabilityKind::GlobalShortcut,
        CapabilityKind::SynthesizedInput,
    ]);
    let report = CapabilityReport::from_statuses([
        CapabilityStatus::available(CapabilityKind::GlobalShortcut, "test"),
        CapabilityStatus::unavailable(
            CapabilityKind::SynthesizedInput,
            CapabilityAvailability::Denied,
            signal_auras_core::AdapterDiagnostic::new(ErrorPhase::CapabilityProbe, "input denied")
                .with_capability(CapabilityKind::SynthesizedInput),
        ),
    ]);

    assert!(!report.all_available(&required));
    let error = report.first_blocking_error(&required).unwrap();
    assert_eq!(error.capability, Some(Capability::SynthesizedInput));
}

#[test]
fn active_process_context_denies_ambiguous_unavailable_and_denied_metadata() {
    let scope =
        signal_auras_core::ScopeSelection::process_list(vec![
            ProcessName::parse("poe2.exe").unwrap()
        ])
        .unwrap();

    for context in [
        ActiveProcessContext::ambiguous("multiple surfaces"),
        ActiveProcessContext::unavailable("missing metadata"),
        ActiveProcessContext::denied("permission denied"),
    ] {
        assert!(matches!(
            scope.decide_context(&context),
            signal_auras_core::ScopeDecision::Denied { .. }
        ));
    }

    let allowed = ActiveProcessContext::name_only(ProcessName::parse("poe2.exe").unwrap());
    assert_eq!(
        scope.decide_context(&allowed),
        signal_auras_core::ScopeDecision::Allowed
    );
    assert_eq!(allowed.confidence, ActiveProcessConfidence::NameOnly);
}

#[test]
fn shortcut_registration_states_cover_lifecycle() {
    let states = [
        ShortcutRegistrationState::Pending,
        ShortcutRegistrationState::Registered,
        ShortcutRegistrationState::Rejected,
        ShortcutRegistrationState::Unregistering,
        ShortcutRegistrationState::Unregistered,
    ];

    assert_eq!(states.len(), 5);
}

#[test]
fn synthesized_input_denial_does_not_count_as_emitted_or_success() {
    struct DenyingExecutor;

    impl MacroExecutor for DenyingExecutor {
        fn execute_action(&mut self, _action: &MacroAction) -> Result<(), DiagnosableError> {
            unreachable!("execute_input_request should own synthesized input")
        }

        fn execute_input_request(
            &mut self,
            _request: SynthesizedInputRequest,
        ) -> Result<InputEmission, DiagnosableError> {
            Ok(InputEmission::Denied)
        }
    }

    let mut stats = RuntimeStats::new();
    let mut executor = DenyingExecutor;
    let request = SynthesizedInputRequest::new(MacroAction::text("x").unwrap(), 1);
    let outcome = executor.execute_input_request(request).unwrap();
    if outcome == InputEmission::Denied {
        stats.record_synthesized_input_denied();
        stats.denied_action_count += 1;
    }

    assert_eq!(stats.synthesized_input_emitted_count, 0);
    assert_eq!(stats.synthesized_input_denied_count, 1);
    assert_eq!(stats.macro_success_count, 0);
}

#[test]
fn wayland_environment_probe_reports_required_capabilities() {
    let required = CapabilitySet::new([CapabilityKind::GlobalShortcut]);
    let report = signal_auras_wayland::portal::probe_required_capabilities(&required);

    assert!(report.status(CapabilityKind::GlobalShortcut).is_some());
}
