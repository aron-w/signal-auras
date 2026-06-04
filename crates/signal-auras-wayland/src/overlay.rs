use signal_auras_core::{
    CleanupReport, DiagnosableError, ErrorPhase, OverlayDiagnosticReason, OverlayLifecycleState,
    OverlayProviderReport, OverlayProviderStatus, OverlaySnapshot, RendererProviderId,
    VisualSnapshot,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

use crate::capability::KdeEnvironment;
#[cfg(test)]
use crate::capability::KdeServiceAvailability;

const QML_LAUNCHER: &str = "qml";
const QML_POLL_INTERVAL_MS: u64 = 50;

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
        if let Some(mut process) = self.processes.remove(&overlay_id) {
            process.stop();
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
            state_path: dir.join("state.json"),
            child: None,
        }
    }

    fn write_snapshot(&self, snapshot: &OverlaySnapshot) -> Result<(), DiagnosableError> {
        let Some(dir) = self.qml_path.parent() else {
            return Err(overlay_error("overlay temp directory is invalid"));
        };
        fs::create_dir_all(dir).map_err(overlay_io_error)?;
        fs::write(
            &self.qml_path,
            qml_overlay_source(&self.overlay_id, &self.state_path),
        )
        .map_err(overlay_io_error)?;
        fs::write(&self.state_path, overlay_snapshot_json(snapshot)).map_err(overlay_io_error)?;
        Ok(())
    }

    fn ensure_running(&mut self) -> Result<(), DiagnosableError> {
        if let Some(child) = &mut self.child {
            if child.try_wait().map_err(overlay_io_error)?.is_none() {
                return Ok(());
            }
            self.child = None;
        }
        if !command_in_path(QML_LAUNCHER) {
            return Err(overlay_error("Qt qml launcher is unavailable in PATH"));
        }
        let child = Command::new(QML_LAUNCHER)
            .arg("--transparent")
            .arg("-f")
            .arg(&self.qml_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(overlay_io_error)?;
        self.child = Some(child);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn remove_files(&self) {
        let _ = fs::remove_file(&self.qml_path);
        let _ = fs::remove_file(&self.state_path);
        if let Some(dir) = self.qml_path.parent() {
            let _ = fs::remove_dir(dir);
        }
    }
}

fn qml_overlay_source(overlay_id: &str, state_path: &Path) -> String {
    let state_url = format!("file://{}", state_path.display());
    format!(
        r##"import QtQuick
import QtQuick.Window

Window {{
    id: root
    title: {title:?}
    x: 0
    y: 0
    width: Screen.width
    height: Screen.height
    color: "transparent"
    visible: true
    flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.Tool | Qt.WindowTransparentForInput
    property string stateUrl: {state_url:?}
    property var bars: []

    function reloadState() {{
        var request = new XMLHttpRequest()
        request.open("GET", stateUrl + "?t=" + Date.now(), false)
        request.send()
        if (request.status === 0 || request.status === 200) {{
            var parsed = JSON.parse(request.responseText)
            bars = parsed.visuals || []
        }}
    }}

    Component.onCompleted: reloadState()

    Timer {{
        interval: {interval_ms}
        repeat: true
        running: true
        onTriggered: root.reloadState()
    }}

    Repeater {{
        model: root.bars
        delegate: Item {{
            x: modelData.x
            y: modelData.y
            width: modelData.w
            height: modelData.h
            opacity: modelData.opacity
            visible: modelData.active

            Rectangle {{
                anchors.fill: parent
                color: modelData.background
                radius: 3
                opacity: 0.72
            }}

            Rectangle {{
                x: 0
                y: 0
                width: parent.width * modelData.fill_fraction
                height: parent.height
                color: modelData.fill
                radius: 3
            }}

            Text {{
                anchors.centerIn: parent
                visible: modelData.label_visible
                text: modelData.ready ? modelData.visual_id + " ready" : modelData.visual_id
                color: "#f8fafc"
                font.pixelSize: Math.max(10, Math.floor(parent.height * 0.62))
                font.bold: true
            }}
        }}
    }}
}}
"##,
        title = format!("Signal Auras Overlay {overlay_id}"),
        state_url = state_url,
        interval_ms = QML_POLL_INTERVAL_MS,
    )
}

fn overlay_snapshot_json(snapshot: &OverlaySnapshot) -> String {
    let visuals = snapshot
        .visuals
        .iter()
        .map(visual_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"overlay_id\":{},\"visuals\":[{}]}}",
        json_string(&snapshot.overlay_id),
        visuals
    )
}

fn visual_json(visual: &VisualSnapshot) -> String {
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
        visual.rect.x,
        visual.rect.y,
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
        let qml = qml_overlay_source("poe2-bars", Path::new("/tmp/signal-auras-state.json"));

        assert!(qml.contains("WindowTransparentForInput"));
        assert!(qml.contains("WindowStaysOnTopHint"));
        assert!(qml.contains("color: \"transparent\""));
        assert!(qml.contains("XMLHttpRequest"));
        assert!(qml.contains("modelData.fill_fraction"));
    }

    #[test]
    fn qml_renderer_serializes_only_sanitized_visual_snapshot_data() {
        let json = overlay_snapshot_json(&active_snapshot("poe2-bars"));

        assert!(json.contains("\"overlay_id\":\"poe2-bars\""));
        assert!(json.contains("\"visual_id\":\"heavy-stun\""));
        assert!(json.contains("\"fill\":\"#6ee7b7\""));
        assert!(json.contains("\"fill_fraction\":0.5"));
        assert!(!json.contains("pixels"));
        assert!(!json.contains("compositor"));
        assert!(!json.contains("SynthesizedInput"));
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
