pub use crate::logging::{ColorMode, RuntimeLog, RuntimeLogGuard};
use crate::{
    logging::{init_runtime_logging, RuntimeLogConfig, RuntimeLogFormat, RuntimeLogLevel},
    prompt::{stdin_is_interactive, ScopePrompt, TerminalPrompt},
};
use signal_auras_core::{
    execute_plan_with_inter_action_delay, queue_controller_callback_outputs, ActiveProcessProvider,
    BindingMode, BindingTrigger, CallbackDisposition, CapabilityKind, CapabilityReport,
    CapabilitySet, ControllerProgram, ControllerRegistration, ControllerRegistrationKind,
    DiagnosableError, ErrorPhase, FocusFreshnessPolicy, HotkeyBinding, HotkeyId, HotkeyRegistrar,
    InputProviderConfig, InputProviderMode, InputProviderOutput, KeyToken, LoopBody,
    LoopDefinition, LoopInterval, LoopRepeat, LuaCallbackScheduler, MacroAction, MacroDefinition,
    MacroExecutor, MacroRunId, MacroRunPoll, MacroRunState, MacroScheduler, MotionDefinition,
    MotionDiscardReason, MotionInputEvent, MotionInputState, MotionRuntime, MotionRuntimeEvent,
    MotionToken, MotionTrigger, OverlayProviderReport, RegistrationState, RuntimeMotion,
    RuntimePress, RuntimeStats, RustOperationBatch, ScopeSelection, ShutdownReason,
    StateTrackerPoller, SynthesizedInputRequest, TrackerState,
};
use signal_auras_lua::{
    load_lua_controller_program_file, load_lua_file, ActiveWindowMetadata, ImperativeLuaController,
    LuaCallbackStep, LuaHostRequest, LuaHostResponse, LuaLogLevel,
};
use signal_auras_wayland::{
    evdev::{EvdevInputWaitOutcome, EvdevObservationProvider, KernelEventTimestamp},
    event_loop::{RuntimeSignalFd, RuntimeTimerFd},
    RealWaylandAdapter,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

const MOTION_FOCUS_STALE_THRESHOLD: Duration = Duration::from_secs(30);

pub trait ControllerHost: MacroExecutor {
    fn sleep(&mut self, duration: Duration) -> Result<(), DiagnosableError> {
        thread::sleep(duration);
        Ok(())
    }

    fn active_window(
        &mut self,
        _include_title: bool,
    ) -> Result<ActiveWindowMetadata, DiagnosableError> {
        Err(DiagnosableError::new(
            ErrorPhase::MacroExecution,
            "active window metadata provider is unsupported",
        )
        .with_capability(signal_auras_core::Capability::ActiveWindowMetadata))
    }

    fn find_window(&mut self, _processes: &[String]) -> Result<Option<String>, DiagnosableError> {
        Err(DiagnosableError::new(
            ErrorPhase::MacroExecution,
            "window lookup provider is unsupported",
        )
        .with_capability(signal_auras_core::Capability::WindowActivation))
    }

    fn activate_window(&mut self, _handle: &str) -> Result<bool, DiagnosableError> {
        Err(DiagnosableError::new(
            ErrorPhase::MacroExecution,
            "window activation provider is unsupported",
        )
        .with_capability(signal_auras_core::Capability::WindowActivation))
    }

    fn wait_active_window(
        &mut self,
        handle: &str,
        timeout: Duration,
    ) -> Result<bool, DiagnosableError> {
        let started = Instant::now();
        loop {
            if self
                .active_window(false)
                .map(|metadata| metadata.title.as_deref() == Some(handle))
                .unwrap_or(false)
            {
                return Ok(true);
            }
            if started.elapsed() >= timeout {
                return Ok(false);
            }
            thread::sleep(Duration::from_millis(25));
        }
    }
}

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

impl ControllerHost for RealWaylandAdapter {
    fn active_window(
        &mut self,
        include_title: bool,
    ) -> Result<ActiveWindowMetadata, DiagnosableError> {
        Ok(ActiveWindowMetadata {
            title: if include_title {
                self.active_window_title()?
            } else {
                None
            },
        })
    }

    fn find_window(&mut self, processes: &[String]) -> Result<Option<String>, DiagnosableError> {
        self.find_window_by_processes(processes)
    }

    fn activate_window(&mut self, handle: &str) -> Result<bool, DiagnosableError> {
        RealWaylandAdapter::activate_window(self, handle)
    }

    fn wait_active_window(
        &mut self,
        handle: &str,
        timeout: Duration,
    ) -> Result<bool, DiagnosableError> {
        let started = Instant::now();
        loop {
            if self.active_window_matches(handle)? {
                return Ok(true);
            }
            if started.elapsed() >= timeout {
                return Ok(false);
            }
            thread::sleep(Duration::from_millis(25));
        }
    }
}

pub fn run_cli(
    args: impl IntoIterator<Item = String>,
    prompt: &mut impl ScopePrompt,
) -> Result<(), DiagnosableError> {
    let args = args.into_iter().collect::<Vec<_>>();
    if args.first().map(String::as_str) == Some("doctor") {
        let options = parse_doctor_args(&args)?;
        let failure_message = match options.command {
            DoctorCommand::Input => "input doctor found missing unsafe input permissions",
            DoctorCommand::Keys => "key doctor found missing unsafe input permissions",
        };
        let report = match options.command {
            DoctorCommand::Input => input_doctor_report(&options.lua_file)?,
            DoctorCommand::Keys => key_doctor_report(&options.lua_file)?,
        };
        println!("{}", report.render());
        if report.ok {
            return Ok(());
        }
        return Err(DiagnosableError::new(
            ErrorPhase::CapabilityProbe,
            failure_message,
        ));
    }
    let options = parse_run_args(&args)?;
    let _log_guard = init_runtime_logging(&options.log);
    let mut adapter = RealWaylandAdapter::new();
    if lua_file_looks_like_controller(&options.lua_file)? {
        start_live_real_controller_runner_with_options(&options.lua_file, &mut adapter, options.log)
            .map(|_| ())
    } else {
        start_live_real_runner_with_options(&options.lua_file, prompt, &mut adapter, options.log)
            .map(|_| ())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    pub lua_file: PathBuf,
    pub log: RuntimeLog,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorOptions {
    pub command: DoctorCommand,
    pub lua_file: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorCommand {
    Input,
    Keys,
}

pub fn parse_run_args(args: &[String]) -> Result<RunOptions, DiagnosableError> {
    if args.first().map(String::as_str) != Some("run") {
        return Err(DiagnosableError::new(
            ErrorPhase::ArgumentValidation,
            run_usage(),
        ));
    }
    let mut verbose = false;
    let mut config = RuntimeLogConfig::new(false);
    let mut paths = Vec::new();
    for arg in &args[1..] {
        match arg.as_str() {
            "--verbose" | "-v" => verbose = true,
            "--color=auto" => config.color_mode = ColorMode::Auto,
            "--color=always" => config.color_mode = ColorMode::Always,
            "--color=never" => config.color_mode = ColorMode::Never,
            "--no-color" => config.color_mode = ColorMode::Never,
            value if let Some(level) = value.strip_prefix("--log-level=") => {
                config.level = Some(level.parse::<RuntimeLogLevel>().map_err(|error| {
                    DiagnosableError::new(ErrorPhase::ArgumentValidation, error.to_string())
                })?);
            }
            value if let Some(format) = value.strip_prefix("--log-format=") => {
                config.format = format.parse::<RuntimeLogFormat>().map_err(|error| {
                    DiagnosableError::new(ErrorPhase::ArgumentValidation, error.to_string())
                })?;
            }
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
            run_usage(),
        ));
    }
    config.verbose = verbose;
    Ok(RunOptions {
        lua_file: paths.remove(0),
        log: RuntimeLog::from_config(config),
    })
}

fn run_usage() -> &'static str {
    "usage: signal-auras run [--verbose|-v] [--log-level=off|error|warn|info|debug|trace] [--log-format=auto|pretty|compact] [--color=auto|always|never] [--no-color] <lua-file>"
}

pub fn parse_doctor_args(args: &[String]) -> Result<DoctorOptions, DiagnosableError> {
    if args.first().map(String::as_str) != Some("doctor") || args.len() != 3 {
        return Err(DiagnosableError::new(
            ErrorPhase::ArgumentValidation,
            "usage: signal-auras doctor input <lua-file> | signal-auras doctor keys <lua-file>",
        ));
    }
    let command =
        match args.get(1).map(String::as_str) {
            Some("input") => DoctorCommand::Input,
            Some("keys") => DoctorCommand::Keys,
            _ => return Err(DiagnosableError::new(
                ErrorPhase::ArgumentValidation,
                "usage: signal-auras doctor input <lua-file> | signal-auras doctor keys <lua-file>",
            )),
        };
    Ok(DoctorOptions {
        command,
        lua_file: PathBuf::from(&args[2]),
    })
}

fn lua_file_looks_like_controller(lua_file: &Path) -> Result<bool, DiagnosableError> {
    let source = fs::read_to_string(lua_file).map_err(|error| {
        DiagnosableError::new(
            ErrorPhase::ScriptLoad,
            format!("cannot read Lua file '{}': {error}", lua_file.display()),
        )
    })?;
    Ok(source.contains("sa.hotkey")
        || source.contains("sa.motion")
        || source.contains("sa.press")
        || source.contains("sa.timer")
        || source.contains("sa.shutdown")
        || source.contains("sa.state.track")
        || source.contains("sa.callback"))
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

pub type KeyDoctorReport = InputDoctorReport;

const KEY_DOCTOR_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);
const KEY_DOCTOR_DISCOVERY_LIMIT: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
struct KeyDiscoveryObservation {
    device: String,
    raw_code: u16,
}

pub fn key_doctor_report(lua_file: &Path) -> Result<KeyDoctorReport, DiagnosableError> {
    let observations = collect_key_doctor_observations(lua_file)?;
    key_doctor_report_with_probe_and_observations(
        lua_file,
        &RealInputPermissionProbe,
        &observations,
    )
}

fn collect_key_doctor_observations(
    lua_file: &Path,
) -> Result<Vec<KeyDiscoveryObservation>, DiagnosableError> {
    let config = load_lua_file(lua_file)?;
    let Some(provider) = config.input_provider.as_ref() else {
        return Ok(Vec::new());
    };
    let devices = if provider.all_devices {
        signal_auras_wayland::evdev::discover_event_devices()?
    } else {
        provider.devices.clone()
    };
    let mut evdev = EvdevObservationProvider::open(
        devices,
        InputProviderMode::Observe,
        config.leader.clone(),
        provider.all_devices,
    )?;
    let deadline = Instant::now() + KEY_DOCTOR_DISCOVERY_TIMEOUT;
    let mut observations = Vec::new();
    while Instant::now() < deadline && observations.len() < KEY_DOCTOR_DISCOVERY_LIMIT {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = remaining.min(Duration::from_millis(250));
        match evdev.wait_next_observed_input_event_or_runtime_fd(timeout, &[])? {
            EvdevInputWaitOutcome::Input(event) => {
                if event.raw.event_type == 0x01 {
                    observations.push(KeyDiscoveryObservation {
                        device: event.source.display().to_string(),
                        raw_code: event.raw.code,
                    });
                }
            }
            EvdevInputWaitOutcome::RuntimeFd(_) | EvdevInputWaitOutcome::Timeout => {}
        }
    }
    Ok(observations)
}

fn key_doctor_report_with_probe_and_observations(
    lua_file: &Path,
    probe: &impl InputPermissionProbe,
    observations: &[KeyDiscoveryObservation],
) -> Result<KeyDoctorReport, DiagnosableError> {
    let config = load_lua_file(lua_file)?;
    let mut ok = true;
    let mut lines = vec![
        "# Signal Auras key doctor".to_string(),
        format!("user={}", probe.current_user()),
        "persistence=none".to_string(),
    ];

    let Some(provider) = config.input_provider.as_ref() else {
        lines.push("input_provider=none".to_string());
        lines.push(
            "result=failed key discovery requires an explicit evdev input_provider".to_string(),
        );
        return Ok(KeyDoctorReport { ok: false, lines });
    };

    lines.push(format!(
        "input_provider backend=evdev mode={:?} output={:?}",
        provider.mode, provider.output
    ));
    if provider.all_devices {
        lines.push("evdev=all status=explicit_current_run".to_string());
    } else {
        let mut seen = BTreeSet::new();
        for path in &provider.devices {
            let mut status = probe.read_access(path);
            if !seen.insert(path.clone()) {
                status = InputPathStatus::Duplicate;
            }
            if status != InputPathStatus::Accessible {
                ok = false;
            }
            lines.push(format!(
                "evdev path={} {}",
                path.display(),
                render_input_path_status(&status)
            ));
        }
    }

    if provider.output == InputProviderOutput::Uinput {
        let status = probe.read_write_access(Path::new("/dev/uinput"));
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

    if observations.is_empty() {
        lines.push("observed=none reason=no_key_events_received".to_string());
    }
    for observation in observations {
        lines.push(render_key_discovery_observation(provider, observation));
    }

    if ok {
        lines.push("result=ok key discovery report is current-run only".to_string());
    } else {
        lines.push("result=failed key discovery permissions are incomplete".to_string());
    }

    Ok(KeyDoctorReport { ok, lines })
}

fn render_key_discovery_observation(
    provider: &InputProviderConfig,
    observation: &KeyDiscoveryObservation,
) -> String {
    let Some(token) = KeyToken::from_evdev_code(observation.raw_code) else {
        return format!(
            "key device={} raw_code={} token=unknown aliases=none triggerability=unsupported emittability=unsupported reason=unknown_key_code",
            observation.device, observation.raw_code
        );
    };
    let aliases = token.aliases();
    let aliases = if aliases.is_empty() {
        "none".to_string()
    } else {
        aliases.join(",")
    };
    let emittability = if provider.output == InputProviderOutput::Uinput {
        "supported"
    } else {
        "unavailable"
    };
    format!(
        "key device={} raw_code={} token={} aliases={} triggerability=supported emittability={}",
        observation.device,
        observation.raw_code,
        token.name(),
        aliases,
        emittability
    )
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
    let log = RuntimeLog::default();
    log.info(format!("event=startup script_path={}", lua_file.display()));
    let config = load_lua_file(lua_file)?;
    log.info("event=script_validation result=ok");

    let scope = match config.scope.clone() {
        Some(script_scope) => ScopeSelection::from_script(script_scope),
        None => match prompt.resolve_missing_scope()?.into_scope()? {
            Some(scope) => scope,
            None => {
                log.info("event=scope_prompt result=cancelled");
                return Ok(RuntimeStats::new());
            }
        },
    };
    log.info(format!("event=effective_scope {}", scope.describe()));
    log.info("event=capability_probe result=mock-adapter");

    let mut stats = RuntimeStats::new();
    let bindings = config.bindings_for_scope(scope.clone());
    let motions = config.motions_for_scope(scope.clone());
    let presses = config.presses_for_scope(scope);
    for binding in &bindings {
        stats.record_registration_attempt();
        match registrar.register(binding.clone()) {
            Ok(id) => {
                stats.record_registration_success();
                log.info(format!(
                    "event=registration trigger={} mode={} id={}",
                    binding.trigger_label(),
                    binding.mode.as_str(),
                    id.as_str()
                ));
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
        &presses,
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

pub fn start_controller_runner_with_lifecycle<R, P, E, L>(
    lua_file: &Path,
    registrar: &mut R,
    active_process_provider: &P,
    executor: &mut E,
    lifecycle: &mut L,
    capabilities: CapabilityReport,
) -> Result<RuntimeStats, DiagnosableError>
where
    R: HotkeyRegistrar,
    P: ActiveProcessProvider,
    E: ControllerHost,
    L: RunnerLifecycle,
{
    let log = RuntimeLog::default();
    log.info(format!(
        "event=startup controller_script_path={}",
        lua_file.display()
    ));
    let program = load_lua_controller_program_file(lua_file)?;
    let runtime = load_imperative_controller_runtime(lua_file)?;
    log.info(format!(
        "event=registration result=ok registrations={} callbacks={}",
        program.registrations().registrations().len(),
        program.callbacks().count()
    ));
    program.validate_capabilities(&capabilities)?;
    log.info("event=capability_probe result=ok");

    let mut stats = RuntimeStats::new();
    let hotkey_bindings = controller_hotkey_bindings(&program)?;
    for binding in &hotkey_bindings {
        stats.record_registration_attempt();
        match registrar.register(binding.clone()) {
            Ok(id) => {
                stats.record_registration_success();
                log.info(format!(
                    "event=registration controller=true trigger={} mode={} id={}",
                    binding.trigger_label(),
                    binding.mode.as_str(),
                    id.as_str()
                ));
            }
            Err(error) => {
                stats.record_registration_failure();
                cleanup_after_error(registrar, ErrorPhase::Registration)?;
                return Err(error);
            }
        }
    }

    let shutdown_reason = match run_controller_lifecycle(
        &program,
        runtime.as_ref(),
        active_process_provider,
        executor,
        lifecycle,
        &capabilities,
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
    let log = RuntimeLog::default();
    log.info(format!("event=startup script_path={}", lua_file.display()));
    let config = load_lua_file(lua_file)?;
    log.info("event=script_validation result=ok");

    let scope = match config.scope.clone() {
        Some(script_scope) => ScopeSelection::from_script(script_scope),
        None => match prompt.resolve_missing_scope()?.into_scope()? {
            Some(scope) => scope,
            None => {
                log.info("event=scope_prompt result=cancelled");
                return Ok(RuntimeStats::new());
            }
        },
    };
    log.info(format!("event=effective_scope {}", scope.describe()));

    let mut stats = RuntimeStats::new();
    let bindings = config.bindings_for_scope(scope.clone());
    let motions = config.motions_for_scope(scope.clone());
    let presses = config.presses_for_scope(scope.clone());
    let required = CapabilitySet::for_configuration_scope(&config, &scope);
    log.info("event=startup provider=kde-plasma-wayland");
    adapter.configure_input_provider(config.input_provider.as_ref(), config.leader.as_ref())?;
    let report = adapter.probe_capabilities(&required);
    if let Some(error) = report.first_blocking_error(&required) {
        stats.record_capability_probe_failure();
        stats.record_permission_failure();
        log.warn(format!(
            "event=capability_probe result=failed hint=check_permissions error={error}"
        ));
        return Err(error);
    }
    stats.record_capability_probe_success();
    log.info("event=capability_probe result=ok");

    for binding in &bindings {
        stats.record_registration_attempt();
        match adapter.register(binding.clone()) {
            Ok(id) => {
                stats.record_registration_success();
                log.info(format!(
                    "event=registration trigger={} mode={} id={}",
                    binding.trigger_label(),
                    binding.mode.as_str(),
                    id.as_str()
                ));
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
        &presses,
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

pub fn start_real_controller_runner_with_lifecycle<L>(
    lua_file: &Path,
    adapter: &mut RealWaylandAdapter,
    lifecycle: &mut L,
) -> Result<RuntimeStats, DiagnosableError>
where
    L: RunnerLifecycle,
{
    let log = RuntimeLog::default();
    log.info(format!(
        "event=startup controller_script_path={}",
        lua_file.display()
    ));
    let program = load_lua_controller_program_file(lua_file)?;
    let runtime = load_imperative_controller_runtime(lua_file)?;
    log.info(format!(
        "event=registration result=ok registrations={} callbacks={}",
        program.registrations().registrations().len(),
        program.callbacks().count()
    ));
    let required = program.required_capabilities().clone();
    log.info("event=startup provider=kde-plasma-wayland");
    adapter.configure_input_provider(program.input_provider.as_ref(), program.leader.as_ref())?;
    let report = adapter.probe_capabilities(&required);
    let mut stats = RuntimeStats::new();
    if let Some(error) = report.first_blocking_error(&required) {
        stats.record_capability_probe_failure();
        stats.record_permission_failure();
        log.warn(format!(
            "event=capability_probe result=failed hint=check_permissions error={error}"
        ));
        return Err(error);
    }
    stats.record_capability_probe_success();
    log.info("event=capability_probe result=ok");
    if required.contains(CapabilityKind::ActiveProcessMetadata) {
        adapter.ensure_active_process_provider()?;
        stats.record_kde_bridge_setup();
    }

    let hotkey_bindings = controller_hotkey_bindings(&program)?;
    for binding in &hotkey_bindings {
        stats.record_registration_attempt();
        match adapter.register(binding.clone()) {
            Ok(id) => {
                stats.record_registration_success();
                log.info(format!(
                    "event=registration controller=true trigger={} mode={} id={}",
                    binding.trigger_label(),
                    binding.mode.as_str(),
                    id.as_str()
                ));
            }
            Err(error) => {
                stats.record_registration_failure();
                cleanup_after_error(adapter, ErrorPhase::Registration)?;
                return Err(error);
            }
        }
    }

    let active_adapter = RealWaylandAdapter::new();
    let shutdown_reason = match run_controller_lifecycle(
        &program,
        runtime.as_ref(),
        &active_adapter,
        adapter,
        lifecycle,
        &report,
        &mut stats,
    ) {
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

pub fn start_live_real_runner(
    lua_file: &Path,
    prompt: &mut impl ScopePrompt,
    adapter: &mut RealWaylandAdapter,
) -> Result<RuntimeStats, DiagnosableError> {
    start_live_real_runner_with_options(lua_file, prompt, adapter, RuntimeLog::default())
}

pub fn start_live_real_controller_runner_with_options(
    lua_file: &Path,
    adapter: &mut RealWaylandAdapter,
    log: RuntimeLog,
) -> Result<RuntimeStats, DiagnosableError> {
    let log_guard = init_runtime_logging(&log);
    log.info(format!(
        "event=startup controller_script_path={}",
        lua_file.display()
    ));
    let program = load_lua_controller_program_file(lua_file)?;
    let runtime = load_imperative_controller_runtime(lua_file)?;
    log.info(format!(
        "event=registration result=ok registrations={} callbacks={}",
        program.registrations().registrations().len(),
        program.callbacks().count()
    ));
    let required = program.required_capabilities().clone();
    let signal_fd = RuntimeSignalFd::shutdown()?;
    log.info("event=startup provider=kde-plasma-wayland");
    adapter.configure_input_provider(program.input_provider.as_ref(), program.leader.as_ref())?;
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
    let mut stats = RuntimeStats::new();
    if let Some(error) = report.first_blocking_error(&required) {
        stats.record_capability_probe_failure();
        stats.record_permission_failure();
        log.warn(format!(
            "event=capability_probe result=failed hint=check_permissions error={error}"
        ));
        log_guard.log_summary(&log);
        return Err(error);
    }
    stats.record_capability_probe_success();
    log.info("event=capability_probe result=ok");
    if required.contains(CapabilityKind::ActiveProcessMetadata) {
        log.debug("event=active_process_provider_start provider=kwin-script");
        adapter.ensure_active_process_provider()?;
        stats.record_kde_bridge_setup();
        log.debug("event=active_process_provider_ready provider=kwin-script");
    }

    let hotkey_bindings = controller_hotkey_bindings(&program)?;
    for binding in &hotkey_bindings {
        stats.record_registration_attempt();
        match adapter.register(binding.clone()) {
            Ok(id) => {
                stats.record_registration_success();
                log.info(format!(
                    "event=registration controller=true trigger={} mode={} id={}",
                    binding.trigger_label(),
                    binding.mode.as_str(),
                    id.as_str()
                ));
            }
            Err(error) => {
                stats.record_registration_failure();
                cleanup_after_error(adapter, ErrorPhase::Registration)?;
                return Err(error);
            }
        }
    }

    let shutdown_reason = match run_live_real_controller_lifecycle(
        &program,
        runtime.as_ref(),
        adapter,
        &report,
        &mut stats,
        log,
        signal_fd,
    ) {
        Ok(reason) => reason,
        Err(error) => {
            println!("{}", stats.render_summary(ShutdownReason::RuntimeError));
            adapter.cancel_pending()?;
            cleanup_after_error(adapter, ErrorPhase::Shutdown)?;
            log_guard.log_summary(&log);
            return Err(error);
        }
    };

    println!("{}", stats.render_summary(shutdown_reason));
    adapter.cancel_pending()?;
    adapter.unregister_all()?;
    log.info(format!("event=shutdown reason={shutdown_reason:?}"));
    log_guard.log_summary(&log);
    Ok(stats)
}

pub fn start_live_real_runner_with_options(
    lua_file: &Path,
    prompt: &mut impl ScopePrompt,
    adapter: &mut RealWaylandAdapter,
    log: RuntimeLog,
) -> Result<RuntimeStats, DiagnosableError> {
    let log_guard = init_runtime_logging(&log);
    log.info(format!("event=startup script_path={}", lua_file.display()));
    let config = load_lua_file(lua_file)?;
    log.info("event=script_validation result=ok");
    log.debug_lazy(|| {
        format!(
            "event=config_loaded bindings={} motions={} presses={} input_provider={} leader={}",
            config.bindings().len(),
            config.motions().len(),
            config.presses().len(),
            config.input_provider.is_some(),
            config
                .leader
                .as_ref()
                .map(signal_auras_core::MotionToken::describe)
                .unwrap_or_else(|| "none".to_string())
        )
    });
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
                log.info("event=scope_prompt result=cancelled");
                log_guard.log_summary(&log);
                return Ok(RuntimeStats::new());
            }
        },
    };
    log.info(format!("event=effective_scope {}", scope.describe()));

    let mut stats = RuntimeStats::new();
    let bindings = config.bindings_for_scope(scope.clone());
    let motions = config.motions_for_scope(scope.clone());
    let presses = config.presses_for_scope(scope.clone());
    let required = CapabilitySet::for_configuration_scope(&config, &scope);
    let signal_fd = RuntimeSignalFd::shutdown()?;
    log.info("event=startup provider=kde-plasma-wayland");
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
        log.warn(format!(
            "event=capability_probe result=failed hint=check_permissions error={error}"
        ));
        log_guard.log_summary(&log);
        return Err(error);
    }
    stats.record_capability_probe_success();
    log.info("event=capability_probe result=ok");
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
                log.info(format!(
                    "event=registration trigger={} mode={} id={}",
                    binding.trigger_label(),
                    binding.mode.as_str(),
                    id.as_str()
                ));
            }
            Err(error) => {
                stats.record_registration_failure();
                cleanup_after_error(adapter, ErrorPhase::Registration)?;
                return Err(error);
            }
        }
    }

    let shutdown_reason = match run_live_real_lifecycle(
        &bindings, &motions, &presses, adapter, &mut stats, log, signal_fd,
    ) {
        Ok(reason) => reason,
        Err(error) => {
            println!("{}", stats.render_summary(ShutdownReason::RuntimeError));
            adapter.cancel_pending()?;
            cleanup_after_error(adapter, ErrorPhase::Shutdown)?;
            log_guard.log_summary(&log);
            return Err(error);
        }
    };

    println!("{}", stats.render_summary(shutdown_reason));
    adapter.cancel_pending()?;
    adapter.unregister_all()?;
    log.info(format!("event=shutdown reason={shutdown_reason:?}"));
    log_guard.log_summary(&log);
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
        motion
            .loop_definition
            .as_ref()
            .is_some_and(|loop_definition| {
                loop_definition
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
    ObservedMotionInput {
        event: MotionInputEvent,
        kernel_timestamp: KernelEventTimestamp,
        observed_at: Instant,
    },
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
    presses: &[RuntimePress],
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
    let motion_time_base = Instant::now();
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
                stats.record_motion_event_age_unavailable();
                let motion_events = motion_runtime.handle_input(event.clone());
                handle_press_input(
                    &event,
                    presses,
                    &motion_runtime,
                    active_process_provider,
                    executor,
                    &mut scheduler,
                    stats,
                )?;
                for event in motion_events {
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
            RunnerEvent::ObservedMotionInput {
                event,
                kernel_timestamp,
                observed_at,
            } => {
                record_motion_latency_metrics(stats, observed_at, kernel_timestamp);
                let input_event = event.clone();
                let motion_events = motion_runtime.handle_input_at(
                    event,
                    motion_runtime_event_time(kernel_timestamp, observed_at, motion_time_base),
                );
                handle_press_input(
                    &input_event,
                    presses,
                    &motion_runtime,
                    active_process_provider,
                    executor,
                    &mut scheduler,
                    stats,
                )?;
                for event in motion_events {
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
                if motion_runtime.loop_is_active(&trigger) {
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
            RunnerEvent::Shutdown(reason) => {
                for trigger in motion_runtime.cancel_active_loops() {
                    if let Some(motion) = motions
                        .iter()
                        .find(|motion| motion.definition.trigger == trigger)
                    {
                        execute_motion_loop_after(motion, executor, &mut scheduler, stats)?;
                    }
                }
                return Ok(reason);
            }
            RunnerEvent::RuntimeError(error) => return Err(error),
        }
    }
}

fn controller_hotkey_bindings(
    program: &ControllerProgram,
) -> Result<Vec<HotkeyBinding>, DiagnosableError> {
    program
        .registrations()
        .registrations()
        .iter()
        .filter(|registration| registration.kind == ControllerRegistrationKind::Hotkey)
        .map(controller_hotkey_binding)
        .collect()
}

fn controller_hotkey_binding(
    registration: &ControllerRegistration,
) -> Result<HotkeyBinding, DiagnosableError> {
    Ok(HotkeyBinding {
        trigger: BindingTrigger::keyboard(HotkeyId::parse(&registration.trigger)?),
        scope: registration.scope.clone(),
        mode: registration.mode,
        macro_definition: MacroDefinition::new(vec![MacroAction::delay(1)?])?,
        registration_state: RegistrationState::Pending,
    })
}

fn run_controller_lifecycle<P, E, L>(
    program: &ControllerProgram,
    runtime: Option<&ImperativeLuaController>,
    active_process_provider: &P,
    executor: &mut E,
    lifecycle: &mut L,
    capabilities: &CapabilityReport,
    stats: &mut RuntimeStats,
) -> Result<ShutdownReason, DiagnosableError>
where
    P: ActiveProcessProvider,
    E: ControllerHost,
    L: RunnerLifecycle,
{
    let mut scheduler = LuaCallbackScheduler::new(64, Duration::from_millis(50))?;
    loop {
        match lifecycle.next_event()? {
            RunnerEvent::Hotkey(hotkey) => {
                if let Some(registration) = controller_registration_for_hotkey(program, &hotkey) {
                    let _ = schedule_controller_callback(
                        registration,
                        active_process_provider,
                        &mut scheduler,
                        capabilities,
                        stats,
                        Instant::now(),
                    )?;
                }
            }
            RunnerEvent::Callback {
                hotkey,
                received_at,
            } => {
                stats.record_callback_received();
                stats.record_callback_dispatched(received_at.elapsed().as_millis() as u64);
                if let Some(registration) = controller_registration_for_hotkey(program, &hotkey) {
                    let _ = schedule_controller_callback(
                        registration,
                        active_process_provider,
                        &mut scheduler,
                        capabilities,
                        stats,
                        received_at,
                    )?;
                } else {
                    stats.record_shortcut_ignored();
                }
            }
            RunnerEvent::Trigger(trigger) => {
                if let BindingTrigger::Keyboard(hotkey) = trigger {
                    if let Some(registration) = controller_registration_for_hotkey(program, &hotkey)
                    {
                        let _ = schedule_controller_callback(
                            registration,
                            active_process_provider,
                            &mut scheduler,
                            capabilities,
                            stats,
                            Instant::now(),
                        )?;
                    }
                }
            }
            RunnerEvent::Shutdown(reason) => {
                let cancelled = scheduler.cancel_all();
                if cancelled > 0 {
                    stats.record_cancelled_macro_runs(cancelled as u64);
                }
                return Ok(reason);
            }
            RunnerEvent::RuntimeError(error) => return Err(error),
            RunnerEvent::MotionInput(_)
            | RunnerEvent::ObservedMotionInput { .. }
            | RunnerEvent::MotionRepeatTick(_) => {}
        }
        drain_controller_callbacks(
            program,
            runtime,
            &mut scheduler,
            capabilities,
            executor,
            stats,
        )?;
    }
}

fn controller_registration_for_hotkey<'a>(
    program: &'a ControllerProgram,
    hotkey: &HotkeyId,
) -> Option<&'a ControllerRegistration> {
    program
        .registrations()
        .registrations()
        .iter()
        .find(|registration| {
            registration.kind == ControllerRegistrationKind::Hotkey
                && HotkeyId::parse(&registration.trigger).is_ok_and(|trigger| trigger == *hotkey)
        })
}

fn schedule_controller_callback<P>(
    registration: &ControllerRegistration,
    active_process_provider: &P,
    scheduler: &mut LuaCallbackScheduler,
    capabilities: &CapabilityReport,
    stats: &mut RuntimeStats,
    accepted_at: Instant,
) -> Result<bool, DiagnosableError>
where
    P: ActiveProcessProvider,
{
    schedule_controller_callback_name(
        registration,
        &registration.callback,
        active_process_provider,
        scheduler,
        capabilities,
        stats,
        accepted_at,
    )
}

fn schedule_controller_callback_name<P>(
    registration: &ControllerRegistration,
    callback_name: &str,
    active_process_provider: &P,
    scheduler: &mut LuaCallbackScheduler,
    capabilities: &CapabilityReport,
    stats: &mut RuntimeStats,
    accepted_at: Instant,
) -> Result<bool, DiagnosableError>
where
    P: ActiveProcessProvider,
{
    let active_context = active_process_provider.active_process_context()?;
    let state = registration.scope.scoped_focus_state(&active_context);
    if !state.is_active() {
        if let Some(diagnostic) = state.diagnostic {
            record_scope_denial(stats, &diagnostic);
            tracing::info!(
                event = "callback_received",
                trigger = %registration.trigger,
                reason = state.reason.as_str(),
                details = %diagnostic.render_fields(),
                disposition = "denied"
            );
        }
        return Ok(false);
    }
    stats.record_trigger(&registration.trigger);
    match registration.mode {
        BindingMode::Consume => stats.record_consumed_trigger_event(),
        BindingMode::Passthrough => stats.record_passthrough_trigger_event(),
    }
    stats.record_active_process_match();
    let scheduled = registration.clone().with_callback(callback_name)?;
    let result = scheduler.schedule(&scheduled, capabilities, accepted_at);
    match result.disposition {
        CallbackDisposition::Accepted => {}
        CallbackDisposition::Skipped | CallbackDisposition::Dropped => {
            record_non_repeat_collision_skip(stats);
            stats.record_callback_dropped(1);
        }
        CallbackDisposition::Denied => {
            stats.denied_action_count += 1;
            if result.diagnostic.is_some() {
                stats.record_permission_failure();
            }
        }
        CallbackDisposition::Completed
        | CallbackDisposition::Slow
        | CallbackDisposition::Failed
        | CallbackDisposition::Cancelled => {}
    }
    tracing::debug!(
        event = "callback_received",
        trigger = %registration.trigger,
        callback = callback_name,
        disposition = ?result.disposition,
        queue_depth = scheduler.pending_len()
    );
    stats.record_output_queue_depth(scheduler.pending_len() as u64);
    Ok(result.disposition == CallbackDisposition::Accepted
        && registration.mode == BindingMode::Consume)
}

fn drain_controller_callbacks<E>(
    program: &ControllerProgram,
    runtime: Option<&ImperativeLuaController>,
    scheduler: &mut LuaCallbackScheduler,
    capabilities: &CapabilityReport,
    executor: &mut E,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError>
where
    E: ControllerHost,
{
    while let Some(task) = scheduler.pop_next() {
        let started_at = Instant::now();
        let result =
            execute_controller_task(program, runtime, &task, capabilities, executor, stats);
        let disposition = scheduler.finish(task, started_at.elapsed());
        if disposition == CallbackDisposition::Slow {
            tracing::warn!(event = "callback_received", disposition = "slow");
        }
        result?;
    }
    Ok(())
}

fn execute_controller_task<E>(
    program: &ControllerProgram,
    runtime: Option<&ImperativeLuaController>,
    task: &signal_auras_core::LuaCallbackTask,
    capabilities: &CapabilityReport,
    executor: &mut E,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError>
where
    E: ControllerHost,
{
    let callback = program.callback(&task.callback).ok_or_else(|| {
        DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("controller callback '{}' is unavailable", task.callback),
        )
    })?;
    if callback.actions.is_empty() {
        if let Some(runtime) = runtime {
            return execute_imperative_controller_task(
                runtime,
                &task.callback,
                capabilities,
                executor,
                stats,
            );
        }
    }
    let mut batch = RustOperationBatch::new(256)?;
    queue_controller_callback_outputs(callback, capabilities, &mut batch)?;
    stats.record_output_queue_depth(batch.len() as u64);
    for request in batch.drain() {
        match executor.execute_input_request(request)? {
            signal_auras_core::InputEmission::Emitted => stats.record_synthesized_input_emitted(),
            signal_auras_core::InputEmission::Denied => {
                stats.record_synthesized_input_denied();
                return Err(DiagnosableError::new(
                    ErrorPhase::MacroExecution,
                    "controller synthesized input was denied",
                ));
            }
            signal_auras_core::InputEmission::Failed => {
                return Err(DiagnosableError::new(
                    ErrorPhase::MacroExecution,
                    "controller synthesized input failed",
                ));
            }
            signal_auras_core::InputEmission::Cancelled => {
                return Err(DiagnosableError::new(
                    ErrorPhase::Shutdown,
                    "controller synthesized input was cancelled",
                ));
            }
        }
    }
    stats.macro_success_count += 1;
    Ok(())
}

fn execute_imperative_controller_task<E>(
    runtime: &ImperativeLuaController,
    callback_name: &str,
    capabilities: &CapabilityReport,
    executor: &mut E,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError>
where
    E: ControllerHost,
{
    let coroutine = runtime.start_callback(callback_name)?;
    let declared = runtime.registrations().required_capabilities();
    let mut response = LuaHostResponse::Unit;
    loop {
        match runtime.resume_callback(&coroutine, response, declared)? {
            LuaCallbackStep::Complete => {
                stats.macro_success_count += 1;
                return Ok(());
            }
            LuaCallbackStep::Yielded(request) => {
                if let Some(required) = request.required_capability() {
                    let required = CapabilitySet::new([required]);
                    if let Some(error) = capabilities.first_blocking_error(&required) {
                        stats.record_permission_failure();
                        stats.denied_action_count += 1;
                        return Err(error);
                    }
                }
                response = execute_lua_host_request(request, executor, stats)?;
            }
        }
    }
}

fn execute_lua_host_request<E>(
    request: LuaHostRequest,
    executor: &mut E,
    stats: &mut RuntimeStats,
) -> Result<LuaHostResponse, DiagnosableError>
where
    E: ControllerHost,
{
    match request {
        LuaHostRequest::Sleep { duration_ms } => {
            executor.sleep(Duration::from_millis(duration_ms))?;
            Ok(LuaHostResponse::Unit)
        }
        LuaHostRequest::Log { level, message } => {
            match level {
                LuaLogLevel::Debug => tracing::debug!(event = "lua_log", message = %message),
                LuaLogLevel::Info => tracing::info!(event = "lua_log", message = %message),
                LuaLogLevel::Warn => tracing::warn!(event = "lua_log", message = %message),
            }
            Ok(LuaHostResponse::Unit)
        }
        LuaHostRequest::ActiveWindow { include_title } => executor
            .active_window(include_title)
            .map(LuaHostResponse::ActiveWindow),
        LuaHostRequest::FindWindow { processes } => executor
            .find_window(&processes)
            .map(LuaHostResponse::WindowHandle),
        LuaHostRequest::ActivateWindow { handle } => {
            executor.activate_window(&handle).map(LuaHostResponse::Bool)
        }
        LuaHostRequest::WaitActive { handle, timeout_ms } => executor
            .wait_active_window(&handle, Duration::from_millis(timeout_ms))
            .map(LuaHostResponse::Bool),
        LuaHostRequest::Input { action } => {
            let request = SynthesizedInputRequest::new(action, 0);
            match executor.execute_input_request(request)? {
                signal_auras_core::InputEmission::Emitted => {
                    stats.record_synthesized_input_emitted();
                    Ok(LuaHostResponse::Unit)
                }
                signal_auras_core::InputEmission::Denied => {
                    stats.record_synthesized_input_denied();
                    Err(DiagnosableError::new(
                        ErrorPhase::MacroExecution,
                        "controller synthesized input was denied",
                    ))
                }
                signal_auras_core::InputEmission::Failed => Err(DiagnosableError::new(
                    ErrorPhase::MacroExecution,
                    "controller synthesized input failed",
                )),
                signal_auras_core::InputEmission::Cancelled => Err(DiagnosableError::new(
                    ErrorPhase::Shutdown,
                    "controller synthesized input was cancelled",
                )),
            }
        }
    }
}

fn load_imperative_controller_runtime(
    lua_file: &Path,
) -> Result<Option<ImperativeLuaController>, DiagnosableError> {
    let source = fs::read_to_string(lua_file).map_err(|error| {
        DiagnosableError::new(
            ErrorPhase::ScriptLoad,
            format!(
                "cannot read Lua controller file '{}': {error}",
                lua_file.display()
            ),
        )
    })?;
    if !(source.contains("sa.sleep") || source.contains("sa.window.")) {
        return Ok(None);
    }
    ImperativeLuaController::load_source(&source).map(Some)
}

fn run_live_real_lifecycle(
    bindings: &[HotkeyBinding],
    motions: &[RuntimeMotion],
    presses: &[RuntimePress],
    adapter: &mut RealWaylandAdapter,
    stats: &mut RuntimeStats,
    log: RuntimeLog,
    mut signal_fd: RuntimeSignalFd,
) -> Result<ShutdownReason, DiagnosableError> {
    let timer_fd = RuntimeTimerFd::new()?;
    let mut macro_queue = LiveMacroQueue::default();
    let mut focus_tracker = ScopedFocusTracker::default();
    let mut motion_runtime =
        MotionRuntime::new(motions.iter().map(|motion| motion.definition.clone()));
    let motion_time_base = Instant::now();
    let mut repeat_ticks = motions
        .iter()
        .filter_map(|motion| {
            motion
                .definition
                .loop_definition
                .as_ref()
                .and_then(|loop_definition| {
                    loop_definition
                        .repeat()
                        .map(|repeat| (motion.definition.trigger.clone(), repeat.interval.every_ms))
                })
        })
        .collect::<Vec<_>>();
    let mut last_repeat_ticks = std::collections::BTreeMap::new();
    loop {
        stats.record_event_loop_wakeup();
        macro_queue.drive_ready(adapter, stats)?;
        drain_live_shortcut_callbacks(
            bindings,
            adapter,
            &mut macro_queue,
            &mut focus_tracker,
            stats,
            log,
        )?;
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
                let mut context = LiveMotionInputContext {
                    motions,
                    presses,
                    adapter,
                    macro_queue: &mut macro_queue,
                    motion_runtime: &mut motion_runtime,
                    focus_tracker: &mut focus_tracker,
                    stats,
                    log,
                    motion_time_base,
                };
                handle_observed_input(event, &mut context)?;
            }
            signal_auras_wayland::evdev::EvdevInputWaitOutcome::RuntimeFd(fd)
                if fd == signal_fd.as_raw_fd() =>
            {
                if let Some(reason) = signal_fd.drain_shutdown_reason()? {
                    for cancelled in motion_runtime.cancel_active_loops() {
                        stats.record_motion_repeat_cancel();
                        schedule_live_motion_loop_after(
                            &cancelled,
                            motions,
                            &mut macro_queue,
                            stats,
                        );
                    }
                    macro_queue.drive_ready(adapter, stats)?;
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
                drain_live_shortcut_callbacks(
                    bindings,
                    adapter,
                    &mut macro_queue,
                    &mut focus_tracker,
                    stats,
                    log,
                )?;
            }
            signal_auras_wayland::evdev::EvdevInputWaitOutcome::RuntimeFd(_) => {}
            signal_auras_wayland::evdev::EvdevInputWaitOutcome::Timeout => {}
        }
        while let Some(event) = adapter.next_input_event()? {
            let mut context = LiveMotionInputContext {
                motions,
                presses,
                adapter,
                macro_queue: &mut macro_queue,
                motion_runtime: &mut motion_runtime,
                focus_tracker: &mut focus_tracker,
                stats,
                log,
                motion_time_base,
            };
            handle_observed_input(event, &mut context)?;
        }
        macro_queue.drive_ready(adapter, stats)?;
        let now = Instant::now();
        for (trigger, interval_ms) in &mut repeat_ticks {
            if !motion_runtime.loop_is_active(trigger) {
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
                        &mut focus_tracker,
                        stats,
                        log,
                    )?;
                    if focus_tracker.take_deactivation() {
                        for cancelled in motion_runtime.cancel_active_loops() {
                            stats.record_motion_repeat_cancel();
                            schedule_live_motion_loop_after(
                                &cancelled,
                                motions,
                                &mut macro_queue,
                                stats,
                            );
                            log.debug(format!(
                                "event=motion_repeat_cancelled trigger={} reason=scoped_focus_inactive",
                                trigger_label_for_log(&cancelled)
                            ));
                        }
                    }
                }
                last_repeat_ticks.insert(trigger.clone(), now);
            }
        }
        macro_queue.drive_ready(adapter, stats)?;
    }
}

fn run_live_real_controller_lifecycle(
    program: &ControllerProgram,
    runtime: Option<&ImperativeLuaController>,
    adapter: &mut RealWaylandAdapter,
    capabilities: &CapabilityReport,
    stats: &mut RuntimeStats,
    log: RuntimeLog,
    mut signal_fd: RuntimeSignalFd,
) -> Result<ShutdownReason, DiagnosableError> {
    let timer_fd = RuntimeTimerFd::new()?;
    let mut scheduler = LuaCallbackScheduler::new(64, Duration::from_millis(50))?;
    let mut motion_runtime = MotionRuntime::new(controller_motion_definitions(program)?);
    let mut repeat_ticks = controller_repeat_ticks(program)?;
    let mut last_repeat_ticks = BTreeMap::new();
    let motion_time_base = Instant::now();
    let tracker_time_base = Instant::now();
    let mut state_trackers = if program.state_trackers().is_empty() {
        None
    } else {
        Some(LiveStateTrackerRuntime::new(StateTrackerPoller::new(
            program.state_trackers().clone(),
        )))
    };
    loop {
        stats.record_event_loop_wakeup();
        drain_live_controller_shortcut_callbacks(
            program,
            adapter,
            &mut scheduler,
            capabilities,
            stats,
            log,
        )?;
        poll_live_state_trackers(
            program,
            &mut state_trackers,
            adapter,
            capabilities,
            log,
            tracker_time_base,
        )?;
        drain_controller_callbacks(
            program,
            runtime,
            &mut scheduler,
            capabilities,
            adapter,
            stats,
        )?;
        let wait_timeout =
            next_live_wait_timeout(&repeat_ticks, &last_repeat_ticks, &motion_runtime).min(
                next_live_state_tracker_wait_timeout(&state_trackers, tracker_time_base),
            );
        timer_fd.arm_after(wait_timeout)?;
        let mut runtime_fds = vec![signal_fd.as_raw_fd(), timer_fd.as_raw_fd()];
        if let Some(fd) = adapter.callback_wake_fd() {
            runtime_fds.push(fd);
        }
        match adapter.wait_next_input_or_runtime_fd(wait_timeout, &runtime_fds)? {
            signal_auras_wayland::evdev::EvdevInputWaitOutcome::RuntimeFd(fd)
                if fd == signal_fd.as_raw_fd() =>
            {
                if let Some(reason) = signal_fd.drain_shutdown_reason()? {
                    let cancelled = scheduler.cancel_all();
                    if cancelled > 0 {
                        stats.record_cancelled_macro_runs(cancelled as u64);
                    }
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
                drain_live_controller_shortcut_callbacks(
                    program,
                    adapter,
                    &mut scheduler,
                    capabilities,
                    stats,
                    log,
                )?;
            }
            signal_auras_wayland::evdev::EvdevInputWaitOutcome::Input(event) => {
                let mut context = LiveControllerInputContext {
                    program,
                    adapter,
                    motion_runtime: &mut motion_runtime,
                    scheduler: &mut scheduler,
                    capabilities,
                    stats,
                    last_repeat_ticks: &mut last_repeat_ticks,
                    motion_time_base,
                };
                handle_live_controller_observed_input(event, &mut context)?;
            }
            signal_auras_wayland::evdev::EvdevInputWaitOutcome::RuntimeFd(_)
            | signal_auras_wayland::evdev::EvdevInputWaitOutcome::Timeout => {}
        }
        while let Some(event) = adapter.next_input_event()? {
            let mut context = LiveControllerInputContext {
                program,
                adapter,
                motion_runtime: &mut motion_runtime,
                scheduler: &mut scheduler,
                capabilities,
                stats,
                last_repeat_ticks: &mut last_repeat_ticks,
                motion_time_base,
            };
            handle_live_controller_observed_input(event, &mut context)?;
        }
        poll_live_state_trackers(
            program,
            &mut state_trackers,
            adapter,
            capabilities,
            log,
            tracker_time_base,
        )?;
        drain_controller_callbacks(
            program,
            runtime,
            &mut scheduler,
            capabilities,
            adapter,
            stats,
        )?;
        let now = Instant::now();
        for (trigger, interval_ms) in &mut repeat_ticks {
            if !motion_runtime.loop_is_active(trigger) {
                last_repeat_ticks.remove(trigger);
                continue;
            }
            let due = last_repeat_ticks.get(trigger).is_none_or(|last_tick| {
                now.duration_since(*last_tick).as_millis() >= *interval_ms as u128
            });
            if due {
                if let Some(registration) =
                    matching_controller_motion_registration(program, trigger)?
                {
                    if let Some(loop_policy) = &registration.loop_policy {
                        let _ = schedule_controller_callback_name(
                            registration,
                            &loop_policy.repeat_callback,
                            &*adapter,
                            &mut scheduler,
                            capabilities,
                            stats,
                            now,
                        )?;
                        stats.record_motion_repeat_tick();
                    }
                }
                last_repeat_ticks.insert(trigger.clone(), now);
            }
        }
        drain_controller_callbacks(
            program,
            runtime,
            &mut scheduler,
            capabilities,
            adapter,
            stats,
        )?;
    }
}

fn poll_live_state_trackers(
    program: &ControllerProgram,
    runtime: &mut Option<LiveStateTrackerRuntime>,
    adapter: &mut RealWaylandAdapter,
    capabilities: &CapabilityReport,
    log: RuntimeLog,
    tracker_time_base: Instant,
) -> Result<(), DiagnosableError> {
    let Some(runtime) = runtime.as_mut() else {
        return Ok(());
    };
    let now_ms = tracker_time_base.elapsed().as_millis() as u64;
    let Some(due_in_ms) = runtime.poller.next_due_in_ms(now_ms) else {
        return Ok(());
    };
    if due_in_ms > 0 {
        return Ok(());
    }
    log.trace(format!(
        "event=state_tracker_poll phase=begin tracker_count={} now_ms={}",
        program.state_trackers().trackers().len(),
        now_ms
    ));
    let active_context = adapter.active_process_context()?;
    log.trace(format!(
        "event=state_tracker_focus confidence={:?} metadata_age_ms={} has_pid={} has_app_id={} has_window_class={}",
        active_context.confidence,
        active_context.captured_at.elapsed().as_millis(),
        active_context.process_id.is_some(),
        active_context.app_id.is_some(),
        active_context.window_class.is_some()
    ));
    let outcome = runtime
        .poller
        .poll_due(now_ms, capabilities, &active_context, adapter);
    log.trace(format!(
        "event=state_tracker_poll phase=complete due={} updated={} samples={}",
        outcome.due_trackers,
        outcome.updated.len(),
        outcome.screen_samples
    ));
    for id in outcome.updated {
        let tracker = program
            .state_trackers()
            .trackers()
            .iter()
            .find(|tracker| tracker.id == id);
        let detector_kind = tracker
            .map(|tracker| tracker.detector.kind())
            .unwrap_or("unknown");
        if let Some(state) = runtime.poller.latest_state(&id).cloned() {
            let message = format!(
                "event=state_tracker id={} detector={} confidence={} samples={} {}",
                id,
                detector_kind,
                state.confidence(),
                outcome.screen_samples,
                state.summary()
            );
            match runtime.log_level_for_update(&id, &state) {
                StateTrackerUpdateLogLevel::Info => log.info(message),
                StateTrackerUpdateLogLevel::Trace => log.trace(message),
            }
            if matches!(
                state,
                TrackerState::Inactive {
                    reason: signal_auras_core::TrackerInactiveReason::FocusInactive,
                    ..
                }
            ) {
                if let Some(tracker) = tracker {
                    let focus_state = tracker.scope.scoped_focus_state(&active_context);
                    let fields = focus_state.transition_fields();
                    if runtime.focus_denial_changed(&id, &fields) {
                        log.debug(format!(
                            "event=state_tracker_focus_denial id={} detector={} {}",
                            id, detector_kind, fields
                        ));
                    }
                }
            }
        }
    }
    poll_live_overlays(program, runtime, capabilities, &active_context, log);
    Ok(())
}

fn poll_live_overlays(
    program: &ControllerProgram,
    runtime: &LiveStateTrackerRuntime,
    capabilities: &CapabilityReport,
    active_context: &signal_auras_core::ActiveProcessContext,
    log: RuntimeLog,
) {
    if program.overlays().is_empty() {
        return;
    }
    let snapshots = program.overlays().snapshots(
        0,
        capabilities,
        active_context,
        runtime.poller.latest_states(),
        &OverlayProviderReport::native_available(),
    );
    for snapshot in snapshots {
        if snapshot.is_active() {
            log.trace(format!(
                "event=overlay_snapshot id={} provider={} state=active visuals={}",
                snapshot.overlay_id,
                snapshot.provider.as_str(),
                snapshot.visuals.len()
            ));
        } else if let Some(diagnostic) = snapshot.diagnostic {
            log.debug(format!(
                "event=overlay_snapshot id={} provider={} state={:?} reason={:?} tracker={:?} field={:?}",
                diagnostic.overlay_id,
                diagnostic.provider.as_str(),
                diagnostic.lifecycle,
                diagnostic.reason,
                diagnostic.tracker_id,
                diagnostic.field.map(|field| field.as_str())
            ));
        }
    }
}

struct LiveStateTrackerRuntime {
    poller: StateTrackerPoller,
    last_summaries: BTreeMap<String, String>,
    last_focus_denials: BTreeMap<String, String>,
}

impl LiveStateTrackerRuntime {
    fn new(poller: StateTrackerPoller) -> Self {
        Self {
            poller,
            last_summaries: BTreeMap::new(),
            last_focus_denials: BTreeMap::new(),
        }
    }

    fn log_level_for_update(
        &mut self,
        id: &str,
        state: &TrackerState,
    ) -> StateTrackerUpdateLogLevel {
        let summary = state.summary();
        let unchanged = self.last_summaries.get(id) == Some(&summary);
        self.last_summaries.insert(id.to_string(), summary);
        if unchanged && matches!(state, TrackerState::Inactive { .. }) {
            StateTrackerUpdateLogLevel::Trace
        } else {
            StateTrackerUpdateLogLevel::Info
        }
    }

    fn focus_denial_changed(&mut self, id: &str, fields: &str) -> bool {
        let changed = self
            .last_focus_denials
            .get(id)
            .is_none_or(|previous| previous != fields);
        self.last_focus_denials
            .insert(id.to_string(), fields.to_string());
        changed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StateTrackerUpdateLogLevel {
    Info,
    Trace,
}

fn next_live_state_tracker_wait_timeout(
    runtime: &Option<LiveStateTrackerRuntime>,
    tracker_time_base: Instant,
) -> Duration {
    let Some(runtime) = runtime.as_ref() else {
        return idle_wait_timeout();
    };
    let now_ms = tracker_time_base.elapsed().as_millis() as u64;
    runtime
        .poller
        .next_due_in_ms(now_ms)
        .map(Duration::from_millis)
        .unwrap_or_else(idle_wait_timeout)
}

fn controller_motion_definitions(
    program: &ControllerProgram,
) -> Result<Vec<MotionDefinition>, DiagnosableError> {
    program
        .registrations()
        .registrations()
        .iter()
        .filter(|registration| registration.kind == ControllerRegistrationKind::Motion)
        .map(|registration| {
            MotionDefinition::with_requires_held(
                registration.requires_held.clone(),
                controller_motion_trigger(registration)?,
                registration.mode,
                if registration.loop_policy.is_none() {
                    Some(controller_dummy_macro()?)
                } else {
                    None
                },
                registration
                    .loop_policy
                    .as_ref()
                    .map(controller_loop_definition)
                    .transpose()?,
                signal_auras_core::DEFAULT_MOTION_DURATION.as_millis() as u64,
                0,
            )
        })
        .collect()
}

fn controller_dummy_macro() -> Result<MacroDefinition, DiagnosableError> {
    MacroDefinition::new(vec![MacroAction::delay(1)?])
}

fn controller_loop_definition(
    loop_policy: &signal_auras_core::ControllerLoopPolicy,
) -> Result<LoopDefinition, DiagnosableError> {
    Ok(LoopDefinition::new(
        loop_policy.while_held.clone(),
        loop_policy
            .before_callback
            .as_ref()
            .map(|_| controller_dummy_macro())
            .transpose()?,
        LoopBody::Repeat(LoopRepeat::new(
            LoopInterval::new(loop_policy.repeat_every_ms)?,
            controller_dummy_macro()?,
        )),
        loop_policy
            .after_callback
            .as_ref()
            .map(|_| controller_dummy_macro())
            .transpose()?,
    ))
}

fn controller_repeat_ticks(
    program: &ControllerProgram,
) -> Result<Vec<(MotionTrigger, u64)>, DiagnosableError> {
    program
        .registrations()
        .registrations()
        .iter()
        .filter(|registration| registration.kind == ControllerRegistrationKind::Motion)
        .filter_map(|registration| {
            registration.loop_policy.as_ref().map(|loop_policy| {
                Ok((
                    controller_motion_trigger(registration)?,
                    loop_policy.repeat_every_ms,
                ))
            })
        })
        .collect()
}

fn controller_motion_trigger(
    registration: &ControllerRegistration,
) -> Result<MotionTrigger, DiagnosableError> {
    MotionTrigger::parse(registration.trigger.split_whitespace())
}

fn controller_press_token(
    registration: &ControllerRegistration,
) -> Result<MotionToken, DiagnosableError> {
    MotionToken::parse(&registration.trigger)
}

struct LiveControllerInputContext<'a> {
    program: &'a ControllerProgram,
    adapter: &'a mut RealWaylandAdapter,
    motion_runtime: &'a mut MotionRuntime,
    scheduler: &'a mut LuaCallbackScheduler,
    capabilities: &'a CapabilityReport,
    stats: &'a mut RuntimeStats,
    last_repeat_ticks: &'a mut BTreeMap<MotionTrigger, Instant>,
    motion_time_base: Instant,
}

fn handle_live_controller_observed_input(
    observed: signal_auras_wayland::evdev::ObservedInputEvent,
    context: &mut LiveControllerInputContext<'_>,
) -> Result<(), DiagnosableError> {
    let Some(event) = observed.event.clone() else {
        if observed.grabbed {
            context.adapter.passthrough_raw_input(&observed.raw)?;
        }
        return Ok(());
    };
    if event.token == MotionToken::Leader && event.state == MotionInputState::Pressed {
        context.adapter.arm_input_grab()?;
    }
    let (_, _) = record_motion_latency_metrics(
        context.stats,
        observed.observed_at,
        observed.raw.kernel_timestamp,
    );
    let event_time = motion_runtime_event_time(
        observed.raw.kernel_timestamp,
        observed.observed_at,
        context.motion_time_base,
    );
    let mut consumed = false;
    let motion_events = context
        .motion_runtime
        .handle_input_at(event.clone(), event_time);
    if event.state == MotionInputState::Pressed {
        if let Some(registration) =
            matching_controller_press_registration(context.program, &event, context.motion_runtime)?
        {
            consumed |= schedule_controller_callback(
                registration,
                &*context.adapter,
                context.scheduler,
                context.capabilities,
                context.stats,
                observed.observed_at,
            )?;
        }
    }
    for event in motion_events {
        match event {
            MotionRuntimeEvent::Triggered {
                trigger,
                starts_loop,
            } => {
                if let Some(registration) =
                    matching_controller_motion_registration(context.program, &trigger)?
                {
                    if starts_loop {
                        if let Some(loop_policy) = &registration.loop_policy {
                            if let Some(before_callback) = &loop_policy.before_callback {
                                consumed |= schedule_controller_callback_name(
                                    registration,
                                    before_callback,
                                    &*context.adapter,
                                    context.scheduler,
                                    context.capabilities,
                                    context.stats,
                                    observed.observed_at,
                                )?;
                            }
                            context
                                .last_repeat_ticks
                                .insert(trigger.clone(), Instant::now());
                        }
                    } else if registration.loop_policy.is_none() {
                        consumed |= schedule_controller_callback(
                            registration,
                            &*context.adapter,
                            context.scheduler,
                            context.capabilities,
                            context.stats,
                            observed.observed_at,
                        )?;
                    }
                }
            }
            MotionRuntimeEvent::LoopCancelled { trigger } => {
                context.stats.record_motion_repeat_cancel();
                context.last_repeat_ticks.remove(&trigger);
                if let Some(registration) =
                    matching_controller_motion_registration(context.program, &trigger)?
                {
                    if let Some(after_callback) = registration
                        .loop_policy
                        .as_ref()
                        .and_then(|loop_policy| loop_policy.after_callback.as_ref())
                    {
                        consumed |= schedule_controller_callback_name(
                            registration,
                            after_callback,
                            &*context.adapter,
                            context.scheduler,
                            context.capabilities,
                            context.stats,
                            observed.observed_at,
                        )?;
                    }
                }
            }
            MotionRuntimeEvent::MotionDiscarded { .. } => context.stats.record_motion_discard(),
        }
    }
    if observed.grabbed && !consumed {
        context.adapter.passthrough_raw_input(&observed.raw)?;
    }
    if event.token == MotionToken::Leader && event.state == MotionInputState::Released {
        context.adapter.release_input_grab()?;
    }
    Ok(())
}

fn matching_controller_press_registration<'a>(
    program: &'a ControllerProgram,
    event: &MotionInputEvent,
    motion_runtime: &MotionRuntime,
) -> Result<Option<&'a ControllerRegistration>, DiagnosableError> {
    for registration in program
        .registrations()
        .registrations()
        .iter()
        .filter(|registration| registration.kind == ControllerRegistrationKind::Press)
    {
        if controller_press_token(registration)? == event.token
            && motion_runtime.held_satisfies(&registration.requires_held)
        {
            return Ok(Some(registration));
        }
    }
    Ok(None)
}

fn matching_controller_motion_registration<'a>(
    program: &'a ControllerProgram,
    trigger: &MotionTrigger,
) -> Result<Option<&'a ControllerRegistration>, DiagnosableError> {
    for registration in program
        .registrations()
        .registrations()
        .iter()
        .filter(|registration| registration.kind == ControllerRegistrationKind::Motion)
    {
        if controller_motion_trigger(registration)? == *trigger {
            return Ok(Some(registration));
        }
    }
    Ok(None)
}

fn drain_live_controller_shortcut_callbacks(
    program: &ControllerProgram,
    adapter: &mut RealWaylandAdapter,
    scheduler: &mut LuaCallbackScheduler,
    capabilities: &CapabilityReport,
    stats: &mut RuntimeStats,
    log: RuntimeLog,
) -> Result<(), DiagnosableError> {
    let dropped = adapter.take_callback_dropped_count();
    if dropped > 0 {
        stats.record_callback_dropped(dropped);
        log.warn(format!(
            "event=controller_callback_burst_limited disposition=dropped count={dropped}"
        ));
    }

    while let Some(event) = adapter.next_shortcut_event() {
        stats.record_callback_received();
        let dispatch_latency_ms = event.received_at.elapsed().as_millis() as u64;
        stats.record_callback_dispatched(dispatch_latency_ms);
        log.debug(format!(
            "event=controller_callback_received hotkey={} dispatch_latency_ms={dispatch_latency_ms}",
            event.hotkey.as_str()
        ));
        if let Some(registration) = controller_registration_for_hotkey(program, &event.hotkey) {
            let _ = schedule_controller_callback(
                registration,
                &*adapter,
                scheduler,
                capabilities,
                stats,
                event.received_at,
            )?;
        } else {
            stats.record_shortcut_ignored();
            log.debug(format!(
                "event=controller_callback_ignored hotkey={} reason=unregistered",
                event.hotkey.as_str()
            ));
        }
    }
    Ok(())
}

fn drain_live_shortcut_callbacks(
    bindings: &[HotkeyBinding],
    adapter: &mut RealWaylandAdapter,
    macro_queue: &mut LiveMacroQueue,
    focus_tracker: &mut ScopedFocusTracker,
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
            schedule_live_binding(
                binding,
                active_context,
                macro_queue,
                focus_tracker,
                stats,
                log,
            )?;
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
    scope: ScopeSelection,
    state: MacroRunState,
}

#[derive(Default)]
struct ScopedFocusTracker {
    active: Option<bool>,
    deactivated: bool,
}

impl ScopedFocusTracker {
    fn observe(
        &mut self,
        scope: &ScopeSelection,
        active_context: &signal_auras_core::ActiveProcessContext,
        policy: FocusFreshnessPolicy,
        log: RuntimeLog,
    ) -> bool {
        if matches!(scope, ScopeSelection::ExplicitGlobal) {
            return false;
        }
        let state = scope.scoped_focus_state_at_with_policy(active_context, Instant::now(), policy);
        let previous = self.active.replace(state.is_active());
        if previous == Some(state.is_active()) {
            return false;
        }
        log.info(scoped_focus_transition_log_message(&state));
        let deactivated = previous == Some(true) && !state.is_active();
        if deactivated {
            self.deactivated = true;
        }
        deactivated
    }

    fn take_deactivation(&mut self) -> bool {
        let deactivated = self.deactivated;
        self.deactivated = false;
        deactivated
    }
}

fn scoped_focus_transition_log_message(state: &signal_auras_core::ScopedFocusState) -> String {
    format!(
        "event=scoped_focus_transition {}",
        state.transition_fields()
    )
}

impl LiveMacroQueue {
    fn schedule(
        &mut self,
        trigger_label: String,
        scope: ScopeSelection,
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
            scope,
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

    fn cancel_process_scoped(&mut self) -> usize {
        let mut cancelled = 0;
        for run in &mut self.runs {
            if matches!(run.scope, ScopeSelection::ProcessList { .. }) && !run.state.is_cancelled()
            {
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
        if !motion_runtime.loop_is_active(trigger) {
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

struct LiveMotionInputContext<'a> {
    motions: &'a [RuntimeMotion],
    presses: &'a [RuntimePress],
    adapter: &'a mut RealWaylandAdapter,
    macro_queue: &'a mut LiveMacroQueue,
    motion_runtime: &'a mut MotionRuntime,
    focus_tracker: &'a mut ScopedFocusTracker,
    stats: &'a mut RuntimeStats,
    log: RuntimeLog,
    motion_time_base: Instant,
}

fn handle_observed_motion_input(
    observed: signal_auras_wayland::evdev::ObservedMotionInputEvent,
    context: &mut LiveMotionInputContext<'_>,
) -> Result<bool, DiagnosableError> {
    let (dispatch_after_read_latency_ms, event_age_ms) = record_motion_latency_metrics(
        context.stats,
        observed.observed_at,
        observed.kernel_timestamp,
    );
    context.log.debug(motion_input_debug_message(
        &observed,
        dispatch_after_read_latency_ms,
        event_age_ms,
    ));
    let mut consumed = false;
    let event_time = motion_runtime_event_time(
        observed.kernel_timestamp,
        observed.observed_at,
        context.motion_time_base,
    );
    let input_event = observed.event.clone();
    let motion_events = context
        .motion_runtime
        .handle_input_at(observed.event, event_time);
    if let Some(press) = matching_press(&input_event, context.presses, context.motion_runtime) {
        let active_context = context.adapter.active_process_context()?;
        context.log.debug(format!(
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
        consumed |= schedule_live_press(
            press,
            active_context,
            context.macro_queue,
            context.focus_tracker,
            context.stats,
            context.log,
        )?;
        if context.focus_tracker.take_deactivation() {
            for cancelled in context.motion_runtime.cancel_active_loops() {
                context.stats.record_motion_repeat_cancel();
                schedule_live_motion_loop_after(
                    &cancelled,
                    context.motions,
                    context.macro_queue,
                    context.stats,
                );
                context.log.debug(format!(
                    "event=motion_repeat_cancelled trigger={} reason=scoped_focus_inactive",
                    trigger_label_for_log(&cancelled)
                ));
            }
        }
    }
    for event in motion_events {
        match &event {
            MotionRuntimeEvent::Triggered {
                trigger,
                starts_loop,
            } => {
                context.log.debug(format!(
                    "event=motion_triggered trigger={} starts_loop={starts_loop}",
                    trigger_label_for_log(trigger)
                ));
            }
            MotionRuntimeEvent::LoopCancelled { trigger } => {
                context.log.debug(format!(
                    "event=motion_loop_cancelled trigger={}",
                    trigger_label_for_log(trigger)
                ));
            }
            MotionRuntimeEvent::MotionDiscarded { reason } => {
                context.log.debug(format!(
                    "event=motion_discarded reason={}",
                    motion_discard_reason_label(*reason)
                ));
                context.stats.record_motion_discard();
                continue;
            }
        }
        let active_context = context.adapter.active_process_context()?;
        context.log.debug(format!(
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
        consumed |= schedule_live_motion_runtime_event(
            event,
            context.motions,
            active_context,
            context.macro_queue,
            context.focus_tracker,
            context.stats,
            context.log,
        )?;
        if context.focus_tracker.take_deactivation() {
            for cancelled in context.motion_runtime.cancel_active_loops() {
                context.stats.record_motion_repeat_cancel();
                schedule_live_motion_loop_after(
                    &cancelled,
                    context.motions,
                    context.macro_queue,
                    context.stats,
                );
                context.log.debug(format!(
                    "event=motion_repeat_cancelled trigger={} reason=scoped_focus_inactive",
                    trigger_label_for_log(&cancelled)
                ));
            }
        }
    }
    Ok(consumed)
}

fn handle_observed_input(
    observed: signal_auras_wayland::evdev::ObservedInputEvent,
    context: &mut LiveMotionInputContext<'_>,
) -> Result<(), DiagnosableError> {
    let Some(event) = observed.event.clone() else {
        if observed.grabbed {
            context.adapter.passthrough_raw_input(&observed.raw)?;
        }
        return Ok(());
    };
    if event.token == MotionToken::Leader && event.state == MotionInputState::Pressed {
        context.adapter.arm_input_grab()?;
    }
    let consumed = handle_observed_motion_input(
        signal_auras_wayland::evdev::ObservedMotionInputEvent {
            event: event.clone(),
            source: observed.source.clone(),
            kernel_timestamp: observed.raw.kernel_timestamp,
            observed_at: observed.observed_at,
        },
        context,
    )?;
    if observed.grabbed && !consumed {
        context.adapter.passthrough_raw_input(&observed.raw)?;
    }
    if event.token == MotionToken::Leader && event.state == MotionInputState::Released {
        context.adapter.release_input_grab()?;
    }
    Ok(())
}

fn motion_runtime_event_time(
    kernel_timestamp: KernelEventTimestamp,
    observed_at: Instant,
    motion_time_base: Instant,
) -> Duration {
    match kernel_timestamp {
        KernelEventTimestamp::Monotonic(timestamp) => timestamp,
        KernelEventTimestamp::Unavailable => observed_at
            .checked_duration_since(motion_time_base)
            .unwrap_or(Duration::ZERO),
    }
}

fn motion_input_state_label(state: MotionInputState) -> &'static str {
    match state {
        MotionInputState::Pressed => "pressed",
        MotionInputState::Released => "released",
    }
}

fn motion_discard_reason_label(reason: MotionDiscardReason) -> &'static str {
    match reason {
        MotionDiscardReason::Timeout => "timeout",
        MotionDiscardReason::UnrelatedPress => "unrelated_press",
        MotionDiscardReason::PreconditionReleased => "precondition_released",
    }
}

fn record_motion_latency_metrics(
    stats: &mut RuntimeStats,
    observed_at: Instant,
    kernel_timestamp: KernelEventTimestamp,
) -> (u64, Option<u64>) {
    let dispatch_after_read_latency_ms = duration_millis_u64(observed_at.elapsed());
    stats.record_motion_input_event(dispatch_after_read_latency_ms);
    let event_age_ms = kernel_timestamp.event_age_now().map(duration_millis_u64);
    match event_age_ms {
        Some(age_ms) => stats.record_motion_event_age(age_ms),
        None => stats.record_motion_event_age_unavailable(),
    }
    (dispatch_after_read_latency_ms, event_age_ms)
}

fn duration_millis_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn motion_input_debug_message(
    observed: &signal_auras_wayland::evdev::ObservedMotionInputEvent,
    dispatch_after_read_latency_ms: u64,
    event_age_ms: Option<u64>,
) -> String {
    let event_age = event_age_ms
        .map(|age_ms| age_ms.to_string())
        .unwrap_or_else(|| "unavailable".to_string());
    format!(
        "event=motion_input token={} state={} source={} dispatch_after_read_latency_ms={dispatch_after_read_latency_ms} event_age_ms={event_age}",
        observed.event.token.describe(),
        motion_input_state_label(observed.event.state),
        observed.source.display()
    )
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
    focus_tracker: &mut ScopedFocusTracker,
    stats: &mut RuntimeStats,
    log: RuntimeLog,
) -> Result<(), DiagnosableError> {
    let trigger_label = binding.trigger_label();
    if focus_tracker.observe(
        &binding.scope,
        &active_context,
        FocusFreshnessPolicy::default(),
        log,
    ) {
        let cancelled = macro_queue.cancel_process_scoped();
        stats.record_cancelled_macro_runs(cancelled as u64);
    }
    let state = binding.scope.scoped_focus_state(&active_context);
    if !state.is_active() {
        if let Some(diagnostic) = state.diagnostic {
            record_scope_denial(stats, &diagnostic);
            log.info(format!(
                "event=scoped_focus_transition trigger={} reason={} {} disposition=denied",
                trigger_label,
                state.reason.as_str(),
                diagnostic.render_fields()
            ));
        }
    } else {
        stats.record_trigger(&trigger_label);
        match binding.mode {
            BindingMode::Consume => stats.record_consumed_trigger_event(),
            BindingMode::Passthrough => stats.record_passthrough_trigger_event(),
        }
        stats.record_active_process_match();
        macro_queue.schedule(
            trigger_label,
            binding.scope.clone(),
            &binding.macro_definition,
            0,
            stats,
        );
    }
    Ok(())
}

fn schedule_live_motion_runtime_event(
    event: MotionRuntimeEvent,
    motions: &[RuntimeMotion],
    active_context: signal_auras_core::ActiveProcessContext,
    macro_queue: &mut LiveMacroQueue,
    focus_tracker: &mut ScopedFocusTracker,
    stats: &mut RuntimeStats,
    log: RuntimeLog,
) -> Result<bool, DiagnosableError> {
    match event {
        MotionRuntimeEvent::Triggered {
            trigger,
            starts_loop,
        } => {
            let Some(motion) = motions
                .iter()
                .find(|motion| motion.definition.trigger == trigger)
            else {
                return Ok(false);
            };
            schedule_live_motion_trigger(
                motion,
                active_context,
                macro_queue,
                focus_tracker,
                stats,
                log,
                starts_loop,
            )
        }
        MotionRuntimeEvent::LoopCancelled { trigger } => {
            stats.record_motion_repeat_cancel();
            let trigger_label = format!("{} repeat", trigger.describe());
            let cancelled = macro_queue.cancel_repeat(&trigger_label);
            let before_label = format!("{} loop_before", trigger.describe());
            let once_label = format!("{} loop_once", trigger.describe());
            let cancelled = cancelled
                + macro_queue.cancel_trigger(&before_label)
                + macro_queue.cancel_trigger(&once_label);
            stats.record_cancelled_macro_runs(cancelled as u64);
            schedule_live_motion_loop_after(&trigger, motions, macro_queue, stats);
            log.debug(format!(
                "event=motion_input trigger={} disposition=cancelled queued_macros_cancelled={cancelled}",
                trigger_label_for_log(&trigger)
            ));
            Ok(true)
        }
        MotionRuntimeEvent::MotionDiscarded { .. } => {
            stats.record_motion_discard();
            Ok(false)
        }
    }
}

fn matching_press<'a>(
    event: &MotionInputEvent,
    presses: &'a [RuntimePress],
    motion_runtime: &MotionRuntime,
) -> Option<&'a RuntimePress> {
    if event.state != MotionInputState::Pressed {
        return None;
    }
    presses.iter().find(|press| {
        press.definition.trigger == event.token
            && motion_runtime.held_satisfies(&press.definition.requires_held)
    })
}

fn schedule_live_press(
    press: &RuntimePress,
    active_context: signal_auras_core::ActiveProcessContext,
    macro_queue: &mut LiveMacroQueue,
    focus_tracker: &mut ScopedFocusTracker,
    stats: &mut RuntimeStats,
    log: RuntimeLog,
) -> Result<bool, DiagnosableError> {
    let trigger_label = format!("press {}", press.definition.trigger.describe());
    if focus_tracker.observe(
        &press.scope,
        &active_context,
        FocusFreshnessPolicy::new(MOTION_FOCUS_STALE_THRESHOLD),
        log,
    ) {
        let cancelled = macro_queue.cancel_process_scoped();
        stats.record_cancelled_macro_runs(cancelled as u64);
    }
    let state = press.scope.scoped_focus_state_at_with_policy(
        &active_context,
        Instant::now(),
        FocusFreshnessPolicy::new(MOTION_FOCUS_STALE_THRESHOLD),
    );
    if !state.is_active() {
        if let Some(diagnostic) = state.diagnostic {
            record_scope_denial(stats, &diagnostic);
            log.info(format!(
                "event=scoped_focus_transition trigger={} reason={} {} disposition=denied",
                trigger_label,
                state.reason.as_str(),
                diagnostic.render_fields()
            ));
        }
        return Ok(false);
    }
    stats.record_trigger(&trigger_label);
    match press.definition.mode {
        BindingMode::Consume => stats.record_consumed_trigger_event(),
        BindingMode::Passthrough => stats.record_passthrough_trigger_event(),
    }
    stats.record_active_process_match();
    macro_queue.schedule(
        trigger_label,
        press.scope.clone(),
        &press.definition.macro_definition,
        press.definition.inter_action_delay_ms,
        stats,
    );
    Ok(press.definition.mode == BindingMode::Consume)
}

fn schedule_live_motion_loop_after(
    trigger: &MotionTrigger,
    motions: &[RuntimeMotion],
    macro_queue: &mut LiveMacroQueue,
    stats: &mut RuntimeStats,
) {
    if let Some(motion) = motions
        .iter()
        .find(|motion| motion.definition.trigger == *trigger)
    {
        if let Some(after) = motion
            .definition
            .loop_definition
            .as_ref()
            .and_then(|loop_definition| loop_definition.after.as_ref())
        {
            macro_queue.schedule(
                format!("{} loop_after", trigger.describe()),
                motion.scope.clone(),
                after,
                motion.definition.inter_action_delay_ms,
                stats,
            );
        }
    }
}

fn schedule_live_motion_trigger(
    motion: &RuntimeMotion,
    active_context: signal_auras_core::ActiveProcessContext,
    macro_queue: &mut LiveMacroQueue,
    focus_tracker: &mut ScopedFocusTracker,
    stats: &mut RuntimeStats,
    log: RuntimeLog,
    starts_loop: bool,
) -> Result<bool, DiagnosableError> {
    let trigger_label = motion.definition.trigger.describe();
    if focus_tracker.observe(
        &motion.scope,
        &active_context,
        FocusFreshnessPolicy::new(MOTION_FOCUS_STALE_THRESHOLD),
        log,
    ) {
        let cancelled = macro_queue.cancel_process_scoped();
        stats.record_cancelled_macro_runs(cancelled as u64);
    }
    let state = motion.scope.scoped_focus_state_at_with_policy(
        &active_context,
        Instant::now(),
        FocusFreshnessPolicy::new(MOTION_FOCUS_STALE_THRESHOLD),
    );
    if !state.is_active() {
        if let Some(diagnostic) = state.diagnostic {
            record_scope_denial(stats, &diagnostic);
            log.info(format!(
                "event=scoped_focus_transition trigger={} reason={} {} disposition=denied",
                trigger_label,
                state.reason.as_str(),
                diagnostic.render_fields()
            ));
        }
        Ok(false)
    } else {
        stats.record_trigger(&trigger_label);
        match motion.definition.mode {
            BindingMode::Consume => stats.record_consumed_trigger_event(),
            BindingMode::Passthrough => stats.record_passthrough_trigger_event(),
        }
        stats.record_active_process_match();
        if let Some(macro_definition) = &motion.definition.macro_definition {
            macro_queue.schedule(
                trigger_label,
                motion.scope.clone(),
                macro_definition,
                motion.definition.inter_action_delay_ms,
                stats,
            );
        }
        if starts_loop {
            schedule_live_motion_loop_start(motion, macro_queue, stats)?;
        }
        Ok(true)
    }
}

fn schedule_live_motion_loop_start(
    motion: &RuntimeMotion,
    macro_queue: &mut LiveMacroQueue,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError> {
    let Some(loop_definition) = &motion.definition.loop_definition else {
        return Ok(());
    };
    match &loop_definition.body {
        LoopBody::Once(body) => {
            let definition = combine_optional_and_required(loop_definition.before.as_ref(), body)?;
            macro_queue.schedule(
                format!("{} loop_once", motion.definition.trigger.describe()),
                motion.scope.clone(),
                &definition,
                motion.definition.inter_action_delay_ms,
                stats,
            );
        }
        LoopBody::Repeat(_) => {
            if let Some(before) = &loop_definition.before {
                macro_queue.schedule(
                    format!("{} loop_before", motion.definition.trigger.describe()),
                    motion.scope.clone(),
                    before,
                    motion.definition.inter_action_delay_ms,
                    stats,
                );
            }
        }
    }
    Ok(())
}

fn combine_optional_and_required(
    before: Option<&MacroDefinition>,
    body: &MacroDefinition,
) -> Result<MacroDefinition, DiagnosableError> {
    let mut actions = Vec::new();
    if let Some(before) = before {
        actions.extend_from_slice(before.actions());
    }
    actions.extend_from_slice(body.actions());
    MacroDefinition::new(actions)
}

fn schedule_live_motion_repeat_tick(
    motion: &RuntimeMotion,
    active_context: signal_auras_core::ActiveProcessContext,
    macro_queue: &mut LiveMacroQueue,
    focus_tracker: &mut ScopedFocusTracker,
    stats: &mut RuntimeStats,
    log: RuntimeLog,
) -> Result<(), DiagnosableError> {
    let Some(loop_definition) = &motion.definition.loop_definition else {
        return Ok(());
    };
    let Some(repeat) = loop_definition.repeat() else {
        return Ok(());
    };
    let trigger_label = format!("{} repeat", motion.definition.trigger.describe());
    let before_label = format!("{} loop_before", motion.definition.trigger.describe());
    if macro_queue.trigger_is_pending_or_active(&before_label) {
        return Ok(());
    }
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
    if focus_tracker.observe(
        &motion.scope,
        &active_context,
        FocusFreshnessPolicy::new(MOTION_FOCUS_STALE_THRESHOLD),
        log,
    ) {
        let cancelled = macro_queue.cancel_process_scoped();
        stats.record_cancelled_macro_runs(cancelled as u64);
    }
    let state = motion.scope.scoped_focus_state_at_with_policy(
        &active_context,
        Instant::now(),
        FocusFreshnessPolicy::new(MOTION_FOCUS_STALE_THRESHOLD),
    );
    if !state.is_active() {
        if let Some(diagnostic) = state.diagnostic {
            record_scope_denial(stats, &diagnostic);
            log.info(format!(
                "event=repeat_overload trigger={} reason={} {} disposition=denied",
                trigger_label,
                state.reason.as_str(),
                diagnostic.render_fields()
            ));
        }
    } else {
        stats.record_active_process_match();
        stats.record_motion_repeat_tick();
        log.debug(format!(
            "event=motion_repeat_tick_scheduled trigger={} disposition=executed",
            trigger_label_for_log(&motion.definition.trigger)
        ));
        macro_queue.schedule(
            trigger_label,
            motion.scope.clone(),
            &repeat.macro_definition,
            motion.definition.inter_action_delay_ms,
            stats,
        );
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

#[cfg(test)]
fn decide_motion_scope(
    scope: &ScopeSelection,
    active_context: &signal_auras_core::ActiveProcessContext,
) -> signal_auras_core::ScopeDecision {
    scope.decide_context_with_policy(
        active_context,
        FocusFreshnessPolicy::new(MOTION_FOCUS_STALE_THRESHOLD),
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
    let state = binding.scope.scoped_focus_state(&active_context);
    if !state.is_active() {
        if let Some(diagnostic) = state.diagnostic {
            record_scope_denial(stats, &diagnostic);
            tracing::info!(
                event = "scoped_focus_transition",
                trigger = %trigger_label,
                reason = state.reason.as_str(),
                details = %diagnostic.render_fields(),
                disposition = "denied"
            );
        }
        return Ok(());
    }
    stats.record_trigger(&trigger_label);
    match binding.mode {
        BindingMode::Consume => stats.record_consumed_trigger_event(),
        BindingMode::Passthrough => stats.record_passthrough_trigger_event(),
    }
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
        MotionRuntimeEvent::Triggered {
            trigger,
            starts_loop,
        } => {
            let Some(motion) = motions
                .iter()
                .find(|motion| motion.definition.trigger == trigger)
            else {
                return Ok(());
            };
            handle_motion_trigger(
                motion,
                active_process_provider,
                executor,
                scheduler,
                stats,
                starts_loop,
            )
        }
        MotionRuntimeEvent::LoopCancelled { trigger } => {
            stats.record_motion_repeat_cancel();
            tracing::debug!(
                event = "motion_input",
                trigger = %trigger.describe(),
                disposition = "cancelled"
            );
            Ok(())
        }
        MotionRuntimeEvent::MotionDiscarded { .. } => {
            stats.record_motion_discard();
            Ok(())
        }
    }
}

fn handle_press_input<P, E>(
    event: &MotionInputEvent,
    presses: &[RuntimePress],
    motion_runtime: &MotionRuntime,
    active_process_provider: &P,
    executor: &mut E,
    scheduler: &mut MacroScheduler,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError>
where
    P: ActiveProcessProvider,
    E: MacroExecutor,
{
    let Some(press) = matching_press(event, presses, motion_runtime) else {
        return Ok(());
    };
    let active_context = active_process_provider.active_process_context()?;
    handle_press_with_context(press, active_context, executor, scheduler, stats)
}

fn handle_press_with_context<E>(
    press: &RuntimePress,
    active_context: signal_auras_core::ActiveProcessContext,
    executor: &mut E,
    scheduler: &mut MacroScheduler,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError>
where
    E: MacroExecutor,
{
    let trigger_label = format!("press {}", press.definition.trigger.describe());
    let state = press.scope.scoped_focus_state_at_with_policy(
        &active_context,
        Instant::now(),
        FocusFreshnessPolicy::new(MOTION_FOCUS_STALE_THRESHOLD),
    );
    if !state.is_active() {
        if let Some(diagnostic) = state.diagnostic {
            record_scope_denial(stats, &diagnostic);
            tracing::info!(
                event = "scoped_focus_transition",
                trigger = %trigger_label,
                reason = state.reason.as_str(),
                details = %diagnostic.render_fields(),
                disposition = "denied"
            );
        }
        return Ok(());
    }
    stats.record_trigger(&trigger_label);
    match press.definition.mode {
        BindingMode::Consume => stats.record_consumed_trigger_event(),
        BindingMode::Passthrough => stats.record_passthrough_trigger_event(),
    }
    stats.record_active_process_match();
    execute_motion_macro(
        &trigger_label,
        &press.definition.macro_definition,
        press.definition.inter_action_delay_ms,
        executor,
        scheduler,
        stats,
    )
}

fn handle_motion_trigger<P, E>(
    motion: &RuntimeMotion,
    active_process_provider: &P,
    executor: &mut E,
    scheduler: &mut MacroScheduler,
    stats: &mut RuntimeStats,
    starts_loop: bool,
) -> Result<(), DiagnosableError>
where
    P: ActiveProcessProvider,
    E: MacroExecutor,
{
    let active_context = active_process_provider.active_process_context()?;
    handle_motion_trigger_with_context(
        motion,
        active_context,
        executor,
        scheduler,
        stats,
        starts_loop,
    )
}

fn handle_motion_trigger_with_context<E>(
    motion: &RuntimeMotion,
    active_context: signal_auras_core::ActiveProcessContext,
    executor: &mut E,
    scheduler: &mut MacroScheduler,
    stats: &mut RuntimeStats,
    starts_loop: bool,
) -> Result<(), DiagnosableError>
where
    E: MacroExecutor,
{
    let trigger_label = motion.definition.trigger.describe();
    let state = motion.scope.scoped_focus_state_at_with_policy(
        &active_context,
        Instant::now(),
        FocusFreshnessPolicy::new(MOTION_FOCUS_STALE_THRESHOLD),
    );
    if !state.is_active() {
        if let Some(diagnostic) = state.diagnostic {
            record_scope_denial(stats, &diagnostic);
            tracing::info!(
                event = "scoped_focus_transition",
                trigger = %trigger_label,
                reason = state.reason.as_str(),
                details = %diagnostic.render_fields(),
                disposition = "denied"
            );
        }
        return Ok(());
    }
    stats.record_trigger(&trigger_label);
    match motion.definition.mode {
        BindingMode::Consume => stats.record_consumed_trigger_event(),
        BindingMode::Passthrough => stats.record_passthrough_trigger_event(),
    }
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
    if starts_loop {
        execute_motion_loop_start(motion, executor, scheduler, stats)?;
    }
    Ok(())
}

fn execute_motion_loop_start<E>(
    motion: &RuntimeMotion,
    executor: &mut E,
    scheduler: &mut MacroScheduler,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError>
where
    E: MacroExecutor,
{
    let Some(loop_definition) = &motion.definition.loop_definition else {
        return Ok(());
    };
    match &loop_definition.body {
        LoopBody::Once(body) => {
            let definition = combine_optional_and_required(loop_definition.before.as_ref(), body)?;
            execute_motion_macro(
                &format!("{} loop_once", motion.definition.trigger.describe()),
                &definition,
                motion.definition.inter_action_delay_ms,
                executor,
                scheduler,
                stats,
            )?;
        }
        LoopBody::Repeat(_) => {
            if let Some(before) = &loop_definition.before {
                execute_motion_macro(
                    &format!("{} loop_before", motion.definition.trigger.describe()),
                    before,
                    motion.definition.inter_action_delay_ms,
                    executor,
                    scheduler,
                    stats,
                )?;
            }
        }
    }
    Ok(())
}

fn execute_motion_loop_after<E>(
    motion: &RuntimeMotion,
    executor: &mut E,
    scheduler: &mut MacroScheduler,
    stats: &mut RuntimeStats,
) -> Result<(), DiagnosableError>
where
    E: MacroExecutor,
{
    let Some(after) = motion
        .definition
        .loop_definition
        .as_ref()
        .and_then(|loop_definition| loop_definition.after.as_ref())
    else {
        return Ok(());
    };
    execute_motion_macro(
        &format!("{} loop_after", motion.definition.trigger.describe()),
        after,
        motion.definition.inter_action_delay_ms,
        executor,
        scheduler,
        stats,
    )
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
    if motion
        .definition
        .loop_definition
        .as_ref()
        .and_then(|loop_definition| loop_definition.repeat())
        .is_none()
    {
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
    let Some(repeat) = motion
        .definition
        .loop_definition
        .as_ref()
        .and_then(|loop_definition| loop_definition.repeat())
    else {
        return Ok(());
    };
    let trigger_label = format!("{} repeat", motion.definition.trigger.describe());
    let state = motion.scope.scoped_focus_state_at_with_policy(
        &active_context,
        Instant::now(),
        FocusFreshnessPolicy::new(MOTION_FOCUS_STALE_THRESHOLD),
    );
    if !state.is_active() {
        if let Some(diagnostic) = state.diagnostic {
            record_scope_denial(stats, &diagnostic);
            tracing::info!(
                event = "repeat_overload",
                trigger = %trigger_label,
                reason = state.reason.as_str(),
                details = %diagnostic.render_fields(),
                disposition = "denied"
            );
        }
        return Ok(());
    }
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
        let rendered = RuntimeLog::from_config(RuntimeLogConfig {
            verbose: true,
            level: None,
            format: RuntimeLogFormat::Pretty,
            color_mode: ColorMode::Never,
        })
        .render_plain("DEBUG", "event=motion_input token=<LClick>");

        assert!(!rendered.contains('\x1b'));
        assert!(rendered.contains("DEBUG"));
        assert!(rendered.contains("motion_input"));
        assert!(rendered.contains("token=<LClick>"));
    }

    #[test]
    fn motion_input_debug_message_distinguishes_latency_labels() {
        let observed = signal_auras_wayland::evdev::ObservedMotionInputEvent {
            event: MotionInputEvent::pressed(MotionToken::MouseButton(
                signal_auras_core::MouseButton::Left,
            )),
            source: PathBuf::from("/dev/input/event7"),
            kernel_timestamp: KernelEventTimestamp::monotonic(Duration::from_secs(1)),
            observed_at: Instant::now(),
        };

        let rendered = RuntimeLog::new(true)
            .render_plain("DEBUG", &motion_input_debug_message(&observed, 3, Some(99)));

        assert!(rendered.contains("motion_input"));
        assert!(rendered.contains("dispatch_after_read_latency_ms=3"));
        assert!(rendered.contains("event_age_ms=99"));
        assert!(!rendered.contains("dispatch_latency_ms=3"));
    }

    #[test]
    fn motion_process_scope_uses_longer_stable_focus_window() {
        let scope = ScopeSelection::process_list(vec![signal_auras_core::ProcessName::parse(
            "steam_app_2694490",
        )
        .unwrap()])
        .unwrap();
        let mut context = signal_auras_core::ActiveProcessContext::name_only(
            signal_auras_core::ProcessName::parse("steam_app_2694490").unwrap(),
        );
        context.captured_at = Instant::now()
            - signal_auras_core::DEFAULT_FOCUS_STALE_THRESHOLD
            - Duration::from_millis(500);

        assert!(matches!(
            scope.decide_context(&context),
            signal_auras_core::ScopeDecision::Denied { .. }
        ));
        assert_eq!(
            decide_motion_scope(&scope, &context),
            signal_auras_core::ScopeDecision::Allowed
        );

        context.captured_at =
            Instant::now() - MOTION_FOCUS_STALE_THRESHOLD - Duration::from_millis(1);

        match decide_motion_scope(&scope, &context) {
            signal_auras_core::ScopeDecision::Denied { diagnostic, .. } => {
                assert_eq!(
                    diagnostic.kind,
                    signal_auras_core::ScopeDenialKind::StaleFocus
                );
                assert_eq!(
                    diagnostic.stale_threshold,
                    Some(MOTION_FOCUS_STALE_THRESHOLD)
                );
            }
            signal_auras_core::ScopeDecision::Allowed => {
                panic!("stale motion focus should fail closed")
            }
        }
    }

    #[test]
    fn runtime_log_color_mode_controls_subscriber_ansi() {
        let log = RuntimeLog::from_config(RuntimeLogConfig {
            verbose: true,
            level: None,
            format: RuntimeLogFormat::Pretty,
            color_mode: ColorMode::Always,
        });

        assert_eq!(log.color_mode(), ColorMode::Always);
        assert!(log.color());
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
            "examples/poe2.lua".to_string(),
        ];
        let options = parse_run_args(&args).unwrap();

        assert!(options.log.verbose());
        assert_eq!(options.log.color_mode(), ColorMode::Always);
    }

    #[test]
    fn parses_explicit_log_level_and_format_options() {
        let args = vec![
            "run".to_string(),
            "--verbose".to_string(),
            "--log-level=warn".to_string(),
            "--log-format=compact".to_string(),
            "examples/poe2.lua".to_string(),
        ];
        let options = parse_run_args(&args).unwrap();

        assert_eq!(options.log.config.level, Some(RuntimeLogLevel::Warn));
        assert_eq!(options.log.config.format, RuntimeLogFormat::Compact);
        assert!(!options.log.verbose());
    }

    #[test]
    fn rejects_invalid_log_options() {
        let error = parse_run_args(&[
            "run".to_string(),
            "--log-level=chatty".to_string(),
            "examples/poe2.lua".to_string(),
        ])
        .unwrap_err();

        assert!(error.to_string().contains("invalid log level"));
    }

    #[test]
    fn compact_log_rendering_is_parseable_and_uncolored() {
        let rendered = RuntimeLog::from_config(RuntimeLogConfig {
            verbose: true,
            level: None,
            format: RuntimeLogFormat::Compact,
            color_mode: ColorMode::Always,
        })
        .render_plain(
            "DEBUG",
            "event=capability_probe result=failed hint=check_permissions",
        );

        assert!(rendered.starts_with("runtime_elapsed_ms="));
        assert!(rendered.contains(" level=debug event=capability_probe "));
        assert!(rendered.contains("result=failed hint=check_permissions"));
        assert!(!rendered.contains('\x1b'));
    }

    #[test]
    fn parses_input_doctor_command() {
        let args = vec![
            "doctor".to_string(),
            "input".to_string(),
            "examples/poe2-legacy.lua".to_string(),
        ];
        let options = parse_doctor_args(&args).unwrap();

        assert_eq!(options.command, DoctorCommand::Input);
        assert_eq!(options.lua_file, PathBuf::from("examples/poe2-legacy.lua"));
    }

    #[test]
    fn parses_key_doctor_command() {
        let args = vec![
            "doctor".to_string(),
            "keys".to_string(),
            "examples/poe2-legacy.lua".to_string(),
        ];
        let options = parse_doctor_args(&args).unwrap();

        assert_eq!(options.command, DoctorCommand::Keys);
        assert_eq!(options.lua_file, PathBuf::from("examples/poe2-legacy.lua"));
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
    fn key_doctor_reports_observed_tokens_aliases_and_support() {
        let lua_file = write_doctor_lua(
            r#"
            return {
              input_provider = {
                backend = "evdev",
                mode = "observe",
                output = "uinput",
                devices = { "/dev/input/event9" },
              },
              motions = {
                {
                  trigger = { "PageUp" },
                  macro = macro { key "PageDown" },
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
        probe
            .read_write
            .insert(PathBuf::from("/dev/uinput"), InputPathStatus::Accessible);
        let observations = vec![
            KeyDiscoveryObservation {
                device: "/dev/input/event9".to_string(),
                raw_code: 104,
            },
            KeyDiscoveryObservation {
                device: "/dev/input/event9".to_string(),
                raw_code: 999,
            },
        ];

        let report =
            key_doctor_report_with_probe_and_observations(&lua_file, &probe, &observations)
                .unwrap();
        let rendered = report.render();

        assert!(report.ok);
        assert!(rendered.contains("persistence=none"));
        assert!(rendered.contains("key device=/dev/input/event9 raw_code=104 token=PageUp"));
        assert!(rendered.contains("aliases=PgUp"));
        assert!(rendered.contains("triggerability=supported emittability=supported"));
        assert!(
            rendered.contains("raw_code=999 token=unknown aliases=none triggerability=unsupported")
        );
    }

    #[test]
    fn key_doctor_reports_no_persistence_between_runs() {
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

        let first = key_doctor_report_with_probe_and_observations(
            &lua_file,
            &probe,
            &[KeyDiscoveryObservation {
                device: "/dev/input/event9".to_string(),
                raw_code: 104,
            }],
        )
        .unwrap()
        .render();
        let second = key_doctor_report_with_probe_and_observations(&lua_file, &probe, &[])
            .unwrap()
            .render();

        assert!(first.contains("token=PageUp"));
        assert!(!second.contains("token=PageUp"));
        assert!(second.contains("observed=none"));
        assert!(second.contains("persistence=none"));
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
        let mut focus_tracker = ScopedFocusTracker::default();
        let mut stats = RuntimeStats::new();

        for _ in 0..=10_000 {
            schedule_live_motion_repeat_tick(
                &motion,
                active_context.clone(),
                &mut macro_queue,
                &mut focus_tracker,
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
        let mut focus_tracker = ScopedFocusTracker::default();
        let mut stats = RuntimeStats::new();

        schedule_live_binding(
            &binding,
            active_context.clone(),
            &mut macro_queue,
            &mut focus_tracker,
            &mut stats,
            RuntimeLog::new(false),
        )
        .unwrap();
        schedule_live_binding(
            &binding,
            active_context,
            &mut macro_queue,
            &mut focus_tracker,
            &mut stats,
            RuntimeLog::new(false),
        )
        .unwrap();

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
        let mut focus_tracker = ScopedFocusTracker::default();
        let mut stats = RuntimeStats::new();
        let mut executor = QueueExecutor::default();

        schedule_live_binding(
            &binding,
            active_context.clone(),
            &mut macro_queue,
            &mut focus_tracker,
            &mut stats,
            RuntimeLog::new(false),
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
            &mut focus_tracker,
            &mut stats,
            RuntimeLog::new(false),
        )
        .unwrap();
        let cancelled = macro_queue.cancel_trigger(&trigger_label);
        macro_queue.drive_ready(&mut executor, &mut stats).unwrap();
        assert_eq!(cancelled, 1);
        assert!(!macro_queue.trigger_is_pending_or_active(&trigger_label));

        schedule_live_binding(
            &binding,
            active_context,
            &mut macro_queue,
            &mut focus_tracker,
            &mut stats,
            RuntimeLog::new(false),
        )
        .unwrap();
        assert!(macro_queue.trigger_is_pending_or_active(&trigger_label));
        assert_eq!(stats.non_repeat_trigger_skipped_count, 0);
    }

    #[test]
    fn repeat_overload_accounting_is_isolated_by_binding() {
        let first = repeat_runtime_motion(MotionTrigger::parse(["<Leader>", "a"]).unwrap(), "a");
        let second = repeat_runtime_motion(MotionTrigger::parse(["<Leader>", "b"]).unwrap(), "b");
        let active_context = signal_auras_core::ActiveProcessContext::unavailable("not needed");
        let mut macro_queue = LiveMacroQueue::default();
        let mut focus_tracker = ScopedFocusTracker::default();
        let mut stats = RuntimeStats::new();

        schedule_live_motion_repeat_tick(
            &first,
            active_context.clone(),
            &mut macro_queue,
            &mut focus_tracker,
            &mut stats,
            RuntimeLog::new(false),
        )
        .unwrap();
        schedule_live_motion_repeat_tick(
            &first,
            active_context.clone(),
            &mut macro_queue,
            &mut focus_tracker,
            &mut stats,
            RuntimeLog::new(false),
        )
        .unwrap();
        schedule_live_motion_repeat_tick(
            &second,
            active_context,
            &mut macro_queue,
            &mut focus_tracker,
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
        let mut focus_tracker = ScopedFocusTracker::default();
        let mut stats = RuntimeStats::new();

        schedule_live_motion_repeat_tick(
            &first,
            active_context.clone(),
            &mut macro_queue,
            &mut focus_tracker,
            &mut stats,
            RuntimeLog::new(false),
        )
        .unwrap();
        schedule_live_motion_repeat_tick(
            &second,
            active_context,
            &mut macro_queue,
            &mut focus_tracker,
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

    #[test]
    fn scoped_focus_transition_log_message_is_info_safe() {
        let scope =
            ScopeSelection::process_list(vec![
                signal_auras_core::ProcessName::parse("kate").unwrap()
            ])
            .unwrap();
        let context = signal_auras_core::ActiveProcessContext::name_only(
            signal_auras_core::ProcessName::parse("konsole").unwrap(),
        )
        .with_app_id("org.kde.konsole --secret")
        .with_window_class("Private Title");
        let state = scope.scoped_focus_state(&context);
        let rendered = RuntimeLog::new(false)
            .render_plain("INFO", &scoped_focus_transition_log_message(&state));

        assert!(rendered.contains("INFO"));
        assert!(rendered.contains("scoped_focus_transition"));
        assert!(rendered.contains("state=inactive"));
        assert!(rendered.contains("reason=process_mismatch"));
        assert!(rendered.contains("configured_rule=processes:kate"));
        assert!(!rendered.contains("--secret"));
        assert!(!rendered.contains("Private Title"));
    }

    #[test]
    fn unchanged_inactive_state_tracker_updates_are_trace_only() {
        let trackers = signal_auras_core::StateTrackerDefinitionSet::default();
        let mut runtime = LiveStateTrackerRuntime::new(StateTrackerPoller::new(trackers));
        let inactive = TrackerState::Inactive {
            reason: signal_auras_core::TrackerInactiveReason::FocusInactive,
            confidence: 0,
            freshness_ms: 0,
        };
        let active = TrackerState::HorizontalProgressBar {
            visible: true,
            progress_percent: 42,
            confidence: 95,
            freshness_ms: 0,
        };

        assert_eq!(
            runtime.log_level_for_update("heavy_stun", &inactive),
            StateTrackerUpdateLogLevel::Info
        );
        assert_eq!(
            runtime.log_level_for_update("heavy_stun", &inactive),
            StateTrackerUpdateLogLevel::Trace
        );
        assert_eq!(
            runtime.log_level_for_update("heavy_stun", &active),
            StateTrackerUpdateLogLevel::Info
        );
        assert_eq!(
            runtime.log_level_for_update("heavy_stun", &active),
            StateTrackerUpdateLogLevel::Info
        );
    }

    #[test]
    fn state_tracker_focus_denial_diagnostics_are_deduped() {
        let trackers = signal_auras_core::StateTrackerDefinitionSet::default();
        let mut runtime = LiveStateTrackerRuntime::new(StateTrackerPoller::new(trackers));

        assert!(runtime.focus_denial_changed(
            "refutation_cooldown",
            "state=inactive reason=process_mismatch configured_rule=processes:poe"
        ));
        assert!(!runtime.focus_denial_changed(
            "refutation_cooldown",
            "state=inactive reason=process_mismatch configured_rule=processes:poe"
        ));
        assert!(runtime.focus_denial_changed(
            "refutation_cooldown",
            "state=inactive reason=stale_focus configured_rule=processes:poe"
        ));
    }

    #[test]
    fn scoped_focus_deactivation_cancels_process_scoped_queue() {
        let binding = HotkeyBinding {
            trigger: BindingTrigger::keyboard(HotkeyId::parse("F5").unwrap()),
            mode: BindingMode::Consume,
            scope: ScopeSelection::process_list(vec![signal_auras_core::ProcessName::parse(
                "kate",
            )
            .unwrap()])
            .unwrap(),
            macro_definition: MacroDefinition::new(vec![signal_auras_core::MacroAction::text(
                "queued",
            )
            .unwrap()])
            .unwrap(),
            registration_state: signal_auras_core::RegistrationState::Registered,
        };
        let matching = signal_auras_core::ActiveProcessContext::name_only(
            signal_auras_core::ProcessName::parse("kate").unwrap(),
        );
        let non_matching = signal_auras_core::ActiveProcessContext::name_only(
            signal_auras_core::ProcessName::parse("konsole").unwrap(),
        );
        let mut macro_queue = LiveMacroQueue::default();
        let mut focus_tracker = ScopedFocusTracker::default();
        let mut stats = RuntimeStats::new();

        schedule_live_binding(
            &binding,
            matching,
            &mut macro_queue,
            &mut focus_tracker,
            &mut stats,
            RuntimeLog::new(false),
        )
        .unwrap();
        assert_eq!(macro_queue.runs.len(), 1);

        schedule_live_binding(
            &binding,
            non_matching,
            &mut macro_queue,
            &mut focus_tracker,
            &mut stats,
            RuntimeLog::new(false),
        )
        .unwrap();
        macro_queue
            .drive_ready(&mut QueueExecutor::default(), &mut stats)
            .unwrap();

        assert_eq!(stats.cancelled_macro_run_count, 1);
        assert_eq!(macro_queue.runs.len(), 0);
        assert_eq!(stats.denied_action_count, 1);
    }

    fn mouse_button_repeat_motion() -> MotionDefinition {
        let macro_definition =
            MacroDefinition::new(vec![signal_auras_core::MacroAction::mouse_click(
                signal_auras_core::MouseButton::Left,
            )])
            .unwrap();
        let loop_definition = signal_auras_core::LoopDefinition::new(
            MotionTrigger::parse(["<LClick>"]).unwrap(),
            None,
            signal_auras_core::LoopBody::Repeat(signal_auras_core::LoopRepeat::new(
                signal_auras_core::LoopInterval::new(50).unwrap(),
                macro_definition,
            )),
            None,
        );
        MotionDefinition::new(
            MotionTrigger::parse(["<Leader>", "<LClick>", "<LClick>"]).unwrap(),
            BindingMode::Passthrough,
            None,
            Some(loop_definition),
            signal_auras_core::DEFAULT_MOTION_DURATION.as_millis() as u64,
            0,
        )
        .unwrap()
    }

    fn repeat_runtime_motion(trigger: MotionTrigger, text: &str) -> RuntimeMotion {
        let macro_definition =
            MacroDefinition::new(vec![signal_auras_core::MacroAction::text(text).unwrap()])
                .unwrap();
        let loop_definition = signal_auras_core::LoopDefinition::new(
            trigger.clone(),
            None,
            signal_auras_core::LoopBody::Repeat(signal_auras_core::LoopRepeat::new(
                signal_auras_core::LoopInterval::new(10).unwrap(),
                macro_definition,
            )),
            None,
        );
        RuntimeMotion {
            definition: MotionDefinition::new(
                trigger,
                BindingMode::Passthrough,
                None,
                Some(loop_definition),
                signal_auras_core::DEFAULT_MOTION_DURATION.as_millis() as u64,
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
