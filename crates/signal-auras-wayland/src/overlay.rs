use signal_auras_core::{
    CleanupReport, DiagnosableError, ErrorPhase, OverlayDiagnosticReason, OverlayLifecycleState,
    OverlayProviderReport, OverlayProviderStatus, OverlayRect, OverlaySnapshot, RendererProviderId,
    ScreenPixelFormat, ScreenSample, VisualSnapshot,
};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

use crate::capability::KdeEnvironment;
#[cfg(test)]
use crate::capability::KdeServiceAvailability;

const QML_LAUNCHER: &str = "qml";
const QML_POLL_INTERVAL_MS: u64 = 50;
pub const OVERLAY_SMOKE_ID: &str = "signal_auras_overlay_smoke";
const OVERLAY_SMOKE_RECT: OverlayRect = OverlayRect {
    x: 64,
    y: 64,
    w: 260,
    h: 48,
};
const OVERLAY_SMOKE_FILL: &str = "#ff00ff";
const OVERLAY_SMOKE_BACKGROUND: &str = "#000000";
const OVERLAY_SMOKE_PROBE_X: u32 = 96;
const OVERLAY_SMOKE_PROBE_Y: u32 = 78;
const OVERLAY_SMOKE_PROBE_W: u32 = 120;
const OVERLAY_SMOKE_PROBE_H: u32 = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayWindowPlacement {
    pub overlay_id: String,
    pub title: String,
    pub process_id: Option<u32>,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlaySmokePixelReport {
    pub matched_pixels: u32,
    pub sampled_pixels: u32,
    pub sample_width: u32,
    pub sample_height: u32,
    pub pixel_format: ScreenPixelFormat,
}

impl OverlaySmokePixelReport {
    pub fn passed(&self) -> bool {
        self.sampled_pixels > 0
            && self.matched_pixels.saturating_mul(100) >= self.sampled_pixels * 35
    }
}

pub trait OverlayRendererAdapter {
    fn provider(&self) -> RendererProviderId;
    fn mount_or_update(&mut self, snapshot: OverlaySnapshot) -> Result<(), DiagnosableError>;
    fn hide(
        &mut self,
        overlay_id: impl Into<String>,
        reason: OverlayDiagnosticReason,
    ) -> Result<(), DiagnosableError>;
    fn cleanup(&mut self, overlay_id: impl Into<String>) -> Result<(), DiagnosableError>;

    fn render_snapshot(&mut self, snapshot: OverlaySnapshot) -> Result<(), DiagnosableError> {
        if snapshot.provider != self.provider() {
            return Err(DiagnosableError::new(
                ErrorPhase::Registration,
                format!(
                    "overlay renderer '{}' cannot render provider '{}' snapshots",
                    self.provider().as_str(),
                    snapshot.provider.as_str()
                ),
            ));
        }
        if snapshot.is_active() {
            return self.mount_or_update(snapshot);
        }
        let reason = snapshot
            .diagnostic
            .as_ref()
            .map(|diagnostic| diagnostic.reason)
            .unwrap_or(OverlayDiagnosticReason::ProviderUnavailable);
        match snapshot.lifecycle {
            OverlayLifecycleState::CleanedUp => self.cleanup(snapshot.overlay_id),
            _ => self.hide(snapshot.overlay_id, reason),
        }
    }
}

#[derive(Debug)]
pub enum NativeOverlayRenderer {
    InMemory(InMemoryOverlayRenderer),
    Qml(QmlOverlayRenderer),
}

impl NativeOverlayRenderer {
    pub fn live() -> Self {
        Self::Qml(QmlOverlayRenderer::new())
    }

    pub fn in_memory() -> Self {
        Self::InMemory(InMemoryOverlayRenderer::default())
    }

    pub fn provider_report(&self, environment: &KdeEnvironment) -> OverlayProviderReport {
        let native_status = if !native_overlay_environment_available(environment) {
            OverlayProviderStatus::unavailable(
                RendererProviderId::Native,
                "native overlay provider requires a KDE Plasma Wayland session with KWin",
            )
        } else if self.backend_available() {
            OverlayProviderStatus::available(RendererProviderId::Native)
        } else {
            OverlayProviderStatus::unavailable(
                RendererProviderId::Native,
                "native overlay provider requires the Qt qml launcher in PATH",
            )
        };
        OverlayProviderReport::from_statuses([
            native_status,
            OverlayProviderStatus::unavailable(
                RendererProviderId::WebView,
                "WebView overlay provider is declared for future adapter support",
            ),
            OverlayProviderStatus::unavailable(
                RendererProviderId::TauriWindow,
                "Tauri window overlay provider is declared for future adapter support",
            ),
            OverlayProviderStatus::unavailable(
                RendererProviderId::ToolWindow,
                "tool window overlay provider is declared for future adapter support",
            ),
        ])
    }

    pub fn cleanup_all(&mut self) -> Result<CleanupReport, DiagnosableError> {
        match self {
            Self::InMemory(renderer) => renderer.cleanup_all(),
            Self::Qml(renderer) => renderer.cleanup_all(),
        }
    }

    pub fn active_snapshot(&self, overlay_id: &str) -> Option<&OverlaySnapshot> {
        match self {
            Self::InMemory(renderer) => renderer.active_snapshot(overlay_id),
            Self::Qml(renderer) => renderer.active_snapshot(overlay_id),
        }
    }

    pub fn mounted(&self) -> &[OverlaySnapshot] {
        match self {
            Self::InMemory(renderer) => renderer.mounted(),
            Self::Qml(renderer) => renderer.mounted(),
        }
    }

    pub fn hidden(&self) -> &[String] {
        match self {
            Self::InMemory(renderer) => renderer.hidden(),
            Self::Qml(renderer) => renderer.hidden(),
        }
    }

    pub fn cleaned_up(&self) -> &[String] {
        match self {
            Self::InMemory(renderer) => renderer.cleaned_up(),
            Self::Qml(renderer) => renderer.cleaned_up(),
        }
    }

    pub fn last_lifecycle(&self) -> Option<OverlayLifecycleState> {
        self.mounted().last().map(|snapshot| snapshot.lifecycle)
    }

    pub fn overlay_process_id(&self, overlay_id: &str) -> Option<u32> {
        match self {
            Self::InMemory(_) => None,
            Self::Qml(renderer) => renderer.overlay_process_id(overlay_id),
        }
    }

    pub fn runtime_diagnostic(&self, overlay_id: &str) -> Option<String> {
        match self {
            Self::InMemory(_) => None,
            Self::Qml(renderer) => renderer.runtime_diagnostic(overlay_id),
        }
    }

    fn backend_available(&self) -> bool {
        match self {
            Self::InMemory(_) => true,
            Self::Qml(_) => command_in_path(QML_LAUNCHER),
        }
    }
}

impl Default for NativeOverlayRenderer {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl OverlayRendererAdapter for NativeOverlayRenderer {
    fn provider(&self) -> RendererProviderId {
        RendererProviderId::Native
    }

    fn mount_or_update(&mut self, snapshot: OverlaySnapshot) -> Result<(), DiagnosableError> {
        match self {
            Self::InMemory(renderer) => renderer.mount_or_update(snapshot),
            Self::Qml(renderer) => renderer.mount_or_update(snapshot),
        }
    }

    fn hide(
        &mut self,
        overlay_id: impl Into<String>,
        reason: OverlayDiagnosticReason,
    ) -> Result<(), DiagnosableError> {
        match self {
            Self::InMemory(renderer) => renderer.hide(overlay_id, reason),
            Self::Qml(renderer) => renderer.hide(overlay_id, reason),
        }
    }

    fn cleanup(&mut self, overlay_id: impl Into<String>) -> Result<(), DiagnosableError> {
        match self {
            Self::InMemory(renderer) => renderer.cleanup(overlay_id),
            Self::Qml(renderer) => renderer.cleanup(overlay_id),
        }
    }
}

#[derive(Debug, Default)]
pub struct InMemoryOverlayRenderer {
    mounted: Vec<OverlaySnapshot>,
    active: BTreeMap<String, OverlaySnapshot>,
    hidden: Vec<String>,
    cleaned_up: Vec<String>,
}

impl InMemoryOverlayRenderer {
    pub fn mounted(&self) -> &[OverlaySnapshot] {
        &self.mounted
    }

    pub fn hidden(&self) -> &[String] {
        &self.hidden
    }

    pub fn cleaned_up(&self) -> &[String] {
        &self.cleaned_up
    }

    pub fn active_snapshot(&self, overlay_id: &str) -> Option<&OverlaySnapshot> {
        self.active.get(overlay_id)
    }

    pub fn cleanup_all(&mut self) -> Result<CleanupReport, DiagnosableError> {
        let overlay_ids = self.active.keys().cloned().collect::<Vec<_>>();
        for overlay_id in &overlay_ids {
            self.cleanup(overlay_id.clone())?;
        }
        Ok(CleanupReport::all_succeeded(overlay_ids.len()))
    }

    pub fn mount_or_update(&mut self, snapshot: OverlaySnapshot) -> Result<(), DiagnosableError> {
        if snapshot.provider != RendererProviderId::Native {
            return Err(DiagnosableError::new(
                ErrorPhase::Registration,
                "in-memory overlay renderer supports only native provider snapshots",
            ));
        }
        self.active
            .insert(snapshot.overlay_id.clone(), snapshot.clone());
        self.mounted.push(snapshot);
        Ok(())
    }

    pub fn hide(
        &mut self,
        overlay_id: impl Into<String>,
        _reason: OverlayDiagnosticReason,
    ) -> Result<(), DiagnosableError> {
        let overlay_id = overlay_id.into();
        self.active.remove(&overlay_id);
        self.hidden.push(overlay_id);
        Ok(())
    }

    pub fn cleanup(&mut self, overlay_id: impl Into<String>) -> Result<(), DiagnosableError> {
        let overlay_id = overlay_id.into();
        self.active.remove(&overlay_id);
        self.cleaned_up.push(overlay_id);
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct QmlOverlayRenderer {
    mounted: Vec<OverlaySnapshot>,
    active: BTreeMap<String, OverlaySnapshot>,
    processes: BTreeMap<String, QmlOverlayProcess>,
    hidden: Vec<String>,
    cleaned_up: Vec<String>,
}

impl QmlOverlayRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mounted(&self) -> &[OverlaySnapshot] {
        &self.mounted
    }

    pub fn hidden(&self) -> &[String] {
        &self.hidden
    }

    pub fn cleaned_up(&self) -> &[String] {
        &self.cleaned_up
    }

    pub fn active_snapshot(&self, overlay_id: &str) -> Option<&OverlaySnapshot> {
        self.active.get(overlay_id)
    }

    pub fn overlay_process_id(&self, overlay_id: &str) -> Option<u32> {
        self.processes
            .get(overlay_id)
            .and_then(QmlOverlayProcess::process_id)
    }

    pub fn runtime_diagnostic(&self, overlay_id: &str) -> Option<String> {
        self.processes
            .get(overlay_id)
            .map(QmlOverlayProcess::runtime_diagnostic)
    }

    pub fn cleanup_all(&mut self) -> Result<CleanupReport, DiagnosableError> {
        let overlay_ids = self.processes.keys().cloned().collect::<Vec<_>>();
        for overlay_id in &overlay_ids {
            self.cleanup(overlay_id.clone())?;
        }
        Ok(CleanupReport::all_succeeded(overlay_ids.len()))
    }

    pub fn mount_or_update(&mut self, snapshot: OverlaySnapshot) -> Result<(), DiagnosableError> {
        if snapshot.provider != RendererProviderId::Native {
            return Err(DiagnosableError::new(
                ErrorPhase::Registration,
                "QML overlay renderer supports only native provider snapshots",
            ));
        }
        let overlay_id = snapshot.overlay_id.clone();
        let process = self
            .processes
            .entry(overlay_id.clone())
            .or_insert_with(|| QmlOverlayProcess::new(&overlay_id));
        process.write_snapshot(&snapshot)?;
        process.ensure_running()?;
        self.active.insert(overlay_id, snapshot.clone());
        self.mounted.push(snapshot);
        Ok(())
    }

    pub fn hide(
        &mut self,
        overlay_id: impl Into<String>,
        _reason: OverlayDiagnosticReason,
    ) -> Result<(), DiagnosableError> {
        let overlay_id = overlay_id.into();
        self.active.remove(&overlay_id);
        if let Some(process) = self.processes.get_mut(&overlay_id) {
            process.write_hidden()?;
        }
        self.hidden.push(overlay_id);
        Ok(())
    }

    pub fn cleanup(&mut self, overlay_id: impl Into<String>) -> Result<(), DiagnosableError> {
        let overlay_id = overlay_id.into();
        self.active.remove(&overlay_id);
        if let Some(mut process) = self.processes.remove(&overlay_id) {
            process.stop();
            process.remove_files();
        }
        self.cleaned_up.push(overlay_id);
        Ok(())
    }
}

impl Drop for QmlOverlayRenderer {
    fn drop(&mut self) {
        let _ = self.cleanup_all();
    }
}

#[derive(Debug)]
struct QmlOverlayProcess {
    overlay_id: String,
    qml_path: PathBuf,
    state_path: PathBuf,
    stderr_path: PathBuf,
    qml_written: bool,
    child: Option<Child>,
}

impl QmlOverlayProcess {
    fn new(overlay_id: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "signal-auras-overlay-{}-{}",
            std::process::id(),
            sanitize_path_component(overlay_id)
        ));
        Self {
            overlay_id: overlay_id.to_string(),
            qml_path: dir.join("overlay.qml"),
            state_path: dir.join("state.qml"),
            stderr_path: dir.join("stderr.log"),
            qml_written: false,
            child: None,
        }
    }

    fn write_snapshot(&mut self, snapshot: &OverlaySnapshot) -> Result<(), DiagnosableError> {
        let Some(dir) = self.qml_path.parent() else {
            return Err(overlay_error("overlay temp directory is invalid"));
        };
        fs::create_dir_all(dir).map_err(overlay_io_error)?;
        if !self.qml_written {
            let bounds = overlay_bounds(&snapshot.visuals).unwrap_or(OverlayBounds {
                x: 0,
                y: 0,
                w: 1,
                h: 1,
            });
            fs::write(
                &self.qml_path,
                qml_overlay_source(&self.overlay_id, &self.state_path, bounds),
            )
            .map_err(overlay_io_error)?;
            self.qml_written = true;
        }
        write_atomic(&self.state_path, overlay_snapshot_qml(snapshot)).map_err(overlay_io_error)?;
        Ok(())
    }

    fn write_hidden(&self) -> Result<(), DiagnosableError> {
        write_atomic(&self.state_path, empty_overlay_state_qml()).map_err(overlay_io_error)
    }

    fn ensure_running(&mut self) -> Result<(), DiagnosableError> {
        if let Some(child) = &mut self.child {
            if let Some(status) = child.try_wait().map_err(overlay_io_error)? {
                let stderr = self.stderr_snippet();
                self.child = None;
                return Err(overlay_error(format!(
                    "native QML overlay process exited with {status}: {stderr}"
                )));
            } else {
                return Ok(());
            }
        }
        if !command_in_path(QML_LAUNCHER) {
            return Err(overlay_error("Qt qml launcher is unavailable in PATH"));
        }
        let stderr = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.stderr_path)
            .map_err(overlay_io_error)?;
        let mut command = Command::new(QML_LAUNCHER);
        command
            .arg("--transparent")
            .arg("-f")
            .arg(&self.qml_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(stderr));
        // The runtime handles Ctrl-C through signalfd, so child processes would
        // otherwise inherit blocked SIGINT/SIGTERM. Put QML in its own process
        // group and restore the normal signal mask/handlers before exec.
        unsafe {
            command.pre_exec(|| {
                install_parent_death_signal()?;
                reset_child_shutdown_signals()?;
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn().map_err(overlay_io_error)?;
        tracing::info!(
            event = "overlay_qml_spawn",
            overlay_id = %self.overlay_id,
            qml_path = %self.qml_path.display(),
            state_path = %self.state_path.display(),
            stderr_path = %self.stderr_path.display(),
        );
        self.child = Some(child);
        Ok(())
    }

    fn stderr_snippet(&self) -> String {
        let Ok(stderr) = fs::read_to_string(&self.stderr_path) else {
            return format!("stderr log unavailable at {}", self.stderr_path.display());
        };
        let trimmed = stderr.trim();
        if trimmed.is_empty() {
            return format!("stderr log is empty at {}", self.stderr_path.display());
        }
        let snippet = trimmed.chars().take(1_000).collect::<String>();
        format!("{snippet} (stderr: {})", self.stderr_path.display())
    }

    fn process_id(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    fn runtime_diagnostic(&self) -> String {
        format!(
            "qml_path={} state_path={} {}",
            self.qml_path.display(),
            self.state_path.display(),
            self.stderr_snippet()
        )
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            terminate_process_group(child.id());
            for _ in 0..10 {
                if child.try_wait().ok().flatten().is_some() {
                    return;
                }
                thread::sleep(Duration::from_millis(20));
            }
            kill_process_group(child.id());
            let _ = child.wait();
        }
    }

    fn remove_files(&self) {
        let _ = fs::remove_file(&self.qml_path);
        let _ = fs::remove_file(&self.state_path);
        let _ = fs::remove_file(&self.stderr_path);
        if let Some(dir) = self.qml_path.parent() {
            let _ = fs::remove_dir(dir);
        }
    }
}

pub fn qml_overlay_title(overlay_id: &str) -> String {
    format!("Signal Auras Overlay {overlay_id}")
}

fn qml_overlay_source(overlay_id: &str, state_path: &Path, bounds: OverlayBounds) -> String {
    let state_name = state_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("state.qml");
    let grab_path = std::env::var("SIGNAL_AURAS_OVERLAY_GRAB_PATH").unwrap_or_default();
    format!(
        r##"import QtQuick
import QtQuick.Window

Window {{
    id: root
    title: {title:?}
    x: {x}
    y: {y}
    width: {w}
    height: {h}
    color: "transparent"
    visible: true
    opacity: 0
    flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.WindowTransparentForInput | Qt.WindowDoesNotAcceptFocus
    property url stateUrl: Qt.resolvedUrl({state_name:?})
    property string grabPath: {grab_path:?}
    property bool grabSaved: false

    function reloadState() {{
        if (root.grabPath.length > 0 && !root.grabSaved) {{
            grabTimer.restart()
        }}
        overlayLoader.active = false
        overlayLoader.source = stateUrl.toString() + "?t=" + Date.now()
        overlayLoader.active = true
    }}

    Component.onCompleted: reloadState()

    Timer {{
        interval: {interval_ms}
        repeat: true
        running: true
        onTriggered: root.reloadState()
    }}

    Timer {{
        id: grabTimer
        interval: 600
        repeat: false
        running: false
        onTriggered: paintRoot.grabToImage(function(result) {{
            root.grabSaved = true
            result.saveToFile(root.grabPath)
        }})
    }}

    Item {{
        id: paintRoot
        anchors.fill: parent

        Loader {{
            id: overlayLoader
            anchors.fill: parent
        }}
    }}
}}
"##,
        title = qml_overlay_title(overlay_id),
        x = bounds.x,
        y = bounds.y,
        w = bounds.w,
        h = bounds.h,
        state_name = state_name,
        grab_path = grab_path,
        interval_ms = QML_POLL_INTERVAL_MS,
    )
}

#[cfg(test)]
fn overlay_snapshot_json(snapshot: &OverlaySnapshot) -> String {
    let bounds = overlay_bounds(&snapshot.visuals).unwrap_or(OverlayBounds {
        x: 0,
        y: 0,
        w: 1,
        h: 1,
    });
    let visuals = snapshot
        .visuals
        .iter()
        .map(|visual| visual_json(visual, bounds.x, bounds.y))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"overlay_id\":{},\"x\":{},\"y\":{},\"w\":{},\"h\":{},\"visuals\":[{}]}}",
        json_string(&snapshot.overlay_id),
        bounds.x,
        bounds.y,
        bounds.w,
        bounds.h,
        visuals
    )
}

fn overlay_snapshot_qml(snapshot: &OverlaySnapshot) -> String {
    let bounds = overlay_bounds(&snapshot.visuals).unwrap_or(OverlayBounds {
        x: 0,
        y: 0,
        w: 1,
        h: 1,
    });
    let visuals = snapshot
        .visuals
        .iter()
        .map(|visual| visual_qml(visual, bounds.x, bounds.y))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "import QtQuick\n\nItem {{\n    width: parent ? parent.width : {}\n    height: parent ? parent.height : {}\n{}\n}}\n",
        bounds.w, bounds.h, visuals
    )
}

fn empty_overlay_state_qml() -> &'static str {
    "import QtQuick\n\nItem { width: 1; height: 1 }\n"
}

pub fn overlay_window_placement(snapshot: &OverlaySnapshot) -> Option<OverlayWindowPlacement> {
    if snapshot.provider != RendererProviderId::Native || !snapshot.is_active() {
        return None;
    }
    let bounds = overlay_bounds(&snapshot.visuals)?;
    Some(OverlayWindowPlacement {
        overlay_id: snapshot.overlay_id.clone(),
        title: qml_overlay_title(&snapshot.overlay_id),
        process_id: None,
        x: bounds.x,
        y: bounds.y,
        w: bounds.w,
        h: bounds.h,
    })
}

pub fn overlay_smoke_snapshot() -> OverlaySnapshot {
    OverlaySnapshot {
        overlay_id: OVERLAY_SMOKE_ID.to_string(),
        provider: RendererProviderId::Native,
        lifecycle: OverlayLifecycleState::Active,
        visuals: vec![VisualSnapshot {
            visual_id: "magenta_probe".to_string(),
            rect: OVERLAY_SMOKE_RECT,
            opacity: 1.0,
            fill: OVERLAY_SMOKE_FILL.to_string(),
            background: OVERLAY_SMOKE_BACKGROUND.to_string(),
            label_visible: false,
            fill_fraction: 1.0,
            active: true,
            ready: false,
        }],
        diagnostic: None,
    }
}

pub fn probe_overlay_smoke_pixels(sample: &ScreenSample) -> OverlaySmokePixelReport {
    probe_overlay_smoke_pixels_at(sample, OVERLAY_SMOKE_PROBE_X, OVERLAY_SMOKE_PROBE_Y)
}

pub fn probe_overlay_smoke_grab_pixels(sample: &ScreenSample) -> OverlaySmokePixelReport {
    probe_overlay_smoke_pixels_at(
        sample,
        OVERLAY_SMOKE_PROBE_X.saturating_sub(OVERLAY_SMOKE_RECT.x),
        OVERLAY_SMOKE_PROBE_Y.saturating_sub(OVERLAY_SMOKE_RECT.y),
    )
}

fn probe_overlay_smoke_pixels_at(
    sample: &ScreenSample,
    probe_x: u32,
    probe_y: u32,
) -> OverlaySmokePixelReport {
    let mut report = OverlaySmokePixelReport {
        matched_pixels: 0,
        sampled_pixels: 0,
        sample_width: sample.width,
        sample_height: sample.height,
        pixel_format: sample.pixel_format,
    };
    let Some(x_end) = probe_x.checked_add(OVERLAY_SMOKE_PROBE_W) else {
        return report;
    };
    let Some(y_end) = probe_y.checked_add(OVERLAY_SMOKE_PROBE_H) else {
        return report;
    };
    if x_end > sample.width || y_end > sample.height {
        return report;
    }
    let bytes_per_pixel = sample.pixel_format.bytes_per_pixel();
    let stride = sample.stride as usize;
    if stride < sample.width as usize * bytes_per_pixel {
        return report;
    }
    for y in probe_y..y_end {
        for x in probe_x..x_end {
            let offset = y as usize * stride + x as usize * bytes_per_pixel;
            let Some(pixel) = sample.pixels.get(offset..offset + bytes_per_pixel) else {
                continue;
            };
            report.sampled_pixels += 1;
            if is_smoke_magenta_pixel(pixel, sample.pixel_format) {
                report.matched_pixels += 1;
            }
        }
    }
    report
}

fn is_smoke_magenta_pixel(pixel: &[u8], format: ScreenPixelFormat) -> bool {
    let Some((red, green, blue)) = rgb_channels(pixel, format) else {
        return false;
    };
    red >= 170 && blue >= 170 && green <= 120
}

fn rgb_channels(pixel: &[u8], format: ScreenPixelFormat) -> Option<(u8, u8, u8)> {
    match format {
        ScreenPixelFormat::Luma8 => None,
        ScreenPixelFormat::Rgb888 | ScreenPixelFormat::Rgba8888 | ScreenPixelFormat::Rgbx8888 => {
            Some((*pixel.first()?, *pixel.get(1)?, *pixel.get(2)?))
        }
        ScreenPixelFormat::Bgr888 | ScreenPixelFormat::Bgra8888 | ScreenPixelFormat::Bgrx8888 => {
            Some((*pixel.get(2)?, *pixel.get(1)?, *pixel.first()?))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OverlayBounds {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

fn overlay_bounds(visuals: &[VisualSnapshot]) -> Option<OverlayBounds> {
    let first = visuals.first()?;
    let mut min_x = first.rect.x;
    let mut min_y = first.rect.y;
    let mut max_x = first.rect.x.saturating_add(first.rect.w);
    let mut max_y = first.rect.y.saturating_add(first.rect.h);
    for visual in &visuals[1..] {
        min_x = min_x.min(visual.rect.x);
        min_y = min_y.min(visual.rect.y);
        max_x = max_x.max(visual.rect.x.saturating_add(visual.rect.w));
        max_y = max_y.max(visual.rect.y.saturating_add(visual.rect.h));
    }
    Some(OverlayBounds {
        x: min_x,
        y: min_y,
        w: max_x.saturating_sub(min_x).max(1),
        h: max_y.saturating_sub(min_y).max(1),
    })
}

#[cfg(test)]
fn visual_json(visual: &VisualSnapshot, origin_x: u32, origin_y: u32) -> String {
    format!(
        concat!(
            "{{",
            "\"visual_id\":{},",
            "\"x\":{},\"y\":{},\"w\":{},\"h\":{},",
            "\"opacity\":{},",
            "\"fill\":{},",
            "\"background\":{},",
            "\"label_visible\":{},",
            "\"fill_fraction\":{},",
            "\"active\":{},",
            "\"ready\":{}",
            "}}"
        ),
        json_string(&visual.visual_id),
        visual.rect.x.saturating_sub(origin_x),
        visual.rect.y.saturating_sub(origin_y),
        visual.rect.w,
        visual.rect.h,
        visual.opacity.clamp(0.0, 1.0),
        json_string(&visual.fill),
        json_string(&visual.background),
        visual.label_visible,
        visual.fill_fraction.clamp(0.0, 1.0),
        visual.active,
        visual.ready,
    )
}

fn visual_qml(visual: &VisualSnapshot, origin_x: u32, origin_y: u32) -> String {
    let x = visual.rect.x.saturating_sub(origin_x);
    let y = visual.rect.y.saturating_sub(origin_y);
    let opacity = visual.opacity.clamp(0.0, 1.0);
    let fill_fraction = visual.fill_fraction.clamp(0.0, 1.0);
    format!(
        r##"
    Item {{
        x: {x}
        y: {y}
        width: {w}
        height: {h}
        opacity: {opacity}

        Rectangle {{
            anchors.fill: parent
            color: {background}
            radius: 3
            opacity: 0.72
        }}

        Rectangle {{
            x: 0
            y: 0
            width: parent.width * {fill_fraction}
            height: parent.height
            color: {fill}
            radius: 3
        }}

        Text {{
            anchors.centerIn: parent
            visible: {label_visible}
            text: {label}
            color: "#f8fafc"
            font.pixelSize: Math.max(10, Math.floor(parent.height * 0.62))
            font.bold: true
        }}
    }}
"##,
        x = x,
        y = y,
        w = visual.rect.w,
        h = visual.rect.h,
        opacity = opacity,
        background = json_string(&visual.background),
        fill_fraction = fill_fraction,
        fill = json_string(&visual.fill),
        label_visible = visual.label_visible,
        label = json_string(if visual.ready {
            "ready"
        } else {
            &visual.visual_id
        }),
    )
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn sanitize_path_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn reset_child_shutdown_signals() -> std::io::Result<()> {
    unsafe {
        if libc::signal(libc::SIGINT, libc::SIG_DFL) == libc::SIG_ERR {
            return Err(std::io::Error::last_os_error());
        }
        if libc::signal(libc::SIGTERM, libc::SIG_DFL) == libc::SIG_ERR {
            return Err(std::io::Error::last_os_error());
        }
        let mut mask = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
        if libc::sigemptyset(mask.as_mut_ptr()) != 0 {
            return Err(std::io::Error::last_os_error());
        }
        if libc::sigprocmask(libc::SIG_SETMASK, mask.as_ptr(), std::ptr::null_mut()) != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

fn install_parent_death_signal() -> std::io::Result<()> {
    unsafe {
        if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
            return Err(std::io::Error::last_os_error());
        }
        if libc::getppid() == 1 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "parent process exited before overlay child could install shutdown signal",
            ));
        }
    }
    Ok(())
}

fn terminate_process_group(pid: u32) {
    signal_process_group(pid, libc::SIGTERM);
}

fn kill_process_group(pid: u32) {
    signal_process_group(pid, libc::SIGKILL);
}

fn signal_process_group(pid: u32, signal: libc::c_int) {
    let Ok(pid) = i32::try_from(pid) else {
        return;
    };
    if pid <= 0 {
        return;
    }
    unsafe {
        let _ = libc::kill(-pid, signal);
        let _ = libc::kill(pid, signal);
    }
}

pub fn provider_report_for_environment(environment: &KdeEnvironment) -> OverlayProviderReport {
    NativeOverlayRenderer::live().provider_report(environment)
}

fn native_overlay_environment_available(environment: &KdeEnvironment) -> bool {
    environment
        .wayland_display
        .as_deref()
        .is_some_and(not_empty)
        && environment
            .session_type
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("wayland"))
        && environment
            .current_desktop
            .as_deref()
            .is_some_and(|desktop| {
                desktop
                    .split(':')
                    .any(|part| part.eq_ignore_ascii_case("KDE"))
            })
        && environment.services.kwin
}

fn command_in_path(command: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(command).is_file())
}

fn not_empty(value: &str) -> bool {
    !value.trim().is_empty()
}

fn write_atomic(path: &Path, contents: impl AsRef<[u8]>) -> std::io::Result<()> {
    let mut tmp = path.to_path_buf();
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map_or_else(|| "tmp".to_string(), |extension| format!("{extension}.tmp"));
    tmp.set_extension(format!("{extension}.{}", std::process::id()));
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn overlay_io_error(error: impl std::fmt::Display) -> DiagnosableError {
    overlay_error(error.to_string())
}

fn overlay_error(message: impl Into<String>) -> DiagnosableError {
    DiagnosableError::new(ErrorPhase::Registration, message)
        .with_source("native-qml-overlay")
        .with_remediation("run from the project Nix dev shell or install Qt qml runtime support")
}

#[cfg(test)]
pub(crate) fn available_overlay_environment_for_test() -> KdeEnvironment {
    KdeEnvironment {
        wayland_display: Some("wayland-0".to_string()),
        session_type: Some("wayland".to_string()),
        current_desktop: Some("KDE".to_string()),
        services: KdeServiceAvailability {
            kwin: true,
            ..KdeServiceAvailability::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signal_auras_core::{OverlayDiagnostic, OverlayRect};

    #[test]
    fn provider_report_uses_renderer_backend_availability() {
        let renderer = NativeOverlayRenderer::in_memory();
        let available = renderer.provider_report(&available_overlay_environment_for_test());
        assert!(available
            .status(RendererProviderId::Native)
            .availability
            .allows_activation());
        assert!(!available
            .status(RendererProviderId::WebView)
            .availability
            .allows_activation());

        let unavailable = renderer.provider_report(&KdeEnvironment {
            wayland_display: Some("wayland-0".to_string()),
            session_type: Some("wayland".to_string()),
            current_desktop: Some("GNOME".to_string()),
            services: KdeServiceAvailability {
                kwin: true,
                ..KdeServiceAvailability::default()
            },
        });
        assert!(!unavailable
            .status(RendererProviderId::Native)
            .availability
            .allows_activation());
    }

    #[test]
    fn qml_renderer_generates_transparent_input_passthrough_window_source() {
        let qml = qml_overlay_source(
            "poe2-bars",
            Path::new("/tmp/signal-auras-state.qml"),
            OverlayBounds {
                x: 10,
                y: 20,
                w: 160,
                h: 12,
            },
        );

        assert!(qml.contains("WindowTransparentForInput"));
        assert!(qml.contains("WindowDoesNotAcceptFocus"));
        assert!(qml.contains("WindowStaysOnTopHint"));
        assert!(!qml.contains("Qt.Tool"));
        assert!(qml.contains("color: \"transparent\""));
        assert!(qml.contains("visible: true"));
        assert!(qml.contains("x: 10"));
        assert!(qml.contains("y: 20"));
        assert!(qml.contains("opacity: 0"));
        assert!(qml.contains("width: 160"));
        assert!(qml.contains("height: 12"));
        assert!(qml.contains("Loader"));
        assert!(qml.contains("overlayLoader.source = stateUrl.toString() + \"?t=\" + Date.now()"));
        assert!(qml.contains("grabToImage"));
        assert!(qml.contains("Signal Auras Overlay poe2-bars"));
        assert!(!qml.contains("visible: modelData.active"));
        assert!(!qml.contains("Window.FullScreen"));
        assert!(!qml.contains("Screen.width"));
    }

    #[test]
    fn qml_renderer_serializes_bounded_sanitized_visual_snapshot_data() {
        let json = overlay_snapshot_json(&active_snapshot("poe2-bars"));

        assert!(json.contains("\"overlay_id\":\"poe2-bars\""));
        assert!(json.contains("\"x\":10"));
        assert!(json.contains("\"y\":20"));
        assert!(json.contains("\"w\":160"));
        assert!(json.contains("\"h\":12"));
        assert!(json.contains("\"visual_id\":\"heavy-stun\""));
        assert!(json.contains("\"visual_id\":\"heavy-stun\",\"x\":0,\"y\":0"));
        assert!(json.contains("\"fill\":\"#6ee7b7\""));
        assert!(json.contains("\"fill_fraction\":0.5"));
        assert!(!json.contains("pixels"));
        assert!(!json.contains("compositor"));
        assert!(!json.contains("SynthesizedInput"));
    }

    #[test]
    fn qml_renderer_generates_local_state_component() {
        let qml = overlay_snapshot_qml(&active_snapshot("poe2-bars"));

        assert!(qml.contains("import QtQuick"));
        assert!(qml.contains("x: 0"));
        assert!(qml.contains("y: 0"));
        assert!(qml.contains("width: parent.width * 0.5"));
        assert!(qml.contains("color: \"#6ee7b7\""));
        assert!(!qml.contains("pixels"));
        assert!(!qml.contains("compositor"));
    }

    #[test]
    fn overlay_smoke_pixel_probe_detects_magenta_rendered_region() {
        let mut pixels = vec![0u8; 260 * 120 * 3];
        for y in OVERLAY_SMOKE_PROBE_Y..OVERLAY_SMOKE_PROBE_Y + OVERLAY_SMOKE_PROBE_H {
            for x in OVERLAY_SMOKE_PROBE_X..OVERLAY_SMOKE_PROBE_X + OVERLAY_SMOKE_PROBE_W {
                let offset = (y as usize * 260 + x as usize) * 3;
                pixels[offset] = 255;
                pixels[offset + 1] = 0;
                pixels[offset + 2] = 255;
            }
        }
        let sample =
            ScreenSample::from_pixels(260, 120, 260 * 3, ScreenPixelFormat::Rgb888, 0, pixels);

        let report = probe_overlay_smoke_pixels(&sample);

        assert!(report.passed());
        assert_eq!(
            report.sampled_pixels,
            OVERLAY_SMOKE_PROBE_W * OVERLAY_SMOKE_PROBE_H
        );
        assert_eq!(report.matched_pixels, report.sampled_pixels);
    }

    #[test]
    fn overlay_smoke_grab_probe_uses_local_overlay_coordinates() {
        let local_x = OVERLAY_SMOKE_PROBE_X - OVERLAY_SMOKE_RECT.x;
        let local_y = OVERLAY_SMOKE_PROBE_Y - OVERLAY_SMOKE_RECT.y;
        let mut pixels = vec![0u8; 260 * 120 * 3];
        for y in local_y..local_y + OVERLAY_SMOKE_PROBE_H {
            for x in local_x..local_x + OVERLAY_SMOKE_PROBE_W {
                let offset = (y as usize * 260 + x as usize) * 3;
                pixels[offset] = 255;
                pixels[offset + 1] = 0;
                pixels[offset + 2] = 255;
            }
        }
        let sample =
            ScreenSample::from_pixels(260, 120, 260 * 3, ScreenPixelFormat::Rgb888, 0, pixels);

        let report = probe_overlay_smoke_grab_pixels(&sample);

        assert!(report.passed());
        assert_eq!(report.matched_pixels, report.sampled_pixels);
    }

    #[test]
    fn overlay_smoke_pixel_probe_rejects_missing_rendered_region() {
        let sample = ScreenSample::from_pixels(
            260,
            120,
            260 * 3,
            ScreenPixelFormat::Rgb888,
            0,
            vec![0u8; 260 * 120 * 3],
        );

        let report = probe_overlay_smoke_pixels(&sample);

        assert!(!report.passed());
        assert_eq!(report.matched_pixels, 0);
    }

    #[test]
    fn overlay_window_placement_uses_visual_bounds_and_stable_qml_title() {
        let placement = overlay_window_placement(&active_snapshot("poe2-bars")).unwrap();

        assert_eq!(placement.overlay_id, "poe2-bars");
        assert_eq!(placement.title, "Signal Auras Overlay poe2-bars");
        assert_eq!(placement.process_id, None);
        assert_eq!(placement.x, 10);
        assert_eq!(placement.y, 20);
        assert_eq!(placement.w, 160);
        assert_eq!(placement.h, 12);

        let inactive = OverlaySnapshot {
            lifecycle: OverlayLifecycleState::Inactive,
            ..active_snapshot("poe2-bars")
        };
        assert!(overlay_window_placement(&inactive).is_none());
    }

    #[test]
    fn qml_process_keeps_stderr_path_for_runtime_diagnostics() {
        let process = QmlOverlayProcess::new("poe2-test");

        assert!(process.stderr_path.ends_with("stderr.log"));
    }

    #[test]
    fn qml_state_updates_are_written_atomically() {
        let path = std::env::temp_dir().join(format!(
            "signal-auras-overlay-test-{}-atomic-state.qml",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);

        write_atomic(&path, "import QtQuick\nItem { width: 1 }").unwrap();
        write_atomic(&path, "import QtQuick\nItem { width: 2 }").unwrap();

        let state = fs::read_to_string(&path).unwrap();
        assert_eq!(state, "import QtQuick\nItem { width: 2 }");
        assert!(!state.is_empty());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn qml_renderer_writes_program_once_and_state_each_update() {
        let mut process = QmlOverlayProcess::new("poe2-test");
        process.qml_path = std::env::temp_dir().join(format!(
            "signal-auras-overlay-test-{}-{}.qml",
            std::process::id(),
            "program-once"
        ));
        process.state_path = std::env::temp_dir().join(format!(
            "signal-auras-overlay-test-{}-{}.json",
            std::process::id(),
            "program-once"
        ));
        let _ = fs::remove_file(&process.qml_path);
        let _ = fs::remove_file(&process.state_path);

        process
            .write_snapshot(&active_snapshot("poe2-bars"))
            .unwrap();
        let qml_metadata = fs::metadata(&process.qml_path).unwrap();
        let first_state = fs::read_to_string(&process.state_path).unwrap();

        let mut updated = active_snapshot("poe2-bars");
        updated.visuals[0].fill_fraction = 0.75;
        process.write_snapshot(&updated).unwrap();
        let updated_qml_metadata = fs::metadata(&process.qml_path).unwrap();
        let second_state = fs::read_to_string(&process.state_path).unwrap();

        assert_eq!(
            qml_metadata.modified().unwrap(),
            updated_qml_metadata.modified().unwrap()
        );
        assert_ne!(first_state, second_state);

        process.write_hidden().unwrap();
        let hidden_state = fs::read_to_string(&process.state_path).unwrap();
        assert!(hidden_state.contains("Item { width: 1; height: 1 }"));

        let _ = fs::remove_file(&process.qml_path);
        let _ = fs::remove_file(&process.state_path);
    }

    #[test]
    fn native_renderer_updates_hides_and_cleans_up_sanitized_snapshots() {
        let mut renderer = NativeOverlayRenderer::in_memory();
        let active = active_snapshot("poe2-bars");

        renderer.render_snapshot(active.clone()).unwrap();
        assert_eq!(renderer.mounted(), &[active.clone()]);
        assert_eq!(
            renderer.active_snapshot("poe2-bars").unwrap().visuals[0].fill,
            "#6ee7b7"
        );

        renderer
            .render_snapshot(OverlaySnapshot {
                lifecycle: OverlayLifecycleState::Inactive,
                diagnostic: Some(OverlayDiagnostic {
                    overlay_id: "poe2-bars".to_string(),
                    provider: RendererProviderId::Native,
                    lifecycle: OverlayLifecycleState::Inactive,
                    reason: OverlayDiagnosticReason::FocusInactive,
                    tracker_id: None,
                    field: None,
                    message: "focus inactive".to_string(),
                }),
                ..active.clone()
            })
            .unwrap();
        assert!(renderer.active_snapshot("poe2-bars").is_none());
        assert_eq!(renderer.hidden(), &["poe2-bars".to_string()]);

        renderer.render_snapshot(active).unwrap();
        let report = renderer.cleanup_all().unwrap();
        assert_eq!(report.attempted, 1);
        assert_eq!(renderer.cleaned_up(), &["poe2-bars".to_string()]);
    }

    #[test]
    fn native_renderer_rejects_future_provider_snapshots() {
        let mut renderer = NativeOverlayRenderer::in_memory();
        let mut snapshot = active_snapshot("tool");
        snapshot.provider = RendererProviderId::WebView;

        let error = renderer.render_snapshot(snapshot).unwrap_err();
        assert!(error
            .message
            .contains("cannot render provider 'webview' snapshots"));
    }

    fn active_snapshot(id: &str) -> OverlaySnapshot {
        OverlaySnapshot {
            overlay_id: id.to_string(),
            provider: RendererProviderId::Native,
            lifecycle: OverlayLifecycleState::Active,
            visuals: vec![VisualSnapshot {
                visual_id: "heavy-stun".to_string(),
                rect: OverlayRect {
                    x: 10,
                    y: 20,
                    w: 160,
                    h: 12,
                },
                opacity: 0.65,
                fill: "#6ee7b7".to_string(),
                background: "#111827".to_string(),
                label_visible: false,
                fill_fraction: 0.5,
                active: true,
                ready: false,
            }],
            diagnostic: None,
        }
    }
}
