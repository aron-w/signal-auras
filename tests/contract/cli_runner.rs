use signal_auras_cli::prompt::{ScopePrompt, TerminalPrompt};
use signal_auras_cli::runner::{
    parse_doctor_args, parse_run_args, start_controller_runner_with_lifecycle,
    start_real_controller_runner_with_lifecycle, start_real_runner_with_lifecycle, start_runner,
    start_runner_with_lifecycle, DoctorCommand, RunnerEvent, RunnerLifecycle,
};
use signal_auras_core::{
    available_capability_report, ActiveProcessProvider, CapabilityKind, CapabilitySet,
    ConsentDecision, DiagnosableError, ErrorPhase, HotkeyBinding, HotkeyRegistrar, MacroAction,
    MacroExecutor, ProcessName, RegistrationId, ScopeDenialKind, ScopeSelection, ShutdownReason,
};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use signal_auras_wayland::RealWaylandAdapter;

#[test]
fn cli_requires_run_and_one_path() {
    assert!(parse_run_args(&[]).is_err());
    assert!(parse_run_args(&["run".into(), "a.lua".into(), "b.lua".into()]).is_err());
    let options = parse_run_args(&["run".into(), "--verbose".into(), "a.lua".into()]).unwrap();
    assert_eq!(options.lua_file, PathBuf::from("a.lua"));
    assert!(options.log.verbose);
}

#[test]
fn cli_accepts_doctor_keys_command_shape() {
    let options =
        parse_doctor_args(&["doctor".into(), "keys".into(), "examples/poe2.lua".into()]).unwrap();

    assert_eq!(options.command, DoctorCommand::Keys);
    assert_eq!(options.lua_file, PathBuf::from("examples/poe2.lua"));
}

#[test]
fn prompt_accepts_process_scope_for_current_run() {
    let input = Cursor::new("1\npoe2.exe, alacritty\n");
    let mut output = Vec::new();
    let decision = {
        let mut prompt = TerminalPrompt::new(input, &mut output, true);
        prompt.resolve_missing_scope().unwrap()
    };

    let scope = decision.into_scope().unwrap().unwrap();
    assert_eq!(
        scope,
        ScopeSelection::process_list(vec![
            ProcessName::parse("poe2.exe").unwrap(),
            ProcessName::parse("alacritty").unwrap(),
        ])
        .unwrap()
    );
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("No scope declared by script"));
    assert!(output.contains("Process names"));
}

#[test]
fn stale_focus_denial_diagnostic_identifies_rule_age_and_threshold() {
    let scope = ScopeSelection::process_list(vec![ProcessName::parse("kate").unwrap()]).unwrap();
    let now = Instant::now();
    let mut context =
        signal_auras_core::ActiveProcessContext::name_only(ProcessName::parse("kate").unwrap());
    context.captured_at = now - Duration::from_millis(2_050);

    let signal_auras_core::ScopeDecision::Denied { diagnostic, .. } =
        scope.decide_context_at(&context, now)
    else {
        panic!("stale metadata should deny");
    };

    assert_eq!(diagnostic.kind, ScopeDenialKind::StaleFocus);
    let fields = diagnostic.render_fields();
    assert!(fields.contains("configured_rule=processes:kate"));
    assert!(fields.contains("metadata_age_ms=2050"));
    assert!(fields.contains("stale_threshold_ms=2000"));
}

#[test]
fn prompt_requires_literal_global_confirmation() {
    let mut accepted_output = Vec::new();
    let accepted = {
        let input = Cursor::new("2\nGLOBAL\n");
        let mut prompt = TerminalPrompt::new(input, &mut accepted_output, true);
        prompt.resolve_missing_scope().unwrap()
    };
    assert_eq!(
        accepted.into_scope().unwrap().unwrap(),
        ScopeSelection::ExplicitGlobal
    );
    assert!(String::from_utf8(accepted_output)
        .unwrap()
        .contains("Type GLOBAL"));

    let input = Cursor::new("2\nglobal\n");
    let mut rejected_output = Vec::new();
    let mut prompt = TerminalPrompt::new(input, &mut rejected_output, true);
    assert_eq!(
        prompt.resolve_missing_scope().unwrap(),
        ConsentDecision::Cancel
    );
}

#[test]
fn prompt_cancel_and_non_interactive_do_not_select_global_scope() {
    let input = Cursor::new("3\n");
    let mut output = Vec::new();
    let mut prompt = TerminalPrompt::new(input, &mut output, true);
    assert_eq!(
        prompt
            .resolve_missing_scope()
            .unwrap()
            .into_scope()
            .unwrap(),
        None
    );

    let input = Cursor::new("");
    let mut output = Vec::new();
    let mut prompt = TerminalPrompt::new(input, &mut output, false);
    let error = prompt
        .resolve_missing_scope()
        .unwrap()
        .into_scope()
        .unwrap_err();
    assert_eq!(error.phase, ErrorPhase::ScopePrompt);
    assert!(error.message.contains("interactive stdin"));
    assert!(output.is_empty());
}

#[test]
fn cancelled_missing_scope_exits_before_registration() {
    let lua_file = write_lua(
        r#"
        return {
          hotkeys = {
            ["F5"] = macro {
              text "/hideout",
            },
          },
        }
        "#,
    );
    let mut prompt = FixedPrompt(ConsentDecision::Cancel);
    let mut registrar = RecordingRegistrar::default();
    let active = StaticActive(None);
    let mut executor = CountingExecutor::default();

    let stats = start_runner(
        &lua_file,
        &mut prompt,
        &mut registrar,
        &active,
        &mut executor,
    )
    .unwrap();

    assert_eq!(stats.registration_attempts, 0);
    assert_eq!(registrar.registered_scopes.len(), 0);
    assert_eq!(registrar.unregister_calls, 0);
    assert_eq!(executor.actions, 0);
}

#[test]
fn ctrl_c_shutdown_unregisters_hotkeys_and_returns_summary_stats() {
    let lua_file = write_lua(
        r#"
        return {
          scope = { processes = { "poe2.exe" } },
          hotkeys = {
            ["F5"] = macro {
              text "/hideout",
            },
          },
        }
        "#,
    );
    let mut prompt = FixedPrompt(ConsentDecision::Cancel);
    let mut registrar = RecordingRegistrar::default();
    let active = StaticActive(Some(ProcessName::parse("poe2.exe").unwrap()));
    let mut executor = CountingExecutor::default();

    let stats = start_runner(
        &lua_file,
        &mut prompt,
        &mut registrar,
        &active,
        &mut executor,
    )
    .unwrap();

    assert_eq!(registrar.registered_scopes.len(), 1);
    assert_eq!(registrar.unregister_calls, 1);
    assert_eq!(executor.actions, 0);
    let summary = stats.render_summary(ShutdownReason::CtrlC);
    assert!(summary.contains("final_summary"));
    assert!(summary.contains("reason=CtrlC"));
    assert!(summary.contains("triggers=0"));
    assert!(summary.contains("successes=0"));
}

#[test]
fn lifecycle_runtime_error_unregisters_hotkeys() {
    let lua_file = write_lua(
        r#"
        return {
          scope = { processes = { "poe2.exe" } },
          hotkeys = {
            ["F5"] = macro {
              text "/hideout",
            },
          },
        }
        "#,
    );
    let mut prompt = FixedPrompt(ConsentDecision::Cancel);
    let mut registrar = RecordingRegistrar::default();
    let active = StaticActive(Some(ProcessName::parse("poe2.exe").unwrap()));
    let mut executor = CountingExecutor::default();
    let mut lifecycle = ScriptedLifecycle::new(vec![RunnerEvent::RuntimeError(
        DiagnosableError::new(ErrorPhase::Trigger, "event loop failed"),
    )]);

    let error = start_runner_with_lifecycle(
        &lua_file,
        &mut prompt,
        &mut registrar,
        &active,
        &mut executor,
        &mut lifecycle,
    )
    .unwrap_err();

    assert_eq!(error.phase, ErrorPhase::Trigger);
    assert_eq!(registrar.registered_scopes.len(), 1);
    assert_eq!(registrar.unregister_calls, 1);
}

#[test]
fn registration_failure_runs_startup_cleanup() {
    let lua_file = write_lua(
        r#"
        return {
          scope = { processes = { "poe2.exe" } },
          hotkeys = {
            ["F5"] = macro {
              text "/hideout",
            },
          },
        }
        "#,
    );
    let mut prompt = FixedPrompt(ConsentDecision::Cancel);
    let mut registrar = FailingRegistrar::default();
    let active = StaticActive(Some(ProcessName::parse("poe2.exe").unwrap()));
    let mut executor = CountingExecutor::default();
    let mut lifecycle = ScriptedLifecycle::new(vec![RunnerEvent::Shutdown(ShutdownReason::CtrlC)]);

    let error = start_runner_with_lifecycle(
        &lua_file,
        &mut prompt,
        &mut registrar,
        &active,
        &mut executor,
        &mut lifecycle,
    )
    .unwrap_err();

    assert_eq!(error.phase, ErrorPhase::Registration);
    assert_eq!(registrar.unregister_calls, 1);
}

#[test]
fn real_runner_fails_before_registration_when_global_shortcut_capability_is_unsupported() {
    let lua_file = write_lua(
        r#"
        return {
          scope = { processes = { "poe2.exe" } },
          hotkeys = {
            ["F5"] = macro {
              text "/hideout",
            },
          },
        }
        "#,
    );
    let mut prompt = FixedPrompt(ConsentDecision::Cancel);
    let mut adapter =
        RealWaylandAdapter::from_environment(signal_auras_wayland::capability::KdeEnvironment {
            wayland_display: Some("wayland-0".into()),
            session_type: Some("wayland".into()),
            current_desktop: Some("KDE".into()),
            services: signal_auras_wayland::capability::KdeServiceAvailability {
                kwin: true,
                kglobalaccel: false,
                portal: true,
            },
        });
    let mut lifecycle = ScriptedLifecycle::new(vec![RunnerEvent::Shutdown(ShutdownReason::CtrlC)]);

    let error =
        start_real_runner_with_lifecycle(&lua_file, &mut prompt, &mut adapter, &mut lifecycle)
            .unwrap_err();

    assert_eq!(error.phase, ErrorPhase::CapabilityProbe);
    assert_eq!(
        error.capability,
        Some(signal_auras_core::Capability::GlobalShortcut)
    );
}

#[test]
fn real_runner_fails_before_registration_on_non_kde_wayland_session() {
    let lua_file = write_lua(
        r#"
        return {
          scope = { processes = { "kate" } },
          hotkeys = {
            ["F5"] = macro {
              text "hello",
            },
          },
        }
        "#,
    );
    let mut prompt = FixedPrompt(ConsentDecision::Cancel);
    let mut adapter =
        RealWaylandAdapter::from_environment(signal_auras_wayland::capability::KdeEnvironment {
            wayland_display: Some("wayland-0".into()),
            session_type: Some("wayland".into()),
            current_desktop: Some("GNOME".into()),
            services: signal_auras_wayland::capability::KdeServiceAvailability::available(),
        });
    let mut lifecycle = ScriptedLifecycle::new(vec![RunnerEvent::Shutdown(ShutdownReason::CtrlC)]);

    let error =
        start_real_runner_with_lifecycle(&lua_file, &mut prompt, &mut adapter, &mut lifecycle)
            .unwrap_err();

    assert_eq!(error.phase, ErrorPhase::CapabilityProbe);
    assert!(error.message.contains("KDE Plasma Wayland"));
}

#[test]
fn real_runner_fails_before_registration_on_non_wayland_session() {
    let lua_file = write_lua(
        r#"
        return {
          scope = { processes = { "kate" } },
          hotkeys = {
            ["F5"] = macro {
              text "hello",
            },
          },
        }
        "#,
    );
    let mut prompt = FixedPrompt(ConsentDecision::Cancel);
    let mut adapter =
        RealWaylandAdapter::from_environment(signal_auras_wayland::capability::KdeEnvironment {
            wayland_display: None,
            session_type: Some("x11".into()),
            current_desktop: Some("KDE".into()),
            services: signal_auras_wayland::capability::KdeServiceAvailability::available(),
        });
    let mut lifecycle = ScriptedLifecycle::new(vec![RunnerEvent::Shutdown(ShutdownReason::CtrlC)]);

    let error =
        start_real_runner_with_lifecycle(&lua_file, &mut prompt, &mut adapter, &mut lifecycle)
            .unwrap_err();

    assert_eq!(error.phase, ErrorPhase::CapabilityProbe);
    assert!(error.message.contains("Wayland"));
}

#[test]
fn real_runner_registers_kde_shortcut_when_required_services_are_available() {
    let lua_file = write_lua(
        r#"
        return {
          scope = { processes = { "kate" } },
          hotkeys = {
            ["F5"] = macro {
              delay(1),
            },
          },
        }
        "#,
    );
    let mut prompt = FixedPrompt(ConsentDecision::Cancel);
    let mut adapter =
        RealWaylandAdapter::from_environment(signal_auras_wayland::capability::KdeEnvironment {
            wayland_display: Some("wayland-0".into()),
            session_type: Some("wayland".into()),
            current_desktop: Some("KDE".into()),
            services: signal_auras_wayland::capability::KdeServiceAvailability::available(),
        });
    let mut lifecycle = ScriptedLifecycle::new(vec![RunnerEvent::Shutdown(ShutdownReason::CtrlC)]);

    let stats =
        start_real_runner_with_lifecycle(&lua_file, &mut prompt, &mut adapter, &mut lifecycle)
            .unwrap();

    assert_eq!(stats.registration_successes, 1);
    assert_eq!(adapter.cleanup_report().attempted, 0);
}

#[test]
fn real_runner_fails_before_registration_when_kde_metadata_is_unavailable_for_process_scope() {
    let lua_file = write_lua(
        r#"
        return {
          scope = { processes = { "kate" } },
          hotkeys = {
            ["F5"] = macro {
              delay(1),
            },
          },
        }
        "#,
    );
    let mut prompt = FixedPrompt(ConsentDecision::Cancel);
    let mut adapter =
        RealWaylandAdapter::from_environment(signal_auras_wayland::capability::KdeEnvironment {
            wayland_display: Some("wayland-0".into()),
            session_type: Some("wayland".into()),
            current_desktop: Some("KDE".into()),
            services: signal_auras_wayland::capability::KdeServiceAvailability {
                kwin: false,
                kglobalaccel: true,
                portal: true,
            },
        });
    let mut lifecycle = ScriptedLifecycle::new(vec![RunnerEvent::Shutdown(ShutdownReason::CtrlC)]);

    let error =
        start_real_runner_with_lifecycle(&lua_file, &mut prompt, &mut adapter, &mut lifecycle)
            .unwrap_err();

    assert_eq!(error.phase, ErrorPhase::CapabilityProbe);
    assert_eq!(
        error.capability,
        Some(signal_auras_core::Capability::ActiveProcess)
    );
    assert!(error.message.contains("KWin"));
}

#[test]
fn real_runner_fails_before_registration_when_kde_portal_input_is_unavailable() {
    let lua_file = write_lua(
        r#"
        return {
          scope = { processes = { "kate" } },
          hotkeys = {
            ["F5"] = macro {
              text "hello",
            },
          },
        }
        "#,
    );
    let mut prompt = FixedPrompt(ConsentDecision::Cancel);
    let mut adapter =
        RealWaylandAdapter::from_environment(signal_auras_wayland::capability::KdeEnvironment {
            wayland_display: Some("wayland-0".into()),
            session_type: Some("wayland".into()),
            current_desktop: Some("KDE".into()),
            services: signal_auras_wayland::capability::KdeServiceAvailability {
                kwin: true,
                kglobalaccel: true,
                portal: false,
            },
        });
    let mut lifecycle = ScriptedLifecycle::new(vec![RunnerEvent::Shutdown(ShutdownReason::CtrlC)]);

    let error =
        start_real_runner_with_lifecycle(&lua_file, &mut prompt, &mut adapter, &mut lifecycle)
            .unwrap_err();

    assert_eq!(error.phase, ErrorPhase::CapabilityProbe);
    assert_eq!(
        error.capability,
        Some(signal_auras_core::Capability::SynthesizedInput)
    );
}

#[test]
fn synthesized_input_denial_is_reported_before_macro_success() {
    struct DenyingExecutor;

    impl MacroExecutor for DenyingExecutor {
        fn execute_action(&mut self, _action: &MacroAction) -> Result<(), DiagnosableError> {
            unreachable!("execute_input_request handles text input")
        }

        fn execute_input_request(
            &mut self,
            _request: signal_auras_core::SynthesizedInputRequest,
        ) -> Result<signal_auras_core::InputEmission, DiagnosableError> {
            Ok(signal_auras_core::InputEmission::Denied)
        }
    }

    let binding = HotkeyBinding {
        trigger: signal_auras_core::BindingTrigger::keyboard(
            signal_auras_core::HotkeyId::parse("F5").unwrap(),
        ),
        mode: signal_auras_core::BindingMode::Consume,
        scope: ScopeSelection::ExplicitGlobal,
        macro_definition: signal_auras_core::MacroDefinition::new(vec![MacroAction::text(
            "/hideout",
        )
        .unwrap()])
        .unwrap(),
        registration_state: signal_auras_core::RegistrationState::Registered,
    };
    let active = StaticActive(Some(ProcessName::parse("poe2.exe").unwrap()));
    let mut executor = DenyingExecutor;
    let mut scheduler = signal_auras_core::MacroScheduler::default();
    let mut stats = signal_auras_core::RuntimeStats::new();

    let error = signal_auras_cli::runner::handle_trigger(
        &binding,
        &active,
        &mut executor,
        &mut scheduler,
        &mut stats,
    )
    .unwrap_err();

    assert_eq!(error.phase, ErrorPhase::MacroExecution);
    assert_eq!(stats.synthesized_input_emitted_count, 0);
    assert_eq!(stats.synthesized_input_denied_count, 1);
    assert_eq!(stats.macro_success_count, 0);
}

#[test]
fn controller_runner_validates_registers_and_executes_callback_outputs() {
    let lua_file = write_lua(
        r#"
        sa.hotkey({
          trigger = "F5",
          scope = { processes = { "poe2.exe" } },
          callback = "hideout",
        })
        sa.callback("hideout", function()
          sa.input.key("Enter")
          sa.input.text("/hideout")
          sa.input.key("Enter")
        end)
        "#,
    );
    let mut registrar = RecordingRegistrar::default();
    let active = StaticActive(Some(ProcessName::parse("poe2.exe").unwrap()));
    let mut executor = CountingExecutor::default();
    let mut lifecycle = ScriptedLifecycle::new(vec![
        RunnerEvent::Callback {
            hotkey: signal_auras_core::HotkeyId::parse("F5").unwrap(),
            received_at: Instant::now(),
        },
        RunnerEvent::Shutdown(ShutdownReason::CtrlC),
    ]);
    let required = CapabilitySet::new([
        CapabilityKind::GlobalShortcut,
        CapabilityKind::ActiveProcessMetadata,
        CapabilityKind::SynthesizedInput,
    ]);

    let stats = start_controller_runner_with_lifecycle(
        &lua_file,
        &mut registrar,
        &active,
        &mut executor,
        &mut lifecycle,
        available_capability_report(&required, "test"),
    )
    .unwrap();

    assert_eq!(registrar.registered_scopes.len(), 1);
    assert_eq!(registrar.unregister_calls, 1);
    assert_eq!(executor.actions, 3);
    assert_eq!(stats.registration_successes, 1);
    assert_eq!(stats.callback_event_received_count, 1);
    assert_eq!(stats.synthesized_input_emitted_count, 3);
}

#[test]
fn controller_runner_fails_before_registration_when_output_capability_denied() {
    let lua_file = write_lua(
        r#"
        sa.hotkey({ trigger = "F5", callback = "hideout" })
        sa.callback("hideout", function()
          sa.input.text("/hideout")
        end)
        "#,
    );
    let mut registrar = RecordingRegistrar::default();
    let active = StaticActive(None);
    let mut executor = CountingExecutor::default();
    let mut lifecycle = ScriptedLifecycle::new(vec![RunnerEvent::Shutdown(ShutdownReason::CtrlC)]);
    let report = available_capability_report(
        &CapabilitySet::new([CapabilityKind::GlobalShortcut]),
        "test",
    );

    let error = start_controller_runner_with_lifecycle(
        &lua_file,
        &mut registrar,
        &active,
        &mut executor,
        &mut lifecycle,
        report,
    )
    .unwrap_err();

    assert_eq!(error.phase, ErrorPhase::CapabilityProbe);
    assert_eq!(registrar.registered_scopes.len(), 0);
    assert_eq!(executor.actions, 0);
}

#[test]
fn real_controller_runner_fails_before_registration_when_output_capability_unavailable() {
    let lua_file = write_lua(
        r#"
        sa.hotkey({ trigger = "F5", callback = "hideout" })
        sa.callback("hideout", function()
          sa.input.text("/hideout")
        end)
        "#,
    );
    let mut adapter =
        RealWaylandAdapter::from_environment(signal_auras_wayland::capability::KdeEnvironment {
            wayland_display: Some("wayland-0".into()),
            session_type: Some("wayland".into()),
            current_desktop: Some("KDE".into()),
            services: signal_auras_wayland::capability::KdeServiceAvailability {
                kwin: true,
                kglobalaccel: true,
                portal: false,
            },
        });
    let mut lifecycle = ScriptedLifecycle::new(vec![RunnerEvent::Shutdown(ShutdownReason::CtrlC)]);

    let error =
        start_real_controller_runner_with_lifecycle(&lua_file, &mut adapter, &mut lifecycle)
            .unwrap_err();

    assert_eq!(error.phase, ErrorPhase::CapabilityProbe);
    assert_eq!(
        error.capability,
        Some(signal_auras_core::Capability::SynthesizedInput)
    );
    assert_eq!(adapter.cleanup_report().attempted, 0);
}

#[derive(Clone)]
struct FixedPrompt(ConsentDecision);

impl ScopePrompt for FixedPrompt {
    fn resolve_missing_scope(&mut self) -> Result<ConsentDecision, DiagnosableError> {
        Ok(self.0.clone())
    }
}

#[derive(Default)]
struct RecordingRegistrar {
    registered_scopes: Vec<ScopeSelection>,
    unregister_calls: usize,
}

impl HotkeyRegistrar for RecordingRegistrar {
    fn register(&mut self, binding: HotkeyBinding) -> Result<RegistrationId, DiagnosableError> {
        self.registered_scopes.push(binding.scope);
        Ok(RegistrationId::new(format!(
            "registration-{}",
            self.registered_scopes.len()
        )))
    }

    fn unregister_all(&mut self) -> Result<(), DiagnosableError> {
        self.unregister_calls += 1;
        Ok(())
    }
}

struct StaticActive(Option<ProcessName>);

impl ActiveProcessProvider for StaticActive {
    fn active_process_name(&self) -> Result<Option<ProcessName>, DiagnosableError> {
        Ok(self.0.clone())
    }
}

#[derive(Default)]
struct CountingExecutor {
    actions: usize,
}

struct ScriptedLifecycle {
    events: std::vec::IntoIter<RunnerEvent>,
}

impl ScriptedLifecycle {
    fn new(events: Vec<RunnerEvent>) -> Self {
        Self {
            events: events.into_iter(),
        }
    }
}

impl RunnerLifecycle for ScriptedLifecycle {
    fn next_event(&mut self) -> Result<RunnerEvent, DiagnosableError> {
        self.events
            .next()
            .ok_or_else(|| DiagnosableError::new(ErrorPhase::Shutdown, "test lifecycle exhausted"))
    }
}

#[derive(Default)]
struct FailingRegistrar {
    unregister_calls: usize,
}

impl HotkeyRegistrar for FailingRegistrar {
    fn register(&mut self, _binding: HotkeyBinding) -> Result<RegistrationId, DiagnosableError> {
        Err(DiagnosableError::new(
            ErrorPhase::Registration,
            "registration failed",
        ))
    }

    fn unregister_all(&mut self) -> Result<(), DiagnosableError> {
        self.unregister_calls += 1;
        Ok(())
    }
}

impl MacroExecutor for CountingExecutor {
    fn execute_action(&mut self, _action: &MacroAction) -> Result<(), DiagnosableError> {
        self.actions += 1;
        Ok(())
    }
}

fn write_lua(source: &str) -> PathBuf {
    static NEXT_FILE_ID: AtomicU64 = AtomicU64::new(0);
    let mut path = std::env::temp_dir();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = NEXT_FILE_ID.fetch_add(1, Ordering::SeqCst);
    path.push(format!(
        "signal-auras-cli-runner-{}-{unique}-{sequence}.lua",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    path
}
