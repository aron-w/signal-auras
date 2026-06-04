#[cfg(test)]
use std::time::Duration;
use std::{
    fmt,
    io::{self, IsTerminal},
    str::FromStr,
    sync::Once,
    time::Instant,
};
use tracing::{field::Visit, Event, Level, Subscriber};
use tracing_appender::non_blocking::{ErrorCounter, NonBlocking, WorkerGuard};
use tracing_subscriber::{
    fmt::{format::Writer, FmtContext, FormatEvent, FormatFields},
    registry::LookupSpan,
    EnvFilter,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeLogConfig {
    pub level: Option<RuntimeLogLevel>,
    pub format: RuntimeLogFormat,
    pub color_mode: ColorMode,
    pub verbose: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeLogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeLogFormat {
    Auto,
    Pretty,
    Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseRuntimeLogValueError {
    message: String,
}

impl fmt::Display for ParseRuntimeLogValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl RuntimeLogConfig {
    pub fn new(verbose: bool) -> Self {
        Self {
            level: None,
            format: RuntimeLogFormat::Auto,
            color_mode: ColorMode::Auto,
            verbose,
        }
    }

    pub fn effective_filter(self) -> EnvFilter {
        if let Some(level) = self.level {
            return EnvFilter::new(level.directive());
        }
        if let Ok(value) = std::env::var("SIGNAL_AURAS_LOG") {
            return EnvFilter::new(value);
        }
        if let Ok(value) = std::env::var("RUST_LOG") {
            return EnvFilter::new(value);
        }
        if self.verbose {
            EnvFilter::new("signal_auras_cli=debug,signal_auras_wayland=debug")
        } else {
            EnvFilter::new("signal_auras_cli=info,signal_auras_wayland=warn")
        }
    }

    pub fn effective_format(self) -> RuntimeLogFormat {
        match self.format {
            RuntimeLogFormat::Auto if io::stderr().is_terminal() => RuntimeLogFormat::Pretty,
            RuntimeLogFormat::Auto => RuntimeLogFormat::Compact,
            format => format,
        }
    }

    pub fn color_enabled(self) -> bool {
        match self.color_mode {
            ColorMode::Auto => {
                self.effective_format() == RuntimeLogFormat::Pretty
                    && io::stderr().is_terminal()
                    && std::env::var_os("NO_COLOR").is_none()
            }
            ColorMode::Always => self.effective_format() == RuntimeLogFormat::Pretty,
            ColorMode::Never => false,
        }
    }
}

impl RuntimeLogLevel {
    fn directive(self) -> &'static str {
        match self {
            RuntimeLogLevel::Off => "off",
            RuntimeLogLevel::Error => "signal_auras_cli=error,signal_auras_wayland=error",
            RuntimeLogLevel::Warn => "signal_auras_cli=warn,signal_auras_wayland=warn",
            RuntimeLogLevel::Info => "signal_auras_cli=info,signal_auras_wayland=info",
            RuntimeLogLevel::Debug => "signal_auras_cli=debug,signal_auras_wayland=debug",
            RuntimeLogLevel::Trace => "signal_auras_cli=trace,signal_auras_wayland=trace",
        }
    }
}

impl FromStr for RuntimeLogLevel {
    type Err = ParseRuntimeLogValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "off" => Ok(Self::Off),
            "error" => Ok(Self::Error),
            "warn" => Ok(Self::Warn),
            "info" => Ok(Self::Info),
            "debug" => Ok(Self::Debug),
            "trace" => Ok(Self::Trace),
            _ => Err(ParseRuntimeLogValueError {
                message: format!(
                    "invalid log level '{value}', expected off|error|warn|info|debug|trace"
                ),
            }),
        }
    }
}

impl FromStr for RuntimeLogFormat {
    type Err = ParseRuntimeLogValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "pretty" => Ok(Self::Pretty),
            "compact" => Ok(Self::Compact),
            _ => Err(ParseRuntimeLogValueError {
                message: format!("invalid log format '{value}', expected auto|pretty|compact"),
            }),
        }
    }
}

impl FromStr for ColorMode {
    type Err = ParseRuntimeLogValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            _ => Err(ParseRuntimeLogValueError {
                message: format!("invalid color mode '{value}', expected auto|always|never"),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeLog {
    pub config: RuntimeLogConfig,
    started_at: Instant,
}

impl PartialEq for RuntimeLog {
    fn eq(&self, other: &Self) -> bool {
        self.config == other.config
    }
}

impl Eq for RuntimeLog {}

impl Default for RuntimeLog {
    fn default() -> Self {
        Self::new(false)
    }
}

impl RuntimeLog {
    pub fn new(verbose: bool) -> Self {
        Self::from_config(RuntimeLogConfig::new(verbose))
    }

    pub fn from_config(config: RuntimeLogConfig) -> Self {
        Self {
            config,
            started_at: Instant::now(),
        }
    }

    pub fn verbose(&self) -> bool {
        self.config.verbose && self.config.level.is_none()
            || matches!(
                self.config.level,
                Some(RuntimeLogLevel::Debug | RuntimeLogLevel::Trace)
            )
    }

    pub fn color_mode(&self) -> ColorMode {
        self.config.color_mode
    }

    pub fn color(&self) -> bool {
        self.config.color_enabled()
    }

    pub fn elapsed_ms(&self) -> u128 {
        self.started_at.elapsed().as_millis()
    }

    pub fn debug(&self, message: impl AsRef<str>) {
        if self.verbose() {
            self.emit(Level::DEBUG, message.as_ref());
        }
    }

    pub fn debug_lazy(&self, message: impl FnOnce() -> String) {
        if self.verbose() {
            self.emit(Level::DEBUG, &message());
        }
    }

    pub fn trace(&self, message: impl AsRef<str>) {
        if matches!(self.config.level, Some(RuntimeLogLevel::Trace)) {
            self.emit(Level::TRACE, message.as_ref());
        }
    }

    pub fn info(&self, message: impl AsRef<str>) {
        self.emit(Level::INFO, message.as_ref());
    }

    pub fn warn(&self, message: impl AsRef<str>) {
        self.emit(Level::WARN, message.as_ref());
    }

    fn emit(&self, level: Level, message: &str) {
        let fields = RuntimeLogFields::from_compat_message(message);
        let runtime_elapsed_ms = self.elapsed_ms();
        match level {
            Level::ERROR => tracing::error!(
                runtime_elapsed_ms,
                event = fields.event,
                details = fields.details
            ),
            Level::WARN => tracing::warn!(
                runtime_elapsed_ms,
                event = fields.event,
                details = fields.details
            ),
            Level::INFO => tracing::info!(
                runtime_elapsed_ms,
                event = fields.event,
                details = fields.details
            ),
            Level::DEBUG => tracing::debug!(
                runtime_elapsed_ms,
                event = fields.event,
                details = fields.details
            ),
            Level::TRACE => tracing::trace!(
                runtime_elapsed_ms,
                event = fields.event,
                details = fields.details
            ),
        }
    }

    #[cfg(test)]
    pub fn render_plain(&self, level: &'static str, message: &str) -> String {
        let format = match self.config.format {
            RuntimeLogFormat::Auto => RuntimeLogFormat::Pretty,
            format => format,
        };
        RuntimeLogFormatter::new(format, self.config.color_enabled(), self.started_at)
            .render_for_test(
                self.started_at.elapsed(),
                level,
                &RuntimeLogFields::from_compat_message(message),
            )
    }
}

struct RuntimeLogFields<'a> {
    event: &'a str,
    details: String,
}

impl<'a> RuntimeLogFields<'a> {
    fn from_compat_message(message: &'a str) -> Self {
        Self {
            event: field_value(message, "event").unwrap_or("runtime"),
            details: without_field(message, "event"),
        }
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
        .join(" ")
}

pub struct RuntimeLogGuard {
    _guard: WorkerGuard,
    counter: ErrorCounter,
}

impl RuntimeLogGuard {
    pub fn dropped_logs(&self) -> u64 {
        self.counter.dropped_lines() as u64
    }

    pub fn log_summary(&self, log: &RuntimeLog) {
        tracing::info!(
            runtime_elapsed_ms = log.elapsed_ms(),
            event = "logging_summary",
            dropped_logs = self.dropped_logs()
        );
    }
}

static TRACING_INIT: Once = Once::new();

pub fn init_runtime_logging(log: &RuntimeLog) -> RuntimeLogGuard {
    let (writer, guard, counter) = non_blocking_stderr();
    let config = log.config;
    let format = config.effective_format();
    let color = config.color_enabled();
    let filter = config.effective_filter();
    let writer_for_subscriber = writer.clone();
    TRACING_INIT.call_once(|| {
        let subscriber = tracing_subscriber::fmt()
            .event_format(RuntimeLogFormatter::new(format, color, log.started_at))
            .with_env_filter(filter)
            .with_writer(move || writer_for_subscriber.clone())
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);
    });
    RuntimeLogGuard {
        _guard: guard,
        counter,
    }
}

fn non_blocking_stderr() -> (NonBlocking, WorkerGuard, ErrorCounter) {
    let (writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .lossy(true)
        .finish(io::stderr());
    let counter = writer.error_counter();
    (writer, guard, counter)
}

#[derive(Debug, Clone)]
pub struct RuntimeLogFormatter {
    format: RuntimeLogFormat,
    color: bool,
    started_at: Instant,
}

impl RuntimeLogFormatter {
    pub fn new(format: RuntimeLogFormat, color: bool, started_at: Instant) -> Self {
        Self {
            format,
            color,
            started_at,
        }
    }

    #[cfg(test)]
    fn render_for_test(
        &self,
        elapsed: Duration,
        level: &'static str,
        fields: &RuntimeLogFields<'_>,
    ) -> String {
        match self.format {
            RuntimeLogFormat::Pretty | RuntimeLogFormat::Auto => {
                self.render_pretty(elapsed.as_millis(), level, fields.event, &fields.details)
            }
            RuntimeLogFormat::Compact => {
                self.render_compact(elapsed.as_millis(), level, fields.event, &fields.details)
            }
        }
    }

    fn render_pretty(&self, elapsed_ms: u128, level: &str, event: &str, fields: &str) -> String {
        let timestamp = format!("{:>5}.{:03}s", elapsed_ms / 1000, elapsed_ms % 1000);
        let level = if self.color {
            color_level(level)
        } else {
            level.to_string()
        };
        if fields.is_empty() {
            format!("{timestamp}  {level:<5}  {event:<30}")
        } else {
            format!("{timestamp}  {level:<5}  {event:<30}  {fields}")
        }
    }

    fn render_compact(&self, elapsed_ms: u128, level: &str, event: &str, fields: &str) -> String {
        if fields.is_empty() {
            format!(
                "runtime_elapsed_ms={elapsed_ms} level={} event={event}",
                level.to_lowercase()
            )
        } else {
            format!(
                "runtime_elapsed_ms={elapsed_ms} level={} event={event} {fields}",
                level.to_lowercase()
            )
        }
    }
}

fn color_level(level: &str) -> String {
    match level {
        "ERROR" => format!("\x1b[31m{level}\x1b[0m"),
        "WARN" => format!("\x1b[33m{level}\x1b[0m"),
        "INFO" => format!("\x1b[32m{level}\x1b[0m"),
        "DEBUG" => format!("\x1b[34m{level}\x1b[0m"),
        "TRACE" => format!("\x1b[35m{level}\x1b[0m"),
        _ => level.to_string(),
    }
}

impl<S, N> FormatEvent<S, N> for RuntimeLogFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut visitor = EventFieldVisitor::default();
        event.record(&mut visitor);
        let elapsed_ms = visitor
            .runtime_elapsed_ms
            .unwrap_or_else(|| self.started_at.elapsed().as_millis());
        let level = event.metadata().level().as_str();
        let event_name = visitor.event.as_deref().unwrap_or("runtime");
        let fields = visitor.fields.join(" ");
        let rendered = match self.format {
            RuntimeLogFormat::Pretty | RuntimeLogFormat::Auto => {
                self.render_pretty(elapsed_ms, level, event_name, &fields)
            }
            RuntimeLogFormat::Compact => {
                self.render_compact(elapsed_ms, level, event_name, &fields)
            }
        };
        writeln!(writer, "{rendered}")
    }
}

#[derive(Default)]
struct EventFieldVisitor {
    runtime_elapsed_ms: Option<u128>,
    event: Option<String>,
    fields: Vec<String>,
}

impl Visit for EventFieldVisitor {
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.record_u128(field, value.max(0) as u128);
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.record_u128(field, value as u128);
    }

    fn record_u128(&mut self, field: &tracing::field::Field, value: u128) {
        if field.name() == "runtime_elapsed_ms" {
            self.runtime_elapsed_ms = Some(value);
        } else {
            self.fields.push(format!("{}={value}", field.name()));
        }
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "event" {
            self.event = Some(value.to_string());
        } else if field.name() == "details" {
            if !value.is_empty() {
                self.fields.push(value.to_string());
            }
        } else {
            self.fields
                .push(format!("{}={}", field.name(), shell_token(value)));
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        if field.name() == "event" {
            self.event = Some(format!("{value:?}").trim_matches('"').to_string());
        } else {
            self.fields.push(format!("{}={value:?}", field.name()));
        }
    }
}

fn shell_token(value: &str) -> String {
    value.replace(' ', "_")
}
