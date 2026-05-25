use signal_auras_cli::prompt::ScopePrompt;
use signal_auras_cli::runner::{
    handle_trigger, start_runner, start_runner_with_lifecycle, RunnerEvent, RunnerLifecycle,
};
use signal_auras_core::{
    ActiveProcessProvider, ConsentDecision, DiagnosableError, ErrorPhase, HotkeyBinding, HotkeyId,
    HotkeyRegistrar, MacroAction, MacroDefinition, MacroExecutor, MacroScheduler, ProcessName,
    RegistrationId, RegistrationState, RuntimeStats, ScopeSelection,
};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct Active(Option<ProcessName>);

impl ActiveProcessProvider for Active {
    fn active_process_name(
        &self,
    ) -> Result<Option<ProcessName>, signal_auras_core::DiagnosableError> {
        Ok(self.0.clone())
    }
}

#[derive(Default)]
struct Executor {
    actions: usize,
    fail_after: Option<usize>,
}

impl MacroExecutor for Executor {
    fn execute_action(
        &mut self,
        _action: &MacroAction,
    ) -> Result<(), signal_auras_core::DiagnosableError> {
        if self.fail_after == Some(self.actions) {
            return Err(DiagnosableError::new(
                ErrorPhase::MacroExecution,
                "executor failed",
            ));
        }
        self.actions += 1;
        Ok(())
    }
}

#[test]
fn scoped_trigger_executes_only_for_matching_process() {
    let binding = HotkeyBinding {
        hotkey: HotkeyId::parse("F5").unwrap(),
        scope: ScopeSelection::process_list(vec![ProcessName::parse("poe2.exe").unwrap()]).unwrap(),
        macro_definition: MacroDefinition::new(vec![MacroAction::text("/hideout").unwrap()])
            .unwrap(),
        registration_state: RegistrationState::Registered,
    };
    let mut executor = Executor::default();
    let mut stats = RuntimeStats::new();
    let mut scheduler = MacroScheduler::default();

    handle_trigger(
        &binding,
        &Active(Some(ProcessName::parse("poe2.exe").unwrap())),
        &mut executor,
        &mut scheduler,
        &mut stats,
    )
    .unwrap();
    assert_eq!(executor.actions, 1);

    handle_trigger(
        &binding,
        &Active(Some(ProcessName::parse("other").unwrap())),
        &mut executor,
        &mut scheduler,
        &mut stats,
    )
    .unwrap();
    assert_eq!(executor.actions, 1);
    assert_eq!(stats.scope_mismatch_count, 1);
}

#[test]
fn trigger_failure_stops_remaining_actions_and_updates_failure_stats() {
    let binding = HotkeyBinding {
        hotkey: HotkeyId::parse("F5").unwrap(),
        scope: ScopeSelection::process_list(vec![ProcessName::parse("poe2.exe").unwrap()]).unwrap(),
        macro_definition: MacroDefinition::new(vec![
            MacroAction::text("/hideout").unwrap(),
            MacroAction::key("Enter").unwrap(),
        ])
        .unwrap(),
        registration_state: RegistrationState::Registered,
    };
    let mut executor = Executor {
        actions: 0,
        fail_after: Some(1),
    };
    let mut stats = RuntimeStats::new();
    let mut scheduler = MacroScheduler::default();

    let error = handle_trigger(
        &binding,
        &Active(Some(ProcessName::parse("poe2.exe").unwrap())),
        &mut executor,
        &mut scheduler,
        &mut stats,
    )
    .unwrap_err();

    assert_eq!(error.phase, ErrorPhase::MacroExecution);
    assert_eq!(executor.actions, 1);
    assert_eq!(stats.macro_success_count, 0);
    assert_eq!(stats.macro_failure_count, 1);
    assert_eq!(stats.trigger_count_by_hotkey["F5"], 1);
}

#[test]
fn repeated_trigger_denial_keeps_macro_from_running_twice() {
    let binding = HotkeyBinding {
        hotkey: HotkeyId::parse("F5").unwrap(),
        scope: ScopeSelection::ExplicitGlobal,
        macro_definition: MacroDefinition::new(vec![MacroAction::text("/hideout").unwrap()])
            .unwrap(),
        registration_state: RegistrationState::Registered,
    };
    let mut executor = Executor::default();
    let mut stats = RuntimeStats::new();
    let mut scheduler = MacroScheduler::default();
    let _running = scheduler.begin("F5").unwrap();

    let error = handle_trigger(
        &binding,
        &Active(None),
        &mut executor,
        &mut scheduler,
        &mut stats,
    )
    .unwrap_err();

    assert_eq!(error.phase, ErrorPhase::Trigger);
    assert_eq!(executor.actions, 0);
    assert_eq!(stats.macro_success_count, 0);
    assert_eq!(stats.macro_failure_count, 0);
    assert_eq!(stats.denied_action_count, 1);
}

#[test]
fn prompt_selected_scope_is_current_run_only() {
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
    let active = Active(Some(ProcessName::parse("poe2.exe").unwrap()));

    let mut first_prompt = Prompt::new(ConsentDecision::ProcessScope(vec![ProcessName::parse(
        "poe2.exe",
    )
    .unwrap()]));
    let mut first_registrar = Registrar::default();
    let mut first_executor = Executor::default();
    start_runner(
        &lua_file,
        &mut first_prompt,
        &mut first_registrar,
        &active,
        &mut first_executor,
    )
    .unwrap();

    let mut second_prompt = Prompt::new(ConsentDecision::ExplicitGlobalConfirmed);
    let mut second_registrar = Registrar::default();
    let mut second_executor = Executor::default();
    start_runner(
        &lua_file,
        &mut second_prompt,
        &mut second_registrar,
        &active,
        &mut second_executor,
    )
    .unwrap();

    assert_eq!(
        first_registrar.scopes,
        vec![ScopeSelection::process_list(vec![ProcessName::parse("poe2.exe").unwrap(),]).unwrap()]
    );
    assert_eq!(
        second_registrar.scopes,
        vec![ScopeSelection::ExplicitGlobal]
    );
    assert_eq!(first_prompt.calls, 1);
    assert_eq!(second_prompt.calls, 1);
}

#[test]
fn shutdown_unregisters_after_allowed_and_denied_activity() {
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
    let mut prompt = Prompt::new(ConsentDecision::Cancel);
    let mut registrar = Registrar::default();
    let active = Active(Some(ProcessName::parse("poe2.exe").unwrap()));
    let mut executor = Executor::default();

    let stats = start_runner(
        &lua_file,
        &mut prompt,
        &mut registrar,
        &active,
        &mut executor,
    )
    .unwrap();

    assert_eq!(registrar.registrations, 1);
    assert_eq!(registrar.unregisters, 1);
    assert_eq!(stats.macro_success_count, 0);
    assert!(stats
        .render_summary(signal_auras_core::ShutdownReason::CtrlC)
        .contains("successes=0"));
}

#[test]
fn lifecycle_executes_hotkey_events_until_ctrl_c_shutdown() {
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
    let mut prompt = Prompt::new(ConsentDecision::Cancel);
    let mut registrar = Registrar::default();
    let active = Active(Some(ProcessName::parse("poe2.exe").unwrap()));
    let mut executor = Executor::default();
    let mut lifecycle = ScriptedLifecycle::new(vec![
        RunnerEvent::Hotkey(HotkeyId::parse("F5").unwrap()),
        RunnerEvent::Shutdown(signal_auras_core::ShutdownReason::CtrlC),
    ]);

    let stats = start_runner_with_lifecycle(
        &lua_file,
        &mut prompt,
        &mut registrar,
        &active,
        &mut executor,
        &mut lifecycle,
    )
    .unwrap();

    assert_eq!(executor.actions, 1);
    assert_eq!(stats.total_triggers(), 1);
    assert_eq!(stats.macro_success_count, 1);
    assert_eq!(registrar.unregisters, 1);
}

struct Prompt {
    decision: ConsentDecision,
    calls: usize,
}

impl Prompt {
    fn new(decision: ConsentDecision) -> Self {
        Self { decision, calls: 0 }
    }
}

impl ScopePrompt for Prompt {
    fn resolve_missing_scope(&mut self) -> Result<ConsentDecision, DiagnosableError> {
        self.calls += 1;
        Ok(self.decision.clone())
    }
}

#[derive(Default)]
struct Registrar {
    scopes: Vec<ScopeSelection>,
    registrations: usize,
    unregisters: usize,
}

impl HotkeyRegistrar for Registrar {
    fn register(&mut self, binding: HotkeyBinding) -> Result<RegistrationId, DiagnosableError> {
        self.registrations += 1;
        self.scopes.push(binding.scope);
        Ok(RegistrationId::new(format!(
            "test-registration-{}",
            self.registrations
        )))
    }

    fn unregister_all(&mut self) -> Result<(), DiagnosableError> {
        self.unregisters += 1;
        Ok(())
    }
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

fn write_lua(source: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    path.push(format!("signal-auras-runner-flow-{unique}.lua"));
    std::fs::write(&path, source).unwrap();
    path
}
