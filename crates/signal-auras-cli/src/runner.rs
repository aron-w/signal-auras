use crate::prompt::{stdin_is_interactive, ScopePrompt, TerminalPrompt};
use signal_auras_core::{
    execute_plan_with_inter_action_delay, ActiveProcessProvider, BindingMode, BindingTrigger,
    CapabilityKind, CapabilitySet, DiagnosableError, ErrorPhase, HotkeyBinding, HotkeyId,
    HotkeyRegistrar, InputProviderConfig, InputProviderMode, InputProviderOutput, MacroDefinition,
    MacroExecutor, MacroRunId, MacroRunPoll, MacroRunState, MacroScheduler, MotionDefinition,
    MotionInputEvent, MotionInputState, MotionRuntime, MotionRuntimeEvent, MotionToken,
    MotionTrigger, RuntimeMotion, RuntimeStats, ScopeDecision, ScopeSelection, ShutdownReason,
    SynthesizedInputRequest,
};
use signal_auras_lua::load_lua_file;
use signal_auras_wayland::{
    event_loop::{RuntimeSignalFd, RuntimeTimerFd},
    RealWaylandAdapter,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{self, IsTerminal},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    sync::Once,
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
    if args.first().map(String::as_str) == Some("doctor") {
        let options = parse_doctor_args(&args)?;
        let report = input_doctor_report(&options.lua_file)?;
        println!("{}", report.render());
        if report.ok {
            return Ok(());
        }
        return Err(DiagnosableError::new(
            ErrorPhase::CapabilityProbe,
            "input doctor found missing unsafe input permissions",
        ));
    }
    let options = parse_run_args(&args)?;
    init_runtime_logging(options.log);
    let mut adapter = RealWaylandAdapter::new();
    start_live_real_runner_with_options(&options.lua_file, prompt, &mut adapter, options.log)
        .map(|_| ())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    pub lua_file: PathBuf,
    pub log: RuntimeLog,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorOptions {
    pub lua_file: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeLog {
    pub verbose: bool,
    color_mode: ColorMode,
    color: bool,
    started_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

impl Default for RuntimeLog {
    fn default() -> Self {
        Self::new(false)
    }
}

impl RuntimeLog {
    pub fn new(verbose: bool) -> Self {
        Self::new_with_color_mode(verbose, ColorMode::Auto)
    }

    pub fn new_with_color_mode(verbose: bool, color_mode: ColorMode) -> Self {
        Self::with_color_enabled(verbose, color_mode, color_enabled(color_mode))
    }

    fn with_color_enabled(verbose: bool, color_mode: ColorMode, color: bool) -> Self {
        Self {
            verbose,
            color_mode,
            color,
            started_at: Instant::now(),
        }
    }

    fn debug(self, message: impl AsRef<str>) {
        if self.verbose {
            self.emit(tracing::Level::DEBUG, message.as_ref());
        }
    }

    fn warn(self, message: impl AsRef<str>) {
        self.emit(tracing::Level::WARN, message.as_ref());
    }

    fn elapsed_ms(self) -> u128 {
        self.started_at.elapsed().as_millis()
    }

    fn emit(self, level: tracing::Level, message: &str) {
        let elapsed_ms = self.elapsed_ms();
        let event = field_value(message, "event").unwrap_or("runtime");
        let details = without_field(message, "event");
        match level {
            tracing::Level::DEBUG => {
                tracing::debug!(runtime_elapsed_ms = elapsed_ms, event, details)
            }
            tracing::Level::WARN => {
                tracing::warn!(runtime_elapsed_ms = elapsed_ms, event, details)
            }
            tracing::Level::ERROR => {
                tracing::error!(runtime_elapsed_ms = elapsed_ms, event, details)
            }
            tracing::Level::INFO => {
                tracing::info!(runtime_elapsed_ms = elapsed_ms, event, details)
            }
            tracing::Level::TRACE => {
                tracing::trace!(runtime_elapsed_ms = elapsed_ms, event, details)
            }
        }
    }

    #[cfg(test)]
    fn render_plain(self, level: &'static str, message: &str) -> String {
        let elapsed = self.started_at.elapsed();
        let timestamp = format!("{:>5}.{:03}s", elapsed.as_secs(), elapsed.subsec_millis());
        let event = field_value(message, "event").unwrap_or("runtime");
        let rest = without_field(message, "event");
        format!("{timestamp}  {level:<5}  {event:<30}  {rest}")
    }
}

fn color_enabled(mode: ColorMode) -> bool {
    match mode {
        ColorMode::Auto => {
            std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none()
        }
        ColorMode::Always => true,
        ColorMode::Never => false,
    }
}

static TRACING_INIT: Once = Once::new();

fn init_runtime_logging(log: RuntimeLog) {
    TRACING_INIT.call_once(|| {
        let filter = if log.verbose {
            "signal_auras_cli=debug,signal_auras_wayland=debug"
        } else {
            "signal_auras_cli=info,signal_auras_wayland=warn"
        };
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(log.color)
            .with_target(false)
            .with_level(false)
            .with_writer(std::io::stderr)
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);
    });
}

pub fn parse_run_args(args: &[String]) -> Result<RunOptions, DiagnosableError> {
    if args.first().map(String::as_str) != Some("run") {
        return Err(DiagnosableError::new(
            ErrorPhase::ArgumentValidation,
            "usage: signal-auras run [--verbose|-v] [--color=auto|always|never] <lua-file>",
        ));
    }
    let mut verbose = false;
    let mut color_mode = ColorMode::Auto;
    let mut paths = Vec::new();
    for arg in &args[1..] {
        match arg.as_str() {
            "--verbose" | "-v" => verbose = true,
            "--color=auto" => color_mode = ColorMode::Auto,
            "--color=always" => color_mode = ColorMode::Always,
            "--color=never" => color_mode = ColorMode::Never,
            "--no-color" => color_mode = ColorMode::Never,
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
            "usage: signal-auras run [--verbose|-v] [--color=auto|always|never] <lua-file>",
        ));
    }
    Ok(RunOptions {
        lua_file: paths.remove(0),
        log: RuntimeLog::new_with_color_mode(verbose, color_mode),
    })
}

pub fn parse_doctor_args(args: &[String]) -> Result<DoctorOptions, DiagnosableError> {
    if args.first().map(String::as_str) != Some("doctor")
        || args.get(1).map(String::as_str) != Some("input")
        || args.len() != 3
    {
        return Err(DiagnosableError::new(
            ErrorPhase::ArgumentValidation,
            "usage: signal-auras doctor input <lua-file>",
        ));
    }
    Ok(DoctorOptions {
        lua_file: PathBuf::from(&args[2]),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputDoctorReport {
    pub ok: bool,
    lines: Vec<String>,
}

impl InputDoctorReport {
    pub fn render(&self) -> String {
        self.lines.join("\n")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InputPathStatus {
    Accessible,
    Missing(String),
    Denied(String),
    Duplicate,
    SelfGenerated(String),
}

trait InputPermissionProbe {
    fn current_user(&self) -> String;
    fn current_groups(&self) -> Vec<String>;
    fn read_access(&self, path: &Path) -> InputPathStatus;
    fn read_write_access(&self, path: &Path) -> InputPathStatus;
    fn symlink_target(&self, path: &Path) -> Option<PathBuf>;
    fn stable_path_for(&self, path: &Path) -> Option<PathBuf>;
    fn device_name(&self, path: &Path) -> Option<String>;
}

struct RealInputPermissionProbe;

impl InputPermissionProbe for RealInputPermissionProbe {
    fn current_user(&self) -> String {
        std::env::var("USER").unwrap_or_else(|_| "unknown".to_string())
    }

    fn current_groups(&self) -> Vec<String> {
        fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|status| {
                status
                    .lines()
                    .find_map(|line| line.strip_prefix("Groups:"))
                    .map(|groups| {
                        groups
                            .split_whitespace()
                            .map(str::to_string)
                            .collect::<Vec<_>>()
                    })
            })
            .unwrap_or_default()
    }

    fn read_access(&self, path: &Path) -> InputPathStatus {
        match OpenOptions::new().read(true).open(path) {
            Ok(_) => InputPathStatus::Accessible,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                InputPathStatus::Missing(error.to_string())
            }
            Err(error) => InputPathStatus::Denied(error.to_string()),
        }
    }

    fn read_write_access(&self, path: &Path) -> InputPathStatus {
        match OpenOptions::new().read(true).write(true).open(path) {
            Ok(_) => InputPathStatus::Accessible,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                InputPathStatus::Missing(error.to_string())
            }
            Err(error) => InputPathStatus::Denied(error.to_string()),
        }
    }

    fn symlink_target(&self, path: &Path) -> Option<PathBuf> {
        fs::read_link(path).ok()
    }

    fn stable_path_for(&self, path: &Path) -> Option<PathBuf> {
        stable_signal_auras_path_for(path)
    }

    fn device_name(&self, path: &Path) -> Option<String> {
        signal_auras_wayland::evdev::evdev_device_name(path)
    }
}

pub fn input_doctor_report(lua_file: &Path) -> Result<InputDoctorReport, DiagnosableError> {
    input_doctor_report_with_probe(lua_file, &RealInputPermissionProbe)
}

fn input_doctor_report_with_probe(
    lua_file: &Path,
    probe: &impl InputPermissionProbe,
) -> Result<InputDoctorReport, DiagnosableError> {
    let config = load_lua_file(lua_file)?;
    let mut ok = true;
    let mut lines = vec![
        "# Signal Auras input doctor".to_string(),
        format!("user={}", probe.current_user()),
        format!("groups={}", probe.current_groups().join(",")),
    ];

    let Some(provider) = config.input_provider.as_ref() else {
        lines.push("input_provider=none".to_string());
        lines.push("result=ok no unsafe evdev/uinput permissions required by script".to_string());
        return Ok(InputDoctorReport { ok, lines });
    };

    lines.push(format!(
        "input_provider backend=evdev mode={:?} output={:?}",
        provider.mode, provider.output
    ));

    if provider.all_devices {
        ok = false;
        lines.push(
            "warning=devices_all selected-device permissions require explicit stable device paths"
                .to_string(),
        );
        lines.push("evdev=all status=not_checked".to_string());
    } else {
        let mut seen = BTreeSet::new();
        for path in &provider.devices {
            let mut status = probe.read_access(path);
            if !seen.insert(path.clone()) {
                status = InputPathStatus::Duplicate;
            } else if let Some(name) = probe.device_name(path) {
                if signal_auras_wayland::evdev::is_signal_auras_virtual_device_name(&name) {
                    status = InputPathStatus::SelfGenerated(name);
                }
            }
            if status != InputPathStatus::Accessible {
                ok = false;
            }
            let target = probe
                .symlink_target(path)
                .map(|target| format!(" target={}", target.display()))
                .unwrap_or_default();
            let recommendation = stable_path_recommendation(path, probe);
            lines.push(format!(
                "evdev path={}{} {}{}",
                path.display(),
                target,
                render_input_path_status(&status),
                recommendation
            ));
        }
    }

    if provider.output == InputProviderOutput::Uinput {
        let path = Path::new("/dev/uinput");
        let status = probe.read_write_access(path);
        if status != InputPathStatus::Accessible {
            ok = false;
        }
        lines.push(format!(
            "uinput path=/dev/uinput {}",
            render_input_path_status(&status)
        ));
    } else {
        lines.push("uinput=not_required output=portal".to_string());
    }

    if ok {
        lines.push("result=ok unsafe input permissions are available".to_string());
    } else {
        lines.push(
            "remediation=enable programs.signal-auras.unsafeInput with selected device matches, rebuild NixOS, then start a new login session"
                .to_string(),
        );
        lines.push("result=failed unsafe input permissions are incomplete".to_string());
    }

    Ok(InputDoctorReport { ok, lines })
}

fn render_input_path_status(status: &InputPathStatus) -> String {
    match status {
        InputPathStatus::Accessible => "status=ok".to_string(),
        InputPathStatus::Missing(error) => format!("status=missing error={}", shell_token(error)),
        InputPathStatus::Denied(error) => format!("status=denied error={}", shell_token(error)),
        InputPathStatus::Duplicate => "status=duplicate".to_string(),
        InputPathStatus::SelfGenerated(name) => {
            format!(
                "status=self_generated excluded=true name={}",
                shell_token(name)
            )
        }
    }
}

fn stable_path_recommendation(path: &Path, probe: &impl InputPermissionProbe) -> String {
    if path.starts_with("/dev/input/by-signal-auras") {
        return " preferred=selected_stable_path".to_string();
    }
    probe
        .stable_path_for(path)
        .map_or_else(String::new, |stable| {
            format!(" recommendation=use_selected_path_{}", stable.display())
        })
}

fn stable_signal_auras_path_for(path: &Path) -> Option<PathBuf> {
    let directory = Path::new("/dev/input/by-signal-auras");
    let entries = fs::read_dir(directory).ok()?;
    let canonical_path = fs::canonicalize(path).ok();
    for entry in entries.filter_map(Result::ok) {
        let stable_path = entry.path();
        let Some(target) = fs::read_link(&stable_path).ok() else {
            continue;
        };
        let resolved = if target.is_absolute() {
            target
        } else {
            directory.join(target)
        };
        if resolved == path
            || canonical_path
                .as_ref()
                .is_some_and(|path| fs::canonicalize(&resolved).ok().as_ref() == Some(path))
        {
            return Some(stable_path);
        }
    }
    None
}

fn shell_token(value: &str) -> String {
    value.replace(' ', "_")
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
    init_runtime_logging(log);
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
    warn_for_observe_mode_mouse_button_repeats(
        log,
        config.input_provider.as_ref(),
        config.motions().values(),
    );

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
    let signal_fd = RuntimeSignalFd::shutdown()?;
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
    if required.contains(CapabilityKind::ActiveProcessMetadata) {
        log.debug("event=active_process_provider_start provider=kwin-script");
        adapter.ensure_active_process_provider()?;
        stats.record_kde_bridge_setup();
        log.debug("event=active_process_provider_ready provider=kwin-script");
    }

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
        match run_live_real_lifecycle(&bindings, &motions, adapter, &mut stats, log, signal_fd) {
            Ok(reason) => reason,
            Err(error) => {
                println!("{}", stats.render_summary(ShutdownReason::RuntimeError));
                adapter.cancel_pending()?;
                cleanup_after_error(adapter, ErrorPhase::Shutdown)?;
                return Err(error);
            }
        };

    println!("{}", stats.render_summary(shutdown_reason));
    adapter.cancel_pending()?;
    adapter.unregister_all()?;
    Ok(stats)
}

fn warn_for_observe_mode_mouse_button_repeats<'a>(
    log: RuntimeLog,
    input_provider: Option<&InputProviderConfig>,
    motions: impl IntoIterator<Item = &'a MotionDefinition>,
) {
    if observe_mode_mouse_button_repeat(input_provider, motions) {
        log.warn(
            "event=input_provider_warning reason=observe_mouse_repeat \
             recommendation=use_grab_mode_for_held_mouse_button_repeats",
        );
    }
}

fn observe_mode_mouse_button_repeat<'a>(
    input_provider: Option<&InputProviderConfig>,
    motions: impl IntoIterator<Item = &'a MotionDefinition>,
) -> bool {
    let Some(input_provider) = input_provider else {
        return false;
    };
    if input_provider.mode != InputProviderMode::Observe
        || input_provider.output != InputProviderOutput::Uinput
    {
        return false;
    }
    motions.into_iter().any(|motion| {
        motion.repeat.as_ref().is_some_and(|repeat| {
            repeat
                .while_held
                .tokens()
                .iter()
                .any(|token| matches!(token, MotionToken::MouseButton(_)))
        })
    })
}

pub trait RunnerLifecycle {
    fn next_event(&mut self) -> Result<RunnerEvent, DiagnosableError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerEvent {
    Hotkey(HotkeyId),
    Callback {
        hotkey: HotkeyId,
        received_at: Instant,
    },
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
            RunnerEvent::Callback {
                hotkey,
                received_at,
            } => {
                stats.record_callback_received();
                stats.record_callback_dispatched(received_at.elapsed().as_millis() as u64);
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
                } else {
                    stats.record_shortcut_ignored();
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
                stats.record_motion_input_event(0);
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
    mut signal_fd: RuntimeSignalFd,
) -> Result<ShutdownReason, DiagnosableError> {
    let timer_fd = RuntimeTimerFd::new()?;
    let mut macro_queue = LiveMacroQueue::default();
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
        stats.record_event_loop_wakeup();
        macro_queue.drive_ready(adapter, stats)?;
        drain_live_shortcut_callbacks(bindings, adapter, &mut macro_queue, stats, log)?;
        let wait_timeout =
            next_live_wait_timeout(&repeat_ticks, &last_repeat_ticks, &motion_runtime)
                .min(macro_queue.next_wait_timeout());
        timer_fd.arm_after(wait_timeout)?;
        let mut runtime_fds = vec![signal_fd.as_raw_fd(), timer_fd.as_raw_fd()];
        if let Some(fd) = adapter.callback_wake_fd() {
            runtime_fds.push(fd);
        }
        match adapter.wait_next_input_or_runtime_fd(wait_timeout, &runtime_fds)? {
            signal_auras_wayland::evdev::EvdevInputWaitOutcome::Input(event) => {
                handle_observed_input(
                    event,
                    motions,
                    adapter,
                    &mut macro_queue,
                    &mut motion_runtime,
                    stats,
                    log,
                )?;
            }
            signal_auras_wayland::evdev::EvdevInputWaitOutcome::RuntimeFd(fd)
                if fd == signal_fd.as_raw_fd() =>
            {
                if let Some(reason) = signal_fd.drain_shutdown_reason()? {
                    return Ok(reason);
                }
            }
            signal_auras_wayland::evdev::EvdevInputWaitOutcome::RuntimeFd(fd)
                if fd == timer_fd.as_raw_fd() =>
            {
                timer_fd.drain()?;
            }
            signal_auras_wayland::evdev::EvdevInputWaitOutcome::RuntimeFd(fd)
                if adapter.callback_wake_fd() == Some(fd) =>
            {
                adapter.drain_callback_wake_fd()?;
                drain_live_shortcut_callbacks(bindings, adapter, &mut macro_queue, stats, log)?;
            }
            signal_auras_wayland::evdev::EvdevInputWaitOutcome::RuntimeFd(_) => {}
            signal_auras_wayland::evdev::EvdevInputWaitOutcome::Timeout => {}
        }
        while let Some(event) = adapter.next_input_event()? {
            handle_observed_input(
                event,
                motions,
                adapter,
                &mut macro_queue,
                &mut motion_runtime,
                stats,
                log,
            )?;
        }
        macro_queue.drive_ready(adapter, stats)?;
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
                    schedule_live_motion_repeat_tick(
                        motion,
                        active_context,
                        &mut macro_queue,
                        stats,
                        log,
                    )?;
                }
                last_repeat_ticks.insert(trigger.clone(), now);
            }
        }
        macro_queue.drive_ready(adapter, stats)?;
    }
}

fn drain_live_shortcut_callbacks(
    bindings: &[HotkeyBinding],
    adapter: &mut RealWaylandAdapter,
    macro_queue: &mut LiveMacroQueue,
    stats: &mut RuntimeStats,
    log: RuntimeLog,
) -> Result<(), DiagnosableError> {
    let dropped = adapter.take_callback_dropped_count();
    if dropped > 0 {
        stats.record_callback_dropped(dropped);
        log.warn(format!(
            "event=callback_burst_limited disposition=dropped count={dropped}"
        ));
    }

    while let Some(event) = adapter.next_shortcut_event() {
        stats.record_callback_received();
        let dispatch_latency_ms = event.received_at.elapsed().as_millis() as u64;
        stats.record_callback_dispatched(dispatch_latency_ms);
        log.debug(format!(
            "event=callback_received hotkey={} dispatch_latency_ms={dispatch_latency_ms}",
            event.hotkey.as_str()
        ));
        if let Some(binding) = bindings
            .iter()
            .find(|binding| binding.trigger == BindingTrigger::Keyboard(event.hotkey.clone()))
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
            schedule_live_binding(binding, active_context, macro_queue, stats)?;
        } else {
            stats.record_shortcut_ignored();
            log.debug(format!(
                "event=callback_ignored hotkey={} reason=unregistered",
                event.hotkey.as_str()
            ));
        }
    }
    Ok(())
}

fn trigger_label_for_log(trigger: &MotionTrigger) -> String {
    trigger.describe().replace(' ', "/")
}

#[derive(Default)]
struct LiveMacroQueue {
    next_id: u64,
    active_triggers: BTreeSet<String>,
    repeat_skip_counts: BTreeMap<String, u64>,
    runs: Vec<LiveMacroRun>,
}

struct LiveMacroRun {
    trigger_label: String,
    state: MacroRunState,
}

impl LiveMacroQueue {
    fn schedule(
        &mut self,
        trigger_label: String,
        definition: &MacroDefinition,
        inter_action_delay_ms: u64,
        stats: &mut RuntimeStats,
    ) -> Option<MacroRunId> {
        if !self.active_triggers.insert(trigger_label.clone()) {
            record_non_repeat_collision_skip(stats);
            return None;
        }
        self.next_id += 1;
        let id = MacroRunId::new(self.next_id);
        self.runs.push(LiveMacroRun {
            trigger_label,
            state: MacroRunState::new(id, definition, inter_action_delay_ms, Instant::now()),
        });
        stats.record_output_queue_depth(self.runs.len() as u64);
        Some(id)
    }

    fn trigger_is_pending_or_active(&self, trigger_label: &str) -> bool {
        self.active_triggers.contains(trigger_label)
    }

    fn record_repeat_skip(&mut self, trigger_label: &str, stats: &mut RuntimeStats) -> u64 {
        stats.record_motion_repeat_skipped(1);
        let count = self
            .repeat_skip_counts
            .entry(trigger_label.to_string())
            .or_default();
        *count += 1;
        *count
    }

    fn cancel_repeat(&mut self, trigger_label: &str) -> usize {
        self.cancel_trigger(trigger_label)
    }

    fn cancel_trigger(&mut self, trigger_label: &str) -> usize {
        let mut cancelled = 0;
        for run in &mut self.runs {
            if run.trigger_label == trigger_label && !run.state.is_cancelled() {
                run.state.cancel();
                cancelled += 1;
            }
        }
        cancelled
    }

    fn next_wait_timeout(&self) -> Duration {
        let now = Instant::now();
        self.runs
            .iter()
            .filter_map(|run| run.state.next_deadline())
            .map(|deadline| deadline.saturating_duration_since(now))
            .min()
            .unwrap_or_else(idle_wait_timeout)
    }

    fn drive_ready(
        &mut self,
        executor: &mut impl MacroExecutor,
        stats: &mut RuntimeStats,
    ) -> Result<(), DiagnosableError> {
        let now = Instant::now();
        let mut index = 0;
        while index < self.runs.len() {
            match self.runs[index].state.poll(now) {
                MacroRunPoll::Ready(request) => {
                    match executor.execute_input_request(request)? {
                        signal_auras_core::InputEmission::Emitted => {
                            stats.record_synthesized_input_emitted();
                        }
                        signal_auras_core::InputEmission::Denied => {
                            stats.record_synthesized_input_denied();
                            return Err(DiagnosableError::new(
                                ErrorPhase::MacroExecution,
                                "synthesized input was denied",
                            ));
                        }
                        signal_auras_core::InputEmission::Failed => {
                            return Err(DiagnosableError::new(
                                ErrorPhase::MacroExecution,
                                "synthesized input failed",
                            ));
                        }
                        signal_auras_core::InputEmission::Cancelled => {
                            return Err(DiagnosableError::new(
                                ErrorPhase::Shutdown,
                                "synthesized input was cancelled",
                            ));
                        }
                    }
                    index += 1;
                }
                MacroRunPoll::Pending(_) => index += 1,
                MacroRunPoll::Complete => {
                    let run = self.runs.remove(index);
                    self.active_triggers.remove(&run.trigger_label);
                    stats.macro_success_count += 1;
                }
                MacroRunPoll::Cancelled => {
                    let run = self.runs.remove(index);
                    self.active_triggers.remove(&run.trigger_label);
                }
            }
        }
        Ok(())
    }
}

fn next_live_wait_timeout(
    repeat_ticks: &[(MotionTrigger, u64)],
    last_repeat_ticks: &std::collections::BTreeMap<MotionTrigger, Instant>,
    motion_runtime: &MotionRuntime,
) -> Duration {
    let mut timeout = idle_wait_timeout();
    let now = Instant::now();
    for (trigger, interval_ms) in repeat_ticks {
        if !motion_runtime.repeat_is_active(trigger) {
            continue;
        }
        let due_in = match last_repeat_ticks.get(trigger) {
            Some(last_tick) => {
                let interval = Duration::from_millis(*interval_ms);
                interval.saturating_sub(now.saturating_duration_since(*last_tick))
            }
            None => Duration::ZERO,
        };
        timeout = timeout.min(due_in);
    }
    timeout
}

fn idle_wait_timeout() -> Duration {
    Duration::from_secs(300)
}

fn handle_observed_motion_input(
    observed: signal_auras_wayland::evdev::ObservedMotionInputEvent,
    motions: &[RuntimeMotion],
    adapter: &mut RealWaylandAdapter,
    macro_queue: &mut LiveMacroQueue,
    motion_runtime: &mut MotionRuntime,
    stats: &mut RuntimeStats,
    log: RuntimeLog,
) -> Result<bool, DiagnosableError> {
    let latency_ms = observed.observed_at.elapsed().as_millis() as u64;
    stats.record_motion_input_event(latency_ms);
    log.debug(format!(
        "event=motion_input token={} state={} source={} dispatch_latency_ms={latency_ms}",
        observed.event.token.describe(),
        motion_input_state_label(observed.event.state),
        observed.source.display()
    ));
    let mut consumed = false;
    for event in motion_runtime.handle_input(observed.event) {
        consumed = true;
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
        schedule_live_motion_runtime_event(event, motions, active_context, macro_queue, stats)?;
    }
    Ok(consumed)
}

fn handle_observed_input(
    observed: signal_auras_wayland::evdev::ObservedInputEvent,
    motions: &[RuntimeMotion],
    adapter: &mut RealWaylandAdapter,
    macro_queue: &mut LiveMacroQueue,
    motion_runtime: &mut MotionRuntime,
    stats: &mut RuntimeStats,
    log: RuntimeLog,
) -> Result<(), DiagnosableError> {
    let Some(event) = observed.event.clone() else {
        if observed.grabbed {
            adapter.passthrough_raw_input(&observed.raw)?;
        }
        return Ok(());
    };
    if event.token == MotionToken::Leader && event.state == MotionInputState::Pressed {
        adapter.arm_input_grab()?;
    }
    let consumed = handle_observed_motion_input(
        signal_auras_wayland::evdev::ObservedMotionInputEvent {
            event: event.clone(),
            source: observed.source.clone(),
            observed_at: observed.observed_at,
        },
        motions,
        adapter,
        macro_queue,
        motion_runtime,
        stats,
        log,
    )?;
    if observed.grabbed && !consumed {
        adapter.passthrough_raw_input(&observed.raw)?;
    }
    if event.token == MotionToken::Leader && event.state == MotionInputState::Released {
        adapter.release_input_grab()?;
    }
    Ok(())
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

fn schedule_live_binding(
    binding: &HotkeyBinding,
    active_context: signal_auras_core::ActiveProcessContext,
    macro_queue: &mut LiveMacroQueue,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError> {
    let trigger_label = binding.trigger_label();
    stats.record_trigger(&trigger_label);
    match binding.mode {
        BindingMode::Consume => stats.record_consumed_trigger_event(),
        BindingMode::Passthrough => stats.record_passthrough_trigger_event(),
    }
    match binding.scope.decide_context(&active_context) {
        ScopeDecision::Allowed => {
            stats.record_active_process_match();
            macro_queue.schedule(trigger_label, &binding.macro_definition, 0, stats);
        }
        ScopeDecision::Denied { reason, diagnostic } => {
            record_scope_denial(stats, &diagnostic);
            println!(
                "denied_trigger hotkey={} reason={reason} {}",
                trigger_label,
                diagnostic.render_fields()
            );
        }
    }
    Ok(())
}

fn schedule_live_motion_runtime_event(
    event: MotionRuntimeEvent,
    motions: &[RuntimeMotion],
    active_context: signal_auras_core::ActiveProcessContext,
    macro_queue: &mut LiveMacroQueue,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError> {
    match event {
        MotionRuntimeEvent::Triggered { trigger, .. } => {
            let Some(motion) = motions
                .iter()
                .find(|motion| motion.definition.trigger == trigger)
            else {
                return Ok(());
            };
            schedule_live_motion_trigger(motion, active_context, macro_queue, stats)
        }
        MotionRuntimeEvent::RepeatCancelled { trigger } => {
            stats.record_motion_repeat_cancel();
            let trigger_label = format!("{} repeat", trigger.describe());
            let cancelled = macro_queue.cancel_repeat(&trigger_label);
            stats.record_cancelled_macro_runs(cancelled as u64);
            println!(
                "motion_repeat_cancelled trigger={} queued_macros_cancelled={cancelled}",
                trigger.describe()
            );
            Ok(())
        }
    }
}

fn schedule_live_motion_trigger(
    motion: &RuntimeMotion,
    active_context: signal_auras_core::ActiveProcessContext,
    macro_queue: &mut LiveMacroQueue,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError> {
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
                macro_queue.schedule(
                    trigger_label,
                    macro_definition,
                    motion.definition.inter_action_delay_ms,
                    stats,
                );
            }
        }
        ScopeDecision::Denied { reason, diagnostic } => {
            record_scope_denial(stats, &diagnostic);
            println!(
                "denied_motion trigger={} reason={reason} {}",
                trigger_label,
                diagnostic.render_fields()
            );
        }
    }
    Ok(())
}

fn schedule_live_motion_repeat_tick(
    motion: &RuntimeMotion,
    active_context: signal_auras_core::ActiveProcessContext,
    macro_queue: &mut LiveMacroQueue,
    stats: &mut RuntimeStats,
    log: RuntimeLog,
) -> Result<(), DiagnosableError> {
    let Some(repeat) = &motion.definition.repeat else {
        return Ok(());
    };
    let trigger_label = format!("{} repeat", motion.definition.trigger.describe());
    if macro_queue.trigger_is_pending_or_active(&trigger_label) {
        let skipped_for_binding = macro_queue.record_repeat_skip(&trigger_label, stats);
        if should_log_repeat_overload_skip(skipped_for_binding) {
            log.debug(repeat_overload_log_message(
                &trigger_label,
                skipped_for_binding,
            ));
        }
        return Ok(());
    }
    match motion.scope.decide_context(&active_context) {
        ScopeDecision::Allowed => {
            stats.record_active_process_match();
            stats.record_motion_repeat_tick();
            log.debug(format!(
                "event=motion_repeat_tick_scheduled trigger={} disposition=executed",
                trigger_label_for_log(&motion.definition.trigger)
            ));
            macro_queue.schedule(
                trigger_label,
                &repeat.macro_definition,
                motion.definition.inter_action_delay_ms,
                stats,
            );
        }
        ScopeDecision::Denied { reason, diagnostic } => {
            record_scope_denial(stats, &diagnostic);
            println!(
                "denied_motion_repeat trigger={} reason={reason} {}",
                trigger_label,
                diagnostic.render_fields()
            );
        }
    }
    Ok(())
}

fn should_log_repeat_overload_skip(skipped_for_binding: u64) -> bool {
    skipped_for_binding == 1 || skipped_for_binding.is_power_of_two()
}

fn repeat_overload_log_message(trigger_label: &str, skipped_for_binding: u64) -> String {
    format!(
        "event=motion_repeat_overload trigger={} disposition=skipped_or_coalesced reason=output_pending skipped_for_binding={skipped_for_binding}",
        trigger_label.replace(' ', "/")
    )
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
                Err(_) => {
                    record_non_repeat_collision_skip(stats);
                    return Ok(());
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
        ScopeDecision::Denied { reason, diagnostic } => {
            record_scope_denial(stats, &diagnostic);
            println!(
                "denied_trigger hotkey={} reason={reason} {}",
                trigger_label,
                diagnostic.render_fields()
            );
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
            stats.record_motion_repeat_cancel();
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
    handle_motion_trigger_with_context(motion, active_context, executor, scheduler, stats)
}

fn handle_motion_trigger_with_context<E>(
    motion: &RuntimeMotion,
    active_context: signal_auras_core::ActiveProcessContext,
    executor: &mut E,
    scheduler: &mut MacroScheduler,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError>
where
    E: MacroExecutor,
{
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
        ScopeDecision::Denied { reason, diagnostic } => {
            record_scope_denial(stats, &diagnostic);
            println!(
                "denied_motion trigger={} reason={reason} {}",
                trigger_label,
                diagnostic.render_fields()
            );
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
    if motion.definition.repeat.is_none() {
        return Ok(());
    }
    let active_context = active_process_provider.active_process_context()?;
    handle_motion_repeat_tick_with_context(motion, active_context, executor, scheduler, stats)
}

fn handle_motion_repeat_tick_with_context<E>(
    motion: &RuntimeMotion,
    active_context: signal_auras_core::ActiveProcessContext,
    executor: &mut E,
    scheduler: &mut MacroScheduler,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError>
where
    E: MacroExecutor,
{
    let Some(repeat) = &motion.definition.repeat else {
        return Ok(());
    };
    let trigger_label = format!("{} repeat", motion.definition.trigger.describe());
    match motion.scope.decide_context(&active_context) {
        ScopeDecision::Allowed => {
            stats.record_active_process_match();
            stats.record_motion_repeat_tick();
            execute_motion_macro(
                &trigger_label,
                &repeat.macro_definition,
                motion.definition.inter_action_delay_ms,
                executor,
                scheduler,
                stats,
            )?;
        }
        ScopeDecision::Denied { reason, diagnostic } => {
            record_scope_denial(stats, &diagnostic);
            println!(
                "denied_motion_repeat trigger={} reason={reason} {}",
                trigger_label,
                diagnostic.render_fields()
            );
        }
    }
    Ok(())
}

fn record_scope_denial(stats: &mut RuntimeStats, diagnostic: &signal_auras_core::ScopeDenial) {
    stats.denied_action_count += 1;
    stats.scope_mismatch_count += 1;
    stats.record_active_process_non_match();
    if diagnostic.counts_as_metadata_unavailable() {
        stats.record_metadata_unavailable();
    }
}

fn record_non_repeat_collision_skip(stats: &mut RuntimeStats) {
    stats.denied_action_count += 1;
    stats.record_non_repeat_trigger_skipped();
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
        Err(_) => {
            record_non_repeat_collision_skip(stats);
            return Ok(());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_log_rendering_does_not_embed_ansi_escapes() {
        let rendered = RuntimeLog::with_color_enabled(true, ColorMode::Never, false)
            .render_plain("DEBUG", "event=motion_input token=<LClick>");

        assert!(!rendered.contains('\x1b'));
        assert!(rendered.contains("DEBUG"));
        assert!(rendered.contains("motion_input"));
        assert!(rendered.contains("token=<LClick>"));
    }

    #[test]
    fn runtime_log_color_mode_controls_subscriber_ansi() {
        let log = RuntimeLog::with_color_enabled(true, ColorMode::Always, true);

        assert_eq!(log.color_mode, ColorMode::Always);
        assert!(log.color);
    }

    #[test]
    fn warns_for_observe_mode_uinput_mouse_button_repeat() {
        let provider = InputProviderConfig::evdev(
            vec![PathBuf::from("/dev/input/event0")],
            InputProviderMode::Observe,
            InputProviderOutput::Uinput,
        )
        .unwrap();
        let motions = vec![mouse_button_repeat_motion()];

        assert!(observe_mode_mouse_button_repeat(Some(&provider), &motions));
    }

    #[test]
    fn parses_color_mode_options() {
        let args = vec![
            "run".to_string(),
            "--verbose".to_string(),
            "--color=always".to_string(),
            "examples/poe2-hideout.lua".to_string(),
        ];
        let options = parse_run_args(&args).unwrap();

        assert!(options.log.verbose);
        assert_eq!(options.log.color_mode, ColorMode::Always);
        assert!(options.log.color);
    }

    #[test]
    fn parses_input_doctor_command() {
        let args = vec![
            "doctor".to_string(),
            "input".to_string(),
            "examples/poe2-hideout.lua".to_string(),
        ];
        let options = parse_doctor_args(&args).unwrap();

        assert_eq!(options.lua_file, PathBuf::from("examples/poe2-hideout.lua"));
    }

    #[test]
    fn input_doctor_reports_selected_devices_and_uinput_access() {
        let lua_file = write_doctor_lua(
            r#"
            return {
              input_provider = {
                backend = "evdev",
                mode = "grab",
                output = "uinput",
                devices = { "/dev/input/by-signal-auras/mouse" },
              },
              leader = "F13",
              motions = {
                {
                  trigger = { "<Leader>", "<LClick>" },
                  mode = "passthrough",
                  macro = macro { mouse_click "left" },
                },
              },
            }
            "#,
        );
        let mut probe = FakeInputProbe::default();
        probe.read.insert(
            PathBuf::from("/dev/input/by-signal-auras/mouse"),
            InputPathStatus::Accessible,
        );
        probe.read_write.insert(
            PathBuf::from("/dev/uinput"),
            InputPathStatus::Denied("permission denied".to_string()),
        );
        probe.targets.insert(
            PathBuf::from("/dev/input/by-signal-auras/mouse"),
            PathBuf::from("../event12"),
        );

        let report = input_doctor_report_with_probe(&lua_file, &probe).unwrap();
        let rendered = report.render();

        assert!(!report.ok);
        assert!(
            rendered.contains("path=/dev/input/by-signal-auras/mouse target=../event12 status=ok")
        );
        assert!(rendered.contains("uinput path=/dev/uinput status=denied"));
        assert!(rendered.contains("programs.signal-auras.unsafeInput"));
    }

    #[test]
    fn input_doctor_recommends_stable_selected_paths() {
        let lua_file = write_doctor_lua(
            r#"
            return {
              input_provider = {
                backend = "evdev",
                mode = "observe",
                devices = { "/dev/input/event9" },
              },
              motions = {
                {
                  trigger = { "f" },
                  macro = macro { text "x" },
                },
              },
            }
            "#,
        );
        let mut probe = FakeInputProbe::default();
        probe.read.insert(
            PathBuf::from("/dev/input/event9"),
            InputPathStatus::Accessible,
        );
        probe.stable_paths.insert(
            PathBuf::from("/dev/input/event9"),
            PathBuf::from("/dev/input/by-signal-auras/main-keyboard"),
        );

        let report = input_doctor_report_with_probe(&lua_file, &probe).unwrap();
        let rendered = report.render();

        assert!(report.ok);
        assert!(rendered
            .contains("recommendation=use_selected_path_/dev/input/by-signal-auras/main-keyboard"));
    }

    #[test]
    fn input_doctor_reports_duplicate_and_own_virtual_selected_devices() {
        let lua_file = write_doctor_lua(
            r#"
            return {
              input_provider = {
                backend = "evdev",
                mode = "observe",
                devices = {
                  "/dev/input/by-signal-auras/mouse",
                  "/dev/input/by-signal-auras/mouse",
                  "/dev/input/event42",
                },
              },
              motions = {
                {
                  trigger = { "f" },
                  macro = macro { text "x" },
                },
              },
            }
            "#,
        );
        let mut probe = FakeInputProbe::default();
        probe.read.insert(
            PathBuf::from("/dev/input/by-signal-auras/mouse"),
            InputPathStatus::Accessible,
        );
        probe.read.insert(
            PathBuf::from("/dev/input/event42"),
            InputPathStatus::Accessible,
        );
        probe.device_names.insert(
            PathBuf::from("/dev/input/event42"),
            signal_auras_wayland::evdev::SIGNAL_AURAS_UINPUT_DEVICE_NAME.to_string(),
        );

        let report = input_doctor_report_with_probe(&lua_file, &probe).unwrap();
        let rendered = report.render();

        assert!(!report.ok);
        assert!(rendered.contains("status=duplicate"));
        assert!(rendered.contains("status=self_generated"));
        assert!(rendered.contains("excluded=true"));
    }

    #[test]
    fn input_doctor_warns_when_all_devices_conflicts_with_selected_permissions() {
        let lua_file = write_doctor_lua(
            r#"
            return {
              input_provider = {
                backend = "evdev",
                mode = "observe",
                devices = "all",
              },
              motions = {
                {
                  trigger = { "f" },
                  macro = macro { text "x" },
                },
              },
            }
            "#,
        );

        let report = input_doctor_report_with_probe(&lua_file, &FakeInputProbe::default()).unwrap();
        let rendered = report.render();

        assert!(!report.ok);
        assert!(rendered.contains("warning=devices_all"));
        assert!(rendered.contains("evdev=all status=not_checked"));
    }

    #[test]
    fn does_not_warn_for_grab_mode_mouse_button_repeat() {
        let provider = InputProviderConfig::evdev(
            vec![PathBuf::from("/dev/input/event0")],
            InputProviderMode::Grab,
            InputProviderOutput::Uinput,
        )
        .unwrap();
        let motions = vec![mouse_button_repeat_motion()];

        assert!(!observe_mode_mouse_button_repeat(Some(&provider), &motions));
    }

    #[test]
    fn idle_wait_timeout_is_long_when_no_runtime_work_is_pending() {
        let motion_runtime = MotionRuntime::new(std::iter::empty::<MotionDefinition>());
        let repeat_timeout =
            next_live_wait_timeout(&[], &std::collections::BTreeMap::new(), &motion_runtime);
        let macro_timeout = LiveMacroQueue::default().next_wait_timeout();

        assert_eq!(repeat_timeout, Duration::from_secs(300));
        assert_eq!(macro_timeout, Duration::from_secs(300));
    }

    #[test]
    fn overloaded_repeat_ticks_are_skipped_without_queue_growth() {
        let motion = repeat_runtime_motion(MotionTrigger::parse(["<Leader>", "a"]).unwrap(), "x");
        let active_context = signal_auras_core::ActiveProcessContext::unavailable("not needed");
        let mut macro_queue = LiveMacroQueue::default();
        let mut stats = RuntimeStats::new();

        for _ in 0..=10_000 {
            schedule_live_motion_repeat_tick(
                &motion,
                active_context.clone(),
                &mut macro_queue,
                &mut stats,
                RuntimeLog::new(false),
            )
            .unwrap();
        }

        assert_eq!(macro_queue.runs.len(), 1);
        assert_eq!(stats.max_output_queue_depth, 1);
        assert_eq!(stats.motion_repeat_tick_count, 1);
        assert_eq!(stats.motion_repeat_skipped_count, 10_000);
        assert_eq!(stats.denied_action_count, 0);
        assert_eq!(stats.macro_failure_count, 0);
    }

    #[test]
    fn non_repeat_trigger_collision_is_skipped_without_queue_error() {
        let binding = runtime_hotkey_binding("F5", "x");
        let active_context = signal_auras_core::ActiveProcessContext::unavailable("not needed");
        let mut macro_queue = LiveMacroQueue::default();
        let mut stats = RuntimeStats::new();

        schedule_live_binding(
            &binding,
            active_context.clone(),
            &mut macro_queue,
            &mut stats,
        )
        .unwrap();
        schedule_live_binding(&binding, active_context, &mut macro_queue, &mut stats).unwrap();

        assert_eq!(macro_queue.runs.len(), 1);
        assert_eq!(stats.max_output_queue_depth, 1);
        assert_eq!(stats.denied_action_count, 1);
        assert_eq!(stats.non_repeat_trigger_skipped_count, 1);
        assert_eq!(stats.macro_failure_count, 0);
    }

    #[test]
    fn non_repeat_trigger_state_clears_after_completion_or_cancellation() {
        let binding = runtime_hotkey_binding("F5", "x");
        let trigger_label = binding.trigger_label();
        let active_context = signal_auras_core::ActiveProcessContext::unavailable("not needed");
        let mut macro_queue = LiveMacroQueue::default();
        let mut stats = RuntimeStats::new();
        let mut executor = QueueExecutor::default();

        schedule_live_binding(
            &binding,
            active_context.clone(),
            &mut macro_queue,
            &mut stats,
        )
        .unwrap();
        assert!(macro_queue.trigger_is_pending_or_active(&trigger_label));
        macro_queue.drive_ready(&mut executor, &mut stats).unwrap();
        macro_queue.drive_ready(&mut executor, &mut stats).unwrap();
        assert!(!macro_queue.trigger_is_pending_or_active(&trigger_label));

        schedule_live_binding(
            &binding,
            active_context.clone(),
            &mut macro_queue,
            &mut stats,
        )
        .unwrap();
        let cancelled = macro_queue.cancel_trigger(&trigger_label);
        macro_queue.drive_ready(&mut executor, &mut stats).unwrap();
        assert_eq!(cancelled, 1);
        assert!(!macro_queue.trigger_is_pending_or_active(&trigger_label));

        schedule_live_binding(&binding, active_context, &mut macro_queue, &mut stats).unwrap();
        assert!(macro_queue.trigger_is_pending_or_active(&trigger_label));
        assert_eq!(stats.non_repeat_trigger_skipped_count, 0);
    }

    #[test]
    fn repeat_overload_accounting_is_isolated_by_binding() {
        let first = repeat_runtime_motion(MotionTrigger::parse(["<Leader>", "a"]).unwrap(), "a");
        let second = repeat_runtime_motion(MotionTrigger::parse(["<Leader>", "b"]).unwrap(), "b");
        let active_context = signal_auras_core::ActiveProcessContext::unavailable("not needed");
        let mut macro_queue = LiveMacroQueue::default();
        let mut stats = RuntimeStats::new();

        schedule_live_motion_repeat_tick(
            &first,
            active_context.clone(),
            &mut macro_queue,
            &mut stats,
            RuntimeLog::new(false),
        )
        .unwrap();
        schedule_live_motion_repeat_tick(
            &first,
            active_context.clone(),
            &mut macro_queue,
            &mut stats,
            RuntimeLog::new(false),
        )
        .unwrap();
        schedule_live_motion_repeat_tick(
            &second,
            active_context,
            &mut macro_queue,
            &mut stats,
            RuntimeLog::new(false),
        )
        .unwrap();

        assert_eq!(macro_queue.runs.len(), 2);
        assert_eq!(stats.motion_repeat_tick_count, 2);
        assert_eq!(stats.motion_repeat_skipped_count, 1);
        let first_label = format!("{} repeat", first.definition.trigger.describe());
        assert_eq!(macro_queue.repeat_skip_counts.get(&first_label), Some(&1));
        let second_label = format!("{} repeat", second.definition.trigger.describe());
        assert!(!macro_queue.repeat_skip_counts.contains_key(&second_label));
    }

    #[test]
    fn cancellation_targets_only_the_released_repeat_binding() {
        let first = repeat_runtime_motion(MotionTrigger::parse(["<Leader>", "a"]).unwrap(), "a");
        let second = repeat_runtime_motion(MotionTrigger::parse(["<Leader>", "b"]).unwrap(), "b");
        let active_context = signal_auras_core::ActiveProcessContext::unavailable("not needed");
        let mut macro_queue = LiveMacroQueue::default();
        let mut stats = RuntimeStats::new();

        schedule_live_motion_repeat_tick(
            &first,
            active_context.clone(),
            &mut macro_queue,
            &mut stats,
            RuntimeLog::new(false),
        )
        .unwrap();
        schedule_live_motion_repeat_tick(
            &second,
            active_context,
            &mut macro_queue,
            &mut stats,
            RuntimeLog::new(false),
        )
        .unwrap();

        let first_label = format!("{} repeat", first.definition.trigger.describe());
        let second_label = format!("{} repeat", second.definition.trigger.describe());
        let cancelled = macro_queue.cancel_repeat(&first_label);
        stats.record_cancelled_macro_runs(cancelled as u64);
        macro_queue
            .drive_ready(&mut QueueExecutor::default(), &mut stats)
            .unwrap();

        assert_eq!(cancelled, 1);
        assert_eq!(stats.cancelled_macro_run_count, 1);
        assert!(!macro_queue.trigger_is_pending_or_active(&first_label));
        assert!(macro_queue.trigger_is_pending_or_active(&second_label));
    }

    #[test]
    fn repeat_overload_log_message_is_rate_limited_and_payload_safe() {
        assert!(should_log_repeat_overload_skip(1));
        assert!(should_log_repeat_overload_skip(2));
        assert!(!should_log_repeat_overload_skip(3));
        assert!(should_log_repeat_overload_skip(4));

        let rendered = RuntimeLog::new(true).render_plain(
            "DEBUG",
            &repeat_overload_log_message("<Leader> then a repeat", 8),
        );

        assert!(rendered.contains("motion_repeat_overload"));
        assert!(rendered.contains("trigger=<Leader>/then/a/repeat"));
        assert!(rendered.contains("skipped_for_binding=8"));
        assert!(!rendered.contains("secret macro payload"));
    }

    fn mouse_button_repeat_motion() -> MotionDefinition {
        let macro_definition =
            MacroDefinition::new(vec![signal_auras_core::MacroAction::mouse_click(
                signal_auras_core::MouseButton::Left,
            )])
            .unwrap();
        let repeat = signal_auras_core::RepeatDefinition::new(
            MotionTrigger::parse(["<LClick>"]).unwrap(),
            signal_auras_core::RepeatInterval::new(50, 80).unwrap(),
            macro_definition,
        );
        MotionDefinition::new(
            MotionTrigger::parse(["<Leader>", "<LClick>", "<LClick>"]).unwrap(),
            BindingMode::Passthrough,
            None,
            Some(repeat),
            0,
        )
        .unwrap()
    }

    fn repeat_runtime_motion(trigger: MotionTrigger, text: &str) -> RuntimeMotion {
        let macro_definition =
            MacroDefinition::new(vec![signal_auras_core::MacroAction::text(text).unwrap()])
                .unwrap();
        let repeat = signal_auras_core::RepeatDefinition::new(
            trigger.clone(),
            signal_auras_core::RepeatInterval::new(10, 10).unwrap(),
            macro_definition,
        );
        RuntimeMotion {
            definition: MotionDefinition::new(
                trigger,
                BindingMode::Passthrough,
                None,
                Some(repeat),
                0,
            )
            .unwrap(),
            scope: ScopeSelection::explicit_global_from_prompt(true).unwrap(),
        }
    }

    fn runtime_hotkey_binding(hotkey: &str, text: &str) -> HotkeyBinding {
        HotkeyBinding {
            trigger: BindingTrigger::keyboard(HotkeyId::parse(hotkey).unwrap()),
            mode: BindingMode::Consume,
            scope: ScopeSelection::ExplicitGlobal,
            macro_definition: MacroDefinition::new(vec![signal_auras_core::MacroAction::text(
                text,
            )
            .unwrap()])
            .unwrap(),
            registration_state: signal_auras_core::RegistrationState::Registered,
        }
    }

    #[derive(Default)]
    struct QueueExecutor {
        actions: usize,
    }

    impl MacroExecutor for QueueExecutor {
        fn execute_action(
            &mut self,
            _action: &signal_auras_core::MacroAction,
        ) -> Result<(), DiagnosableError> {
            self.actions += 1;
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeInputProbe {
        read: std::collections::BTreeMap<PathBuf, InputPathStatus>,
        read_write: std::collections::BTreeMap<PathBuf, InputPathStatus>,
        targets: std::collections::BTreeMap<PathBuf, PathBuf>,
        stable_paths: std::collections::BTreeMap<PathBuf, PathBuf>,
        device_names: std::collections::BTreeMap<PathBuf, String>,
    }

    impl InputPermissionProbe for FakeInputProbe {
        fn current_user(&self) -> String {
            "alice".to_string()
        }

        fn current_groups(&self) -> Vec<String> {
            vec!["100".to_string(), "200".to_string()]
        }

        fn read_access(&self, path: &Path) -> InputPathStatus {
            self.read.get(path).cloned().unwrap_or_else(|| {
                InputPathStatus::Missing("No such file or directory".to_string())
            })
        }

        fn read_write_access(&self, path: &Path) -> InputPathStatus {
            self.read_write.get(path).cloned().unwrap_or_else(|| {
                InputPathStatus::Missing("No such file or directory".to_string())
            })
        }

        fn symlink_target(&self, path: &Path) -> Option<PathBuf> {
            self.targets.get(path).cloned()
        }

        fn stable_path_for(&self, path: &Path) -> Option<PathBuf> {
            self.stable_paths.get(path).cloned()
        }

        fn device_name(&self, path: &Path) -> Option<String> {
            self.device_names.get(path).cloned()
        }
    }

    fn write_doctor_lua(source: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("signal-auras-doctor-{}.lua", unique));
        std::fs::write(&path, source).unwrap();
        path
    }
}
