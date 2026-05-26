use signal_auras_core::{
    ActiveProcessConfidence, ActiveProcessContext, ActiveProcessProvider, BindingMode,
    BindingTrigger, Capability, CapabilityAvailability, CapabilityKind, CapabilityReport,
    CapabilitySet, CapabilityStatus, CompositeTrigger, DiagnosableError, ErrorPhase, HotkeyBinding,
    HotkeyRegistrar, InputEmission, LuaAutomationConfiguration, MacroAction, MacroDefinition,
    MacroExecutor, ModifierSet, MotionDefinition, MotionTrigger, MouseTrigger, ProcessName,
    RegistrationId, RuntimeStats, ShortcutRegistrationState, SynthesizedInputRequest,
    WheelDirection,
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
        trigger: signal_auras_core::BindingTrigger::keyboard(
            signal_auras_core::HotkeyId::parse("F5").unwrap(),
        ),
        mode: signal_auras_core::BindingMode::Consume,
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
fn composite_consume_bindings_require_observation_and_consumption_capabilities() {
    let binding = HotkeyBinding {
        trigger: BindingTrigger::Composite(CompositeTrigger::new(
            ModifierSet::parse(["Ctrl"]).unwrap(),
            MouseTrigger::Wheel(WheelDirection::Up),
        )),
        mode: BindingMode::Consume,
        scope: signal_auras_core::ScopeSelection::ExplicitGlobal,
        macro_definition: MacroDefinition::new(vec![MacroAction::key("Left").unwrap()]).unwrap(),
        registration_state: signal_auras_core::RegistrationState::Pending,
    };

    let required = CapabilitySet::for_bindings([&binding]);

    assert!(required.contains(CapabilityKind::CompositePointerObservation));
    assert!(required.contains(CapabilityKind::CompositePointerConsumption));
}

#[test]
fn composite_passthrough_bindings_do_not_require_consumption_capability() {
    let binding = HotkeyBinding {
        trigger: BindingTrigger::Composite(CompositeTrigger::new(
            ModifierSet::parse(["Ctrl"]).unwrap(),
            MouseTrigger::Wheel(WheelDirection::Down),
        )),
        mode: BindingMode::Passthrough,
        scope: signal_auras_core::ScopeSelection::ExplicitGlobal,
        macro_definition: MacroDefinition::new(vec![MacroAction::key("Right").unwrap()]).unwrap(),
        registration_state: signal_auras_core::RegistrationState::Pending,
    };

    let required = CapabilitySet::for_bindings([&binding]);

    assert!(required.contains(CapabilityKind::CompositePointerObservation));
    assert!(!required.contains(CapabilityKind::CompositePointerConsumption));
}

#[test]
fn motion_consume_requires_input_observation_consumption_and_synthesized_input() {
    let motion = MotionDefinition::new(
        MotionTrigger::parse(["<Leader>", "<LClick>", "<LClick>"]).unwrap(),
        BindingMode::Consume,
        None,
        Some(signal_auras_core::RepeatDefinition::new(
            MotionTrigger::parse(["<Leader>", "<LClick>"]).unwrap(),
            signal_auras_core::RepeatInterval::new(50, 80).unwrap(),
            MacroDefinition::new(vec![MacroAction::mouse_click(
                signal_auras_core::MouseButton::Left,
            )])
            .unwrap(),
        )),
        0,
    )
    .unwrap();
    let config = LuaAutomationConfiguration::with_bindings_and_motions(
        None,
        None,
        signal_auras_core::AutomationDefaults::default(),
        None,
        Vec::new(),
        vec![motion],
    )
    .unwrap();

    let required = CapabilitySet::for_configuration(&config);

    assert!(required.contains(CapabilityKind::CompositePointerObservation));
    assert!(required.contains(CapabilityKind::CompositePointerConsumption));
    assert!(required.contains(CapabilityKind::SynthesizedInput));
}

#[test]
fn motion_passthrough_does_not_require_input_consumption() {
    let motion = MotionDefinition::new(
        MotionTrigger::parse(["<Leader>", "<LClick>", "<LClick>"]).unwrap(),
        BindingMode::Passthrough,
        Some(MacroDefinition::new(vec![MacroAction::text("x").unwrap()]).unwrap()),
        None,
        0,
    )
    .unwrap();
    let config = LuaAutomationConfiguration::with_bindings_and_motions(
        None,
        None,
        signal_auras_core::AutomationDefaults::default(),
        None,
        Vec::new(),
        vec![motion],
    )
    .unwrap();

    let required = CapabilitySet::for_configuration(&config);

    assert!(required.contains(CapabilityKind::CompositePointerObservation));
    assert!(!required.contains(CapabilityKind::CompositePointerConsumption));
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

#[test]
fn kde_provider_selection_accepts_only_kde_wayland_with_services() {
    use signal_auras_wayland::capability::{
        KdeEnvironment, KdeServiceAvailability, KdeSession, KdeSessionState,
    };

    let services = KdeServiceAvailability::available();
    let session = KdeSession::detect(KdeEnvironment {
        wayland_display: Some("wayland-0".into()),
        session_type: Some("wayland".into()),
        current_desktop: Some("KDE".into()),
        services: services.clone(),
    })
    .unwrap();
    assert_eq!(session.state, KdeSessionState::Available);

    let non_kde = KdeSession::detect(KdeEnvironment {
        wayland_display: Some("wayland-0".into()),
        session_type: Some("wayland".into()),
        current_desktop: Some("GNOME".into()),
        services: services.clone(),
    })
    .unwrap_err();
    assert_eq!(non_kde.phase, ErrorPhase::CapabilityProbe);
    assert!(non_kde.message.contains("KDE Plasma Wayland"));

    let x11 = KdeSession::detect(KdeEnvironment {
        wayland_display: None,
        session_type: Some("x11".into()),
        current_desktop: Some("KDE".into()),
        services: services.clone(),
    })
    .unwrap_err();
    assert!(x11.message.contains("Wayland"));

    let missing_kwin = KdeSession::detect(KdeEnvironment {
        wayland_display: Some("wayland-0".into()),
        session_type: Some("wayland".into()),
        current_desktop: Some("KDE".into()),
        services: KdeServiceAvailability {
            kwin: false,
            ..services
        },
    })
    .unwrap_err();
    assert!(missing_kwin.message.contains("KWin"));
}

#[test]
fn kde_capability_probe_maps_missing_services_to_required_capabilities() {
    use signal_auras_wayland::capability::{KdeEnvironment, KdeServiceAvailability};

    let adapter = signal_auras_wayland::KdePlasmaAdapter::from_environment(KdeEnvironment {
        wayland_display: Some("wayland-0".into()),
        session_type: Some("wayland".into()),
        current_desktop: Some("KDE".into()),
        services: KdeServiceAvailability {
            kwin: true,
            kglobalaccel: false,
            portal: false,
        },
    });
    let required = CapabilitySet::new([
        CapabilityKind::GlobalShortcut,
        CapabilityKind::ActiveProcessMetadata,
        CapabilityKind::SynthesizedInput,
    ]);
    let report = adapter.probe_capabilities(&required);

    assert_eq!(
        report
            .status(CapabilityKind::GlobalShortcut)
            .unwrap()
            .availability,
        CapabilityAvailability::Unsupported
    );
    assert_eq!(
        report
            .status(CapabilityKind::ActiveProcessMetadata)
            .unwrap()
            .availability,
        CapabilityAvailability::Available
    );
    assert_eq!(
        report
            .status(CapabilityKind::SynthesizedInput)
            .unwrap()
            .availability,
        CapabilityAvailability::Unsupported
    );
}

#[test]
fn kde_global_shortcut_registration_uses_owned_handles_and_cleanup() {
    let mut adapter = signal_auras_wayland::RealWaylandAdapter::from_environment(
        signal_auras_wayland::capability::KdeEnvironment {
            wayland_display: Some("wayland-0".into()),
            session_type: Some("wayland".into()),
            current_desktop: Some("KDE".into()),
            services: signal_auras_wayland::capability::KdeServiceAvailability::available(),
        },
    );
    let binding = HotkeyBinding {
        trigger: signal_auras_core::BindingTrigger::keyboard(
            signal_auras_core::HotkeyId::parse("F5").unwrap(),
        ),
        mode: signal_auras_core::BindingMode::Consume,
        scope: signal_auras_core::ScopeSelection::ExplicitGlobal,
        macro_definition: MacroDefinition::new(vec![MacroAction::text("x").unwrap()]).unwrap(),
        registration_state: signal_auras_core::RegistrationState::Pending,
    };

    let id = adapter.register(binding).unwrap();
    assert_eq!(id.as_str(), "kde-F5");
    assert_eq!(adapter.cleanup_report().attempted, 1);

    adapter.unregister_all().unwrap();
    assert_eq!(adapter.cleanup_report().attempted, 0);
}

#[test]
fn kde_bridge_maps_callbacks_to_shortcut_events_and_unloads_once() {
    let mut bridge = signal_auras_wayland::kde_bridge::KdeBridgeState::active_for_test(1);
    bridge
        .push_shortcut_event(signal_auras_core::HotkeyId::parse("F5").unwrap())
        .unwrap();

    let event = bridge.next_shortcut_event().unwrap().unwrap();
    assert_eq!(event.as_str(), "F5");
    assert!(bridge.next_shortcut_event().unwrap().is_none());

    assert_eq!(bridge.unload().unwrap().attempted, 1);
    assert_eq!(bridge.unload().unwrap().attempted, 0);
}

#[test]
fn kde_active_process_context_preserves_app_id_window_class_and_pid() {
    let snapshot = signal_auras_wayland::process::KwinWindowSnapshot {
        visible_name: Some(ProcessName::parse("kate").unwrap()),
        process_id: Some(42),
        app_id: Some("org.kde.kate".into()),
        window_class: Some("kate".into()),
        privileged: false,
    };

    let context = snapshot.into_context();

    assert_eq!(context.visible_name.unwrap().as_str(), "kate");
    assert_eq!(context.process_id, Some(42));
    assert_eq!(context.app_id.as_deref(), Some("org.kde.kate"));
    assert_eq!(context.window_class.as_deref(), Some("kate"));
    assert_eq!(context.confidence, ActiveProcessConfidence::Exact);
}

#[test]
fn kde_privileged_active_surface_is_unavailable_for_matching() {
    let context = signal_auras_wayland::process::KwinWindowSnapshot {
        visible_name: Some(ProcessName::parse("kscreenlocker").unwrap()),
        process_id: None,
        app_id: None,
        window_class: None,
        privileged: true,
    }
    .into_context();

    assert_eq!(context.confidence, ActiveProcessConfidence::Unavailable);
    assert!(context.matchable_name().is_none());
}

#[test]
fn portal_input_validates_text_before_emitting() {
    let good = SynthesizedInputRequest::new(MacroAction::text("hello").unwrap(), 1);
    let bad = SynthesizedInputRequest::new(MacroAction::text("snowman ☃").unwrap(), 1);

    assert_eq!(
        signal_auras_wayland::portal::synthesize_input(good).unwrap(),
        InputEmission::Emitted
    );
    assert!(signal_auras_wayland::portal::synthesize_input(bad).is_err());
}

#[test]
fn portal_input_session_closes_idempotently() {
    let mut session = signal_auras_wayland::portal::PortalInputSession::open();
    let request = SynthesizedInputRequest::new(MacroAction::key("Enter").unwrap(), 1);

    assert_eq!(session.synthesize(request).unwrap(), InputEmission::Emitted);
    assert_eq!(session.close().attempted, 1);
    assert_eq!(session.close().attempted, 0);
}

#[test]
fn portal_input_session_accepts_mouse_click_requests() {
    let session = signal_auras_wayland::portal::PortalInputSession::open();
    let request = SynthesizedInputRequest::new(
        MacroAction::mouse_click(signal_auras_core::MouseButton::Left),
        1,
    );

    assert_eq!(session.synthesize(request).unwrap(), InputEmission::Emitted);
}
