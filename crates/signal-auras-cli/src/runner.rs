use crate::prompt::{stdin_is_interactive, ScopePrompt, TerminalPrompt};
use signal_auras_core::{
    execute_plan_with_inter_action_delay, ActiveProcessProvider, BindingMode, BindingTrigger,
    CapabilitySet, DiagnosableError, ErrorPhase, HotkeyBinding, HotkeyId, HotkeyRegistrar,
    MacroDefinition, MacroExecutor, MacroScheduler, MotionInputEvent, MotionInputState,
    MotionRuntime, MotionRuntimeEvent, MotionTrigger, RuntimeMotion, RuntimeStats, ScopeDecision,
    ScopeSelection, ShutdownReason, SynthesizedInputRequest,
};
use signal_auras_lua::load_lua_file;
use signal_auras_wayland::RealWaylandAdapter;
use std::{
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

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
    let options = parse_run_args(&args)?;
    let mut adapter = RealWaylandAdapter::new();
    start_live_real_runner_with_options(&options.lua_file, prompt, &mut adapter, options.log)
        .map(|_| ())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    pub lua_file: PathBuf,
    pub log: RuntimeLog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeLog {
    pub verbose: bool,
    color: bool,
    started_at: Instant,
}

impl Default for RuntimeLog {
    fn default() -> Self {
        Self::new(false)
    }
}

impl RuntimeLog {
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            color: std::env::var_os("NO_COLOR").is_none(),
            started_at: Instant::now(),
        }
    }

    fn debug(self, message: impl AsRef<str>) {
        if self.verbose {
            println!("{}", self.render("DEBUG", message.as_ref()));
        }
    }

    fn render(self, level: &'static str, message: &str) -> String {
        let elapsed = self.started_at.elapsed();
        let timestamp = format!("{:>5}.{:03}s", elapsed.as_secs(), elapsed.subsec_millis());
        let event = field_value(message, "event").unwrap_or("runtime");
        let rest = without_field(message, "event");
        let level = paint(self.color, level_color(level), format!("{level:<5}"));
        let event = paint(self.color, "\x1b[36m", format!("{event:<30}"));
        let timestamp = paint(self.color, "\x1b[2m", timestamp);
        format!("{timestamp}  {level}  {event}  {rest}")
    }
}

pub fn parse_run_args(args: &[String]) -> Result<RunOptions, DiagnosableError> {
    if args.first().map(String::as_str) != Some("run") {
        return Err(DiagnosableError::new(
            ErrorPhase::ArgumentValidation,
            "usage: signal-auras run [--verbose|-v] <lua-file>",
        ));
    }
    let mut verbose = false;
    let mut paths = Vec::new();
    for arg in &args[1..] {
        match arg.as_str() {
            "--verbose" | "-v" => verbose = true,
            value if value.starts_with('-') => {
                return Err(DiagnosableError::new(
                    ErrorPhase::ArgumentValidation,
                    format!("unsupported run option '{value}'"),
                ));
            }
            value => paths.push(PathBuf::from(value)),
        }
    }
    if paths.len() != 1 {
        return Err(DiagnosableError::new(
            ErrorPhase::ArgumentValidation,
            "usage: signal-auras run [--verbose|-v] <lua-file>",
        ));
    }
    Ok(RunOptions {
        lua_file: paths.remove(0),
        log: RuntimeLog::new(verbose),
    })
}

fn level_color(level: &str) -> &'static str {
    match level {
        "DEBUG" => "\x1b[34m",
        "WARN" => "\x1b[33m",
        "ERROR" => "\x1b[31m",
        _ => "\x1b[37m",
    }
}

fn paint(enabled: bool, color: &str, value: impl AsRef<str>) -> String {
    if enabled {
        format!("{color}{}\x1b[0m", value.as_ref())
    } else {
        value.as_ref().to_string()
    }
}

fn field_value<'a>(message: &'a str, field: &str) -> Option<&'a str> {
    let prefix = format!("{field}=");
    message
        .split_whitespace()
        .find_map(|part| part.strip_prefix(&prefix))
}

fn without_field(message: &str, field: &str) -> String {
    let prefix = format!("{field}=");
    message
        .split_whitespace()
        .filter(|part| !part.starts_with(&prefix))
        .collect::<Vec<_>>()
        .join("  ")
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
    let bindings = config.bindings_for_scope(scope.clone());
    let motions = config.motions_for_scope(scope);
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
        &motions,
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
    let bindings = config.bindings_for_scope(scope.clone());
    let motions = config.motions_for_scope(scope.clone());
    let required = CapabilitySet::for_configuration_scope(&config, &scope);
    println!("provider selected=kde-plasma-wayland");
    adapter.configure_input_provider(config.input_provider.as_ref(), config.leader.as_ref())?;
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
    let shutdown_reason = match run_lifecycle(
        &bindings,
        &motions,
        &active_adapter,
        adapter,
        lifecycle,
        &mut stats,
    ) {
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
    start_live_real_runner_with_options(lua_file, prompt, adapter, RuntimeLog::default())
}

pub fn start_live_real_runner_with_options(
    lua_file: &Path,
    prompt: &mut impl ScopePrompt,
    adapter: &mut RealWaylandAdapter,
    log: RuntimeLog,
) -> Result<RuntimeStats, DiagnosableError> {
    println!("startup script_path={}", lua_file.display());
    let config = load_lua_file(lua_file)?;
    println!("script_validation result=ok");
    log.debug(format!(
        "event=config_loaded bindings={} motions={} input_provider={} leader={}",
        config.bindings().len(),
        config.motions().len(),
        config.input_provider.is_some(),
        config
            .leader
            .as_ref()
            .map(signal_auras_core::MotionToken::describe)
            .unwrap_or_else(|| "none".to_string())
    ));

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
    let bindings = config.bindings_for_scope(scope.clone());
    let motions = config.motions_for_scope(scope.clone());
    let required = CapabilitySet::for_configuration_scope(&config, &scope);
    println!("provider selected=kde-plasma-wayland");
    adapter.configure_input_provider(config.input_provider.as_ref(), config.leader.as_ref())?;
    if let Some(summary) = adapter.input_provider_summary() {
        log.debug(format!("event=input_provider_configured {summary}"));
    } else {
        log.debug("event=input_provider_configured provider=none");
    }
    log.debug(format!(
        "event=capability_probe_start required={}",
        required
            .iter()
            .map(|kind| kind.to_string())
            .collect::<Vec<_>>()
            .join(",")
    ));
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

    let shutdown_reason =
        match run_live_real_lifecycle(&bindings, &motions, adapter, &mut stats, log) {
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
    MotionInput(MotionInputEvent),
    MotionRepeatTick(MotionTrigger),
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
    motions: &[RuntimeMotion],
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
    let mut motion_runtime =
        MotionRuntime::new(motions.iter().map(|motion| motion.definition.clone()));
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
            RunnerEvent::MotionInput(event) => {
                for event in motion_runtime.handle_input(event) {
                    handle_motion_runtime_event(
                        event,
                        motions,
                        active_process_provider,
                        executor,
                        &mut scheduler,
                        stats,
                    )?;
                }
            }
            RunnerEvent::MotionRepeatTick(trigger) => {
                if motion_runtime.repeat_is_active(&trigger) {
                    if let Some(motion) = motions
                        .iter()
                        .find(|motion| motion.definition.trigger == trigger)
                    {
                        handle_motion_repeat_tick(
                            motion,
                            active_process_provider,
                            executor,
                            &mut scheduler,
                            stats,
                        )?;
                    }
                }
            }
            RunnerEvent::Shutdown(reason) => return Ok(reason),
            RunnerEvent::RuntimeError(error) => return Err(error),
        }
    }
}

fn run_live_real_lifecycle(
    bindings: &[HotkeyBinding],
    motions: &[RuntimeMotion],
    adapter: &mut RealWaylandAdapter,
    stats: &mut RuntimeStats,
    log: RuntimeLog,
) -> Result<ShutdownReason, DiagnosableError> {
    CTRL_C_REQUESTED.store(false, Ordering::SeqCst);
    install_ctrl_c_handler();
    let mut scheduler = MacroScheduler::default();
    let active_adapter = RealWaylandAdapter::new();
    let mut motion_runtime =
        MotionRuntime::new(motions.iter().map(|motion| motion.definition.clone()));
    let mut repeat_ticks = motions
        .iter()
        .filter_map(|motion| {
            motion
                .definition
                .repeat
                .as_ref()
                .map(|repeat| (motion.definition.trigger.clone(), repeat.interval.min_ms))
        })
        .collect::<Vec<_>>();
    let mut last_repeat_ticks = std::collections::BTreeMap::new();
    loop {
        if CTRL_C_REQUESTED.load(Ordering::SeqCst) {
            return Ok(ShutdownReason::CtrlC);
        }
        if let Some(hotkey) = adapter.next_shortcut_event() {
            log.debug(format!(
                "event=shortcut_received hotkey={}",
                hotkey.as_str()
            ));
            if let Some(binding) = bindings
                .iter()
                .find(|binding| binding.trigger == BindingTrigger::Keyboard(hotkey.clone()))
            {
                let active_context = adapter.active_process_context()?;
                log.debug(format!(
                    "event=active_process_context confidence={:?} visible_name={} app_id={} window_class={}",
                    active_context.confidence,
                    active_context
                        .visible_name
                        .as_ref()
                        .map(signal_auras_core::ProcessName::as_str)
                        .unwrap_or("none"),
                    active_context.app_id.as_deref().unwrap_or("none"),
                    active_context.window_class.as_deref().unwrap_or("none")
                ));
                handle_trigger_with_context(
                    binding,
                    active_context,
                    adapter,
                    &mut scheduler,
                    stats,
                )?;
            }
        }
        while let Some(event) = adapter.next_motion_input_event()? {
            log.debug(format!(
                "event=motion_input token={} state={}",
                event.token.describe(),
                motion_input_state_label(event.state)
            ));
            for event in motion_runtime.handle_input(event) {
                match &event {
                    MotionRuntimeEvent::Triggered {
                        trigger,
                        starts_repeat,
                    } => log.debug(format!(
                        "event=motion_triggered trigger={} starts_repeat={starts_repeat}",
                        trigger_label_for_log(trigger)
                    )),
                    MotionRuntimeEvent::RepeatCancelled { trigger } => log.debug(format!(
                        "event=motion_repeat_cancelled trigger={}",
                        trigger_label_for_log(trigger)
                    )),
                }
                handle_motion_runtime_event(
                    event,
                    motions,
                    &active_adapter,
                    adapter,
                    &mut scheduler,
                    stats,
                )?;
            }
        }
        let now = Instant::now();
        for (trigger, interval_ms) in &mut repeat_ticks {
            if !motion_runtime.repeat_is_active(trigger) {
                last_repeat_ticks.remove(trigger);
                continue;
            }
            let due = last_repeat_ticks.get(trigger).is_none_or(|last_tick| {
                now.duration_since(*last_tick).as_millis() >= *interval_ms as u128
            });
            if due {
                if let Some(motion) = motions
                    .iter()
                    .find(|motion| motion.definition.trigger == *trigger)
                {
                    log.debug(format!(
                        "event=motion_repeat_tick trigger={} interval_ms={interval_ms}",
                        trigger_label_for_log(trigger)
                    ));
                    handle_motion_repeat_tick(
                        motion,
                        &active_adapter,
                        adapter,
                        &mut scheduler,
                        stats,
                    )?;
                }
                last_repeat_ticks.insert(trigger.clone(), now);
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn trigger_label_for_log(trigger: &MotionTrigger) -> String {
    trigger.describe().replace(' ', "/")
}

fn motion_input_state_label(state: MotionInputState) -> &'static str {
    match state {
        MotionInputState::Pressed => "pressed",
        MotionInputState::Released => "released",
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
            let result = execute_macro_definition(&binding.macro_definition, 0, executor, stats);
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

fn handle_motion_runtime_event<P, E>(
    event: MotionRuntimeEvent,
    motions: &[RuntimeMotion],
    active_process_provider: &P,
    executor: &mut E,
    scheduler: &mut MacroScheduler,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError>
where
    P: ActiveProcessProvider,
    E: MacroExecutor,
{
    match event {
        MotionRuntimeEvent::Triggered { trigger, .. } => {
            let Some(motion) = motions
                .iter()
                .find(|motion| motion.definition.trigger == trigger)
            else {
                return Ok(());
            };
            handle_motion_trigger(motion, active_process_provider, executor, scheduler, stats)
        }
        MotionRuntimeEvent::RepeatCancelled { trigger } => {
            println!("motion_repeat_cancelled trigger={}", trigger.describe());
            Ok(())
        }
    }
}

fn handle_motion_trigger<P, E>(
    motion: &RuntimeMotion,
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
    let trigger_label = motion.definition.trigger.describe();
    stats.record_trigger(&trigger_label);
    match motion.definition.mode {
        BindingMode::Consume => stats.record_consumed_trigger_event(),
        BindingMode::Passthrough => stats.record_passthrough_trigger_event(),
    }
    match motion.scope.decide_context(&active_context) {
        ScopeDecision::Allowed => {
            stats.record_active_process_match();
            if let Some(macro_definition) = &motion.definition.macro_definition {
                execute_motion_macro(
                    &trigger_label,
                    macro_definition,
                    motion.definition.inter_action_delay_ms,
                    executor,
                    scheduler,
                    stats,
                )?;
            }
        }
        ScopeDecision::Denied { reason } => {
            stats.denied_action_count += 1;
            stats.scope_mismatch_count += 1;
            stats.record_active_process_non_match();
            if active_context.matchable_name().is_none() {
                stats.record_metadata_unavailable();
            }
            println!("denied_motion trigger={} reason={reason}", trigger_label);
        }
    }
    Ok(())
}

fn handle_motion_repeat_tick<P, E>(
    motion: &RuntimeMotion,
    active_process_provider: &P,
    executor: &mut E,
    scheduler: &mut MacroScheduler,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError>
where
    P: ActiveProcessProvider,
    E: MacroExecutor,
{
    let Some(repeat) = &motion.definition.repeat else {
        return Ok(());
    };
    let active_context = active_process_provider.active_process_context()?;
    let trigger_label = format!("{} repeat", motion.definition.trigger.describe());
    match motion.scope.decide_context(&active_context) {
        ScopeDecision::Allowed => {
            stats.record_active_process_match();
            execute_motion_macro(
                &trigger_label,
                &repeat.macro_definition,
                motion.definition.inter_action_delay_ms,
                executor,
                scheduler,
                stats,
            )?;
        }
        ScopeDecision::Denied { reason } => {
            stats.denied_action_count += 1;
            stats.scope_mismatch_count += 1;
            stats.record_active_process_non_match();
            if active_context.matchable_name().is_none() {
                stats.record_metadata_unavailable();
            }
            println!(
                "denied_motion_repeat trigger={} reason={reason}",
                trigger_label
            );
        }
    }
    Ok(())
}

fn execute_motion_macro<E>(
    trigger_label: &str,
    macro_definition: &MacroDefinition,
    inter_action_delay_ms: u64,
    executor: &mut E,
    scheduler: &mut MacroScheduler,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError>
where
    E: MacroExecutor,
{
    let guard = match scheduler.begin(trigger_label) {
        Ok(guard) => guard,
        Err(error) => {
            stats.denied_action_count += 1;
            return Err(error);
        }
    };
    let result = execute_macro_definition(macro_definition, inter_action_delay_ms, executor, stats);
    scheduler.finish(guard);
    match result {
        Ok(()) => {
            stats.macro_success_count += 1;
            Ok(())
        }
        Err(error) => {
            stats.macro_failure_count += 1;
            Err(error)
        }
    }
}

fn execute_macro_definition<E>(
    macro_definition: &MacroDefinition,
    inter_action_delay_ms: u64,
    executor: &mut E,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError>
where
    E: MacroExecutor,
{
    let mut sequence = 0usize;
    execute_plan_with_inter_action_delay(macro_definition, inter_action_delay_ms, |action| {
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
    })
}
