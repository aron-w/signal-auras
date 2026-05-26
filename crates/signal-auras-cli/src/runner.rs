use crate::prompt::{stdin_is_interactive, ScopePrompt, TerminalPrompt};
use signal_auras_core::{
    execute_plan, ActiveProcessProvider, BindingMode, BindingTrigger, CapabilitySet,
    DiagnosableError, ErrorPhase, HotkeyBinding, HotkeyId, HotkeyRegistrar, MacroExecutor,
    MacroScheduler, RuntimeStats, ScopeDecision, ScopeSelection, ShutdownReason,
    SynthesizedInputRequest,
};
use signal_auras_lua::load_lua_file;
use signal_auras_wayland::RealWaylandAdapter;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

pub struct StdioPrompt;

impl ScopePrompt for StdioPrompt {
    fn resolve_missing_scope(
        &mut self,
    ) -> Result<signal_auras_core::ConsentDecision, DiagnosableError> {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut prompt = TerminalPrompt::new(stdin.lock(), stdout.lock(), stdin_is_interactive());
        prompt.resolve_missing_scope()
    }
}

pub fn run_cli(
    args: impl IntoIterator<Item = String>,
    prompt: &mut impl ScopePrompt,
) -> Result<(), DiagnosableError> {
    let args = args.into_iter().collect::<Vec<_>>();
    let path = parse_run_args(&args)?;
    let mut adapter = RealWaylandAdapter::new();
    start_live_real_runner(&path, prompt, &mut adapter).map(|_| ())
}

pub fn parse_run_args(args: &[String]) -> Result<PathBuf, DiagnosableError> {
    if args.len() != 2 || args[0] != "run" {
        return Err(DiagnosableError::new(
            ErrorPhase::ArgumentValidation,
            "usage: signal-auras run <lua-file>",
        ));
    }
    Ok(PathBuf::from(&args[1]))
}

pub fn start_runner<R, P, E>(
    lua_file: &Path,
    prompt: &mut impl ScopePrompt,
    registrar: &mut R,
    active_process_provider: &P,
    executor: &mut E,
) -> Result<RuntimeStats, DiagnosableError>
where
    R: HotkeyRegistrar,
    P: ActiveProcessProvider,
    E: MacroExecutor,
{
    let mut lifecycle = ImmediateShutdown;
    start_runner_with_lifecycle(
        lua_file,
        prompt,
        registrar,
        active_process_provider,
        executor,
        &mut lifecycle,
    )
}

pub fn start_runner_with_lifecycle<R, P, E, L>(
    lua_file: &Path,
    prompt: &mut impl ScopePrompt,
    registrar: &mut R,
    active_process_provider: &P,
    executor: &mut E,
    lifecycle: &mut L,
) -> Result<RuntimeStats, DiagnosableError>
where
    R: HotkeyRegistrar,
    P: ActiveProcessProvider,
    E: MacroExecutor,
    L: RunnerLifecycle,
{
    println!("startup script_path={}", lua_file.display());
    let config = load_lua_file(lua_file)?;
    println!("script_validation result=ok");

    let scope = match config.scope.clone() {
        Some(script_scope) => ScopeSelection::from_script(script_scope),
        None => match prompt.resolve_missing_scope()?.into_scope()? {
            Some(scope) => scope,
            None => {
                println!("scope_prompt result=cancelled");
                return Ok(RuntimeStats::new());
            }
        },
    };
    println!("effective_scope {}", scope.describe());
    println!("capability_probe result=mock-adapter");

    let mut stats = RuntimeStats::new();
    let bindings = config.bindings_for_scope(scope);
    for binding in &bindings {
        stats.record_registration_attempt();
        match registrar.register(binding.clone()) {
            Ok(id) => {
                stats.record_registration_success();
                println!(
                    "binding_registered trigger={} mode={} id={}",
                    binding.trigger_label(),
                    binding.mode.as_str(),
                    id.as_str()
                );
            }
            Err(error) => {
                stats.record_registration_failure();
                cleanup_after_error(registrar, ErrorPhase::Registration)?;
                return Err(error);
            }
        }
    }

    let shutdown_reason = match run_lifecycle(
        &bindings,
        active_process_provider,
        executor,
        lifecycle,
        &mut stats,
    ) {
        Ok(reason) => reason,
        Err(error) => {
            println!("{}", stats.render_summary(ShutdownReason::RuntimeError));
            cleanup_after_error(registrar, ErrorPhase::Shutdown)?;
            return Err(error);
        }
    };

    println!("{}", stats.render_summary(shutdown_reason));
    registrar.unregister_all()?;
    Ok(stats)
}

pub fn start_real_runner_with_lifecycle<L>(
    lua_file: &Path,
    prompt: &mut impl ScopePrompt,
    adapter: &mut RealWaylandAdapter,
    lifecycle: &mut L,
) -> Result<RuntimeStats, DiagnosableError>
where
    L: RunnerLifecycle,
{
    println!("startup script_path={}", lua_file.display());
    let config = load_lua_file(lua_file)?;
    println!("script_validation result=ok");

    let scope = match config.scope.clone() {
        Some(script_scope) => ScopeSelection::from_script(script_scope),
        None => match prompt.resolve_missing_scope()?.into_scope()? {
            Some(scope) => scope,
            None => {
                println!("scope_prompt result=cancelled");
                return Ok(RuntimeStats::new());
            }
        },
    };
    println!("effective_scope {}", scope.describe());

    let mut stats = RuntimeStats::new();
    let bindings = config.bindings_for_scope(scope);
    let required = CapabilitySet::for_bindings(&bindings);
    println!("provider selected=kde-plasma-wayland");
    let report = adapter.probe_capabilities(&required);
    if let Some(error) = report.first_blocking_error(&required) {
        stats.record_capability_probe_failure();
        stats.record_permission_failure();
        println!("capability_probe result=failed error={error}");
        return Err(error);
    }
    stats.record_capability_probe_success();
    println!("capability_probe result=ok");

    for binding in &bindings {
        stats.record_registration_attempt();
        match adapter.register(binding.clone()) {
            Ok(id) => {
                stats.record_registration_success();
                println!(
                    "binding_registered trigger={} mode={} id={}",
                    binding.trigger_label(),
                    binding.mode.as_str(),
                    id.as_str()
                );
            }
            Err(error) => {
                stats.record_registration_failure();
                cleanup_after_error(adapter, ErrorPhase::Registration)?;
                return Err(error);
            }
        }
    }

    let active_adapter = RealWaylandAdapter::new();
    let shutdown_reason =
        match run_lifecycle(&bindings, &active_adapter, adapter, lifecycle, &mut stats) {
            Ok(reason) => reason,
            Err(error) => {
                println!("{}", stats.render_summary(ShutdownReason::RuntimeError));
                cleanup_after_error(adapter, ErrorPhase::Shutdown)?;
                return Err(error);
            }
        };

    println!("{}", stats.render_summary(shutdown_reason));
    adapter.cancel_pending()?;
    adapter.unregister_all()?;
    Ok(stats)
}

pub fn start_live_real_runner(
    lua_file: &Path,
    prompt: &mut impl ScopePrompt,
    adapter: &mut RealWaylandAdapter,
) -> Result<RuntimeStats, DiagnosableError> {
    println!("startup script_path={}", lua_file.display());
    let config = load_lua_file(lua_file)?;
    println!("script_validation result=ok");

    let scope = match config.scope.clone() {
        Some(script_scope) => ScopeSelection::from_script(script_scope),
        None => match prompt.resolve_missing_scope()?.into_scope()? {
            Some(scope) => scope,
            None => {
                println!("scope_prompt result=cancelled");
                return Ok(RuntimeStats::new());
            }
        },
    };
    println!("effective_scope {}", scope.describe());

    let mut stats = RuntimeStats::new();
    let bindings = config.bindings_for_scope(scope);
    let required = CapabilitySet::for_bindings(&bindings);
    println!("provider selected=kde-plasma-wayland");
    let report = adapter.probe_capabilities(&required);
    if let Some(error) = report.first_blocking_error(&required) {
        stats.record_capability_probe_failure();
        stats.record_permission_failure();
        println!("capability_probe result=failed error={error}");
        return Err(error);
    }
    stats.record_capability_probe_success();
    println!("capability_probe result=ok");

    for binding in &bindings {
        stats.record_registration_attempt();
        match adapter.register(binding.clone()) {
            Ok(id) => {
                stats.record_registration_success();
                println!(
                    "binding_registered trigger={} mode={} id={}",
                    binding.trigger_label(),
                    binding.mode.as_str(),
                    id.as_str()
                );
            }
            Err(error) => {
                stats.record_registration_failure();
                cleanup_after_error(adapter, ErrorPhase::Registration)?;
                return Err(error);
            }
        }
    }

    let shutdown_reason = match run_live_real_lifecycle(&bindings, adapter, &mut stats) {
        Ok(reason) => reason,
        Err(error) => {
            println!("{}", stats.render_summary(ShutdownReason::RuntimeError));
            cleanup_after_error(adapter, ErrorPhase::Shutdown)?;
            return Err(error);
        }
    };

    println!("{}", stats.render_summary(shutdown_reason));
    adapter.cancel_pending()?;
    adapter.unregister_all()?;
    Ok(stats)
}

pub trait RunnerLifecycle {
    fn next_event(&mut self) -> Result<RunnerEvent, DiagnosableError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerEvent {
    Hotkey(HotkeyId),
    Trigger(BindingTrigger),
    Shutdown(ShutdownReason),
    RuntimeError(DiagnosableError),
}

pub struct ImmediateShutdown;

impl RunnerLifecycle for ImmediateShutdown {
    fn next_event(&mut self) -> Result<RunnerEvent, DiagnosableError> {
        Ok(RunnerEvent::Shutdown(ShutdownReason::CtrlC))
    }
}

pub struct CtrlCShutdown {
    installed: bool,
}

impl CtrlCShutdown {
    pub fn new() -> Self {
        Self { installed: false }
    }
}

impl Default for CtrlCShutdown {
    fn default() -> Self {
        Self::new()
    }
}

impl RunnerLifecycle for CtrlCShutdown {
    fn next_event(&mut self) -> Result<RunnerEvent, DiagnosableError> {
        if !self.installed {
            install_ctrl_c_handler();
            self.installed = true;
        }
        while !CTRL_C_REQUESTED.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(50));
        }
        Ok(RunnerEvent::Shutdown(ShutdownReason::CtrlC))
    }
}

static CTRL_C_REQUESTED: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
fn install_ctrl_c_handler() {
    const SIGINT: i32 = 2;

    unsafe extern "C" {
        fn signal(sig: i32, handler: extern "C" fn(i32)) -> usize;
    }

    extern "C" fn handle_sigint(_signal: i32) {
        CTRL_C_REQUESTED.store(true, Ordering::SeqCst);
    }

    // Safety: installs a small SIGINT handler that only writes to an AtomicBool,
    // which is signal-safe for this purpose. The previous handler is intentionally
    // not restored because this terminal runner owns the process lifetime.
    unsafe {
        signal(SIGINT, handle_sigint);
    }
}

#[cfg(not(unix))]
fn install_ctrl_c_handler() {
    CTRL_C_REQUESTED.store(true, Ordering::SeqCst);
}

fn run_lifecycle<P, E, L>(
    bindings: &[HotkeyBinding],
    active_process_provider: &P,
    executor: &mut E,
    lifecycle: &mut L,
    stats: &mut RuntimeStats,
) -> Result<ShutdownReason, DiagnosableError>
where
    P: ActiveProcessProvider,
    E: MacroExecutor,
    L: RunnerLifecycle,
{
    let mut scheduler = MacroScheduler::default();
    loop {
        match lifecycle.next_event()? {
            RunnerEvent::Hotkey(hotkey) => {
                if let Some(binding) = bindings
                    .iter()
                    .find(|binding| binding.trigger == BindingTrigger::Keyboard(hotkey.clone()))
                {
                    handle_trigger(
                        binding,
                        active_process_provider,
                        executor,
                        &mut scheduler,
                        stats,
                    )?;
                }
            }
            RunnerEvent::Trigger(trigger) => {
                if let Some(binding) = bindings.iter().find(|binding| binding.trigger == trigger) {
                    handle_trigger(
                        binding,
                        active_process_provider,
                        executor,
                        &mut scheduler,
                        stats,
                    )?;
                }
            }
            RunnerEvent::Shutdown(reason) => return Ok(reason),
            RunnerEvent::RuntimeError(error) => return Err(error),
        }
    }
}

fn run_live_real_lifecycle(
    bindings: &[HotkeyBinding],
    adapter: &mut RealWaylandAdapter,
    stats: &mut RuntimeStats,
) -> Result<ShutdownReason, DiagnosableError> {
    CTRL_C_REQUESTED.store(false, Ordering::SeqCst);
    install_ctrl_c_handler();
    let mut scheduler = MacroScheduler::default();
    loop {
        if CTRL_C_REQUESTED.load(Ordering::SeqCst) {
            return Ok(ShutdownReason::CtrlC);
        }
        if let Some(hotkey) = adapter.next_shortcut_event() {
            if let Some(binding) = bindings
                .iter()
                .find(|binding| binding.trigger == BindingTrigger::Keyboard(hotkey.clone()))
            {
                let active_context = adapter.active_process_context()?;
                handle_trigger_with_context(
                    binding,
                    active_context,
                    adapter,
                    &mut scheduler,
                    stats,
                )?;
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn cleanup_after_error(
    registrar: &mut impl HotkeyRegistrar,
    phase: ErrorPhase,
) -> Result<(), DiagnosableError> {
    registrar.unregister_all().map_err(|error| {
        DiagnosableError::new(phase, format!("cleanup failed after runner error: {error}"))
    })
}

pub fn handle_trigger<P, E>(
    binding: &HotkeyBinding,
    active_process_provider: &P,
    executor: &mut E,
    scheduler: &mut MacroScheduler,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError>
where
    P: ActiveProcessProvider,
    E: MacroExecutor,
{
    let active_context = active_process_provider.active_process_context()?;
    handle_trigger_with_context(binding, active_context, executor, scheduler, stats)
}

fn handle_trigger_with_context<E>(
    binding: &HotkeyBinding,
    active_context: signal_auras_core::ActiveProcessContext,
    executor: &mut E,
    scheduler: &mut MacroScheduler,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError>
where
    E: MacroExecutor,
{
    let trigger_label = binding.trigger_label();
    stats.record_trigger(&trigger_label);
    match binding.mode {
        BindingMode::Consume => stats.record_consumed_trigger_event(),
        BindingMode::Passthrough => stats.record_passthrough_trigger_event(),
    }
    match binding.scope.decide_context(&active_context) {
        ScopeDecision::Allowed => {
            stats.record_active_process_match();
            let guard = match scheduler.begin(&trigger_label) {
                Ok(guard) => guard,
                Err(error) => {
                    stats.denied_action_count += 1;
                    return Err(error);
                }
            };
            let mut sequence = 0usize;
            let result = execute_plan(&binding.macro_definition, |action| {
                sequence += 1;
                let request = SynthesizedInputRequest::new(action.clone(), sequence);
                match executor.execute_input_request(request)? {
                    signal_auras_core::InputEmission::Emitted => {
                        stats.record_synthesized_input_emitted();
                        Ok(())
                    }
                    signal_auras_core::InputEmission::Denied => {
                        stats.record_synthesized_input_denied();
                        Err(DiagnosableError::new(
                            ErrorPhase::MacroExecution,
                            "synthesized input was denied",
                        ))
                    }
                    signal_auras_core::InputEmission::Failed => Err(DiagnosableError::new(
                        ErrorPhase::MacroExecution,
                        "synthesized input failed",
                    )),
                    signal_auras_core::InputEmission::Cancelled => Err(DiagnosableError::new(
                        ErrorPhase::Shutdown,
                        "synthesized input was cancelled",
                    )),
                }
            });
            scheduler.finish(guard);
            match result {
                Ok(()) => stats.macro_success_count += 1,
                Err(error) => {
                    stats.macro_failure_count += 1;
                    return Err(error);
                }
            }
        }
        ScopeDecision::Denied { reason } => {
            stats.denied_action_count += 1;
            stats.scope_mismatch_count += 1;
            stats.record_active_process_non_match();
            if active_context.matchable_name().is_none() {
                stats.record_metadata_unavailable();
            }
            println!("denied_trigger hotkey={} reason={reason}", trigger_label);
        }
    }
    Ok(())
}
