use crate::{
    ActiveProcessContext, AdapterDiagnostic, CapabilityAvailability, CapabilityKind,
    CapabilityReport, CapabilitySet, DetectorDefinition, DiagnosableError, ErrorPhase,
    RadialCooldownPhase, ScopeDecision, ScopeSelection, StateTrackerDefinitionSet, TrackerState,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RendererProviderId {
    Native,
    WebView,
    TauriWindow,
    ToolWindow,
}

impl RendererProviderId {
    pub fn parse(value: &str) -> Result<Self, DiagnosableError> {
        match value {
            "native" => Ok(Self::Native),
            "webview" => Ok(Self::WebView),
            "tauri_window" => Ok(Self::TauriWindow),
            "tool_window" => Ok(Self::ToolWindow),
            other => Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("unknown overlay provider '{other}'"),
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::WebView => "webview",
            Self::TauriWindow => "tauri_window",
            Self::ToolWindow => "tool_window",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlaySurfaceKind {
    Overlay,
}

impl OverlaySurfaceKind {
    pub fn parse(value: Option<&str>) -> Result<Self, DiagnosableError> {
        match value.unwrap_or("overlay") {
            "overlay" => Ok(Self::Overlay),
            other => Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("unsupported overlay surface '{other}'"),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl OverlayRect {
    pub fn new(x: i64, y: i64, w: i64, h: i64) -> Result<Self, DiagnosableError> {
        if x < 0 || y < 0 {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "overlay rect coordinates cannot be negative",
            ));
        }
        if w <= 0 || h <= 0 {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "overlay rect width and height must be positive",
            ));
        }
        Ok(Self {
            x: x as u32,
            y: y as u32,
            w: w as u32,
            h: h as u32,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OverlayStyle {
    pub fill: Option<String>,
    pub background: Option<String>,
    pub opacity: Option<f32>,
    pub label_visible: Option<bool>,
}

impl OverlayStyle {
    pub fn new(
        fill: Option<impl Into<String>>,
        background: Option<impl Into<String>>,
        opacity: Option<f32>,
        label_visible: Option<bool>,
    ) -> Result<Self, DiagnosableError> {
        if let Some(opacity) = opacity {
            validate_opacity(opacity)?;
        }
        let fill = fill.map(Into::into).map(validate_color).transpose()?;
        let background = background.map(Into::into).map(validate_color).transpose()?;
        Ok(Self {
            fill,
            background,
            opacity,
            label_visible,
        })
    }

    fn apply_to(&self, snapshot: &mut VisualSnapshot) {
        if let Some(fill) = &self.fill {
            snapshot.fill = fill.clone();
        }
        if let Some(background) = &self.background {
            snapshot.background = background.clone();
        }
        if let Some(opacity) = self.opacity {
            snapshot.opacity = opacity;
        }
        if let Some(label_visible) = self.label_visible {
            snapshot.label_visible = label_visible;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateField {
    ProgressPercent,
    RemainingMs,
}

impl StateField {
    pub fn parse(value: &str) -> Result<Self, DiagnosableError> {
        match value {
            "progress_percent" => Ok(Self::ProgressPercent),
            "remaining_ms" => Ok(Self::RemainingMs),
            other => Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!("unknown overlay state field '{other}'"),
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProgressPercent => "progress_percent",
            Self::RemainingMs => "remaining_ms",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateBinding {
    pub tracker_id: String,
    pub field: StateField,
}

impl StateBinding {
    pub fn new(tracker_id: impl Into<String>, field: StateField) -> Result<Self, DiagnosableError> {
        let tracker_id = normalize_id(tracker_id.into(), "overlay state binding tracker")?;
        Ok(Self { tracker_id, field })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressBarVisualDefinition {
    pub id: String,
    pub binding: StateBinding,
    pub rect: OverlayRect,
    pub opacity: f32,
    pub fill: String,
    pub background: String,
    pub label_visible: bool,
    pub ready_style: Option<OverlayStyle>,
    pub inactive_style: Option<OverlayStyle>,
}

impl ProgressBarVisualDefinition {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        binding: StateBinding,
        rect: OverlayRect,
        opacity: f32,
        fill: impl Into<String>,
        background: impl Into<String>,
        label_visible: bool,
        ready_style: Option<OverlayStyle>,
        inactive_style: Option<OverlayStyle>,
    ) -> Result<Self, DiagnosableError> {
        validate_opacity(opacity)?;
        Ok(Self {
            id: normalize_id(id.into(), "overlay visual id")?,
            binding,
            rect,
            opacity,
            fill: validate_color(fill.into())?,
            background: validate_color(background.into())?,
            label_visible,
            ready_style,
            inactive_style,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VisualDefinition {
    ProgressBar(ProgressBarVisualDefinition),
}

impl VisualDefinition {
    pub fn id(&self) -> &str {
        match self {
            Self::ProgressBar(visual) => &visual.id,
        }
    }

    pub fn binding(&self) -> &StateBinding {
        match self {
            Self::ProgressBar(visual) => &visual.binding,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OverlayDefinition {
    pub id: String,
    pub scope: ScopeSelection,
    pub surface_kind: OverlaySurfaceKind,
    pub provider: RendererProviderId,
    pub visuals: Vec<VisualDefinition>,
}

impl OverlayDefinition {
    pub fn new(
        id: impl Into<String>,
        scope: ScopeSelection,
        surface_kind: OverlaySurfaceKind,
        provider: RendererProviderId,
        visuals: impl IntoIterator<Item = VisualDefinition>,
    ) -> Result<Self, DiagnosableError> {
        let visuals = visuals.into_iter().collect::<Vec<_>>();
        if visuals.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "overlay requires at least one visual",
            ));
        }
        let mut seen = BTreeSet::new();
        for visual in &visuals {
            if !seen.insert(visual.id().to_string()) {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!("duplicate overlay visual id '{}'", visual.id()),
                ));
            }
        }
        Ok(Self {
            id: normalize_id(id.into(), "overlay id")?,
            scope,
            surface_kind,
            provider,
            visuals,
        })
    }

    fn required_capabilities(&self) -> CapabilitySet {
        if matches!(self.scope, ScopeSelection::ProcessList { .. }) {
            CapabilitySet::new([CapabilityKind::ActiveProcessMetadata])
        } else {
            CapabilitySet::default()
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct OverlayDefinitionSet {
    overlays: Vec<OverlayDefinition>,
    required_capabilities: CapabilitySet,
}

impl OverlayDefinitionSet {
    pub fn new(
        overlays: impl IntoIterator<Item = OverlayDefinition>,
        trackers: &StateTrackerDefinitionSet,
    ) -> Result<Self, DiagnosableError> {
        let overlays = overlays.into_iter().collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        let mut required = Vec::new();
        for overlay in &overlays {
            if !seen.insert(overlay.id.clone()) {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!("duplicate overlay id '{}'", overlay.id),
                ));
            }
            required.extend(overlay.required_capabilities().iter());
            validate_overlay_bindings(overlay, trackers)?;
        }
        Ok(Self {
            overlays,
            required_capabilities: CapabilitySet::new(required),
        })
    }

    pub fn overlays(&self) -> &[OverlayDefinition] {
        &self.overlays
    }

    pub fn required_capabilities(&self) -> &CapabilitySet {
        &self.required_capabilities
    }

    pub fn is_empty(&self) -> bool {
        self.overlays.is_empty()
    }

    pub fn snapshots(
        &self,
        now_ms: u64,
        capabilities: &CapabilityReport,
        active_context: &ActiveProcessContext,
        tracker_states: &BTreeMap<String, TrackerState>,
        providers: &OverlayProviderReport,
    ) -> Vec<OverlaySnapshot> {
        self.overlays
            .iter()
            .map(|overlay| {
                overlay_snapshot(
                    overlay,
                    now_ms,
                    capabilities,
                    active_context,
                    tracker_states,
                    providers,
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayLifecycleState {
    Registered,
    Available,
    Active,
    Inactive,
    Denied,
    Stale,
    Unavailable,
    Failed,
    CleanedUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayDiagnosticReason {
    ProviderUnavailable,
    PermissionDenied,
    FocusInactive,
    MissingStateSource,
    StaleStateSource,
    UnsupportedState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayDiagnostic {
    pub overlay_id: String,
    pub provider: RendererProviderId,
    pub lifecycle: OverlayLifecycleState,
    pub reason: OverlayDiagnosticReason,
    pub tracker_id: Option<String>,
    pub field: Option<StateField>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OverlaySnapshot {
    pub overlay_id: String,
    pub provider: RendererProviderId,
    pub lifecycle: OverlayLifecycleState,
    pub visuals: Vec<VisualSnapshot>,
    pub diagnostic: Option<OverlayDiagnostic>,
}

impl OverlaySnapshot {
    pub fn is_active(&self) -> bool {
        self.lifecycle == OverlayLifecycleState::Active
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VisualSnapshot {
    pub visual_id: String,
    pub shape: VisualShape,
    pub rect: OverlayRect,
    pub opacity: f32,
    pub fill: String,
    pub background: String,
    pub label_visible: bool,
    pub fill_fraction: f32,
    pub active: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualShape {
    Rect,
    Circle { center_dot: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayProviderStatus {
    pub provider: RendererProviderId,
    pub availability: CapabilityAvailability,
    pub diagnostic: Option<AdapterDiagnostic>,
}

impl OverlayProviderStatus {
    pub fn available(provider: RendererProviderId) -> Self {
        Self {
            provider,
            availability: CapabilityAvailability::Available,
            diagnostic: None,
        }
    }

    pub fn unavailable(provider: RendererProviderId, message: impl Into<String>) -> Self {
        Self {
            provider,
            availability: CapabilityAvailability::Unsupported,
            diagnostic: Some(AdapterDiagnostic::new(ErrorPhase::CapabilityProbe, message)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayProviderReport {
    statuses: BTreeMap<RendererProviderId, OverlayProviderStatus>,
}

impl OverlayProviderReport {
    pub fn from_statuses(statuses: impl IntoIterator<Item = OverlayProviderStatus>) -> Self {
        Self {
            statuses: statuses
                .into_iter()
                .map(|status| (status.provider, status))
                .collect(),
        }
    }

    pub fn native_available() -> Self {
        Self::from_statuses([OverlayProviderStatus::available(RendererProviderId::Native)])
    }

    pub fn status(&self, provider: RendererProviderId) -> OverlayProviderStatus {
        self.statuses.get(&provider).cloned().unwrap_or_else(|| {
            OverlayProviderStatus::unavailable(
                provider,
                format!("overlay provider '{}' is unavailable", provider.as_str()),
            )
        })
    }
}

fn overlay_snapshot(
    overlay: &OverlayDefinition,
    now_ms: u64,
    capabilities: &CapabilityReport,
    active_context: &ActiveProcessContext,
    tracker_states: &BTreeMap<String, TrackerState>,
    providers: &OverlayProviderReport,
) -> OverlaySnapshot {
    let provider_status = providers.status(overlay.provider);
    if !provider_status.availability.allows_activation() {
        return closed_snapshot(
            overlay,
            OverlayLifecycleState::Unavailable,
            OverlayDiagnosticReason::ProviderUnavailable,
            None,
            None,
            provider_status.diagnostic.map_or_else(
                || "overlay provider is unavailable".to_string(),
                |d| d.message,
            ),
        );
    }

    if let Some(error) = capabilities.first_blocking_error(&overlay.required_capabilities()) {
        return closed_snapshot(
            overlay,
            OverlayLifecycleState::Denied,
            OverlayDiagnosticReason::PermissionDenied,
            None,
            None,
            error.message,
        );
    }

    if !matches!(
        overlay.scope.decide_context(active_context),
        ScopeDecision::Allowed
    ) {
        return closed_snapshot(
            overlay,
            OverlayLifecycleState::Inactive,
            OverlayDiagnosticReason::FocusInactive,
            None,
            None,
            "overlay focus is inactive or untrusted",
        );
    }

    let mut visual_snapshots = Vec::new();
    for visual in &overlay.visuals {
        let binding = visual.binding();
        let Some(state) = tracker_states.get(&binding.tracker_id) else {
            return closed_snapshot(
                overlay,
                OverlayLifecycleState::Stale,
                OverlayDiagnosticReason::MissingStateSource,
                Some(binding.tracker_id.clone()),
                Some(binding.field),
                "overlay state source is missing",
            );
        };
        if state_freshness_ms(state) > 250 {
            return closed_snapshot(
                overlay,
                OverlayLifecycleState::Stale,
                OverlayDiagnosticReason::StaleStateSource,
                Some(binding.tracker_id.clone()),
                Some(binding.field),
                "overlay state source is stale",
            );
        }
        let Some(snapshot) = visual_snapshot(visual, state, now_ms) else {
            return closed_snapshot(
                overlay,
                OverlayLifecycleState::Inactive,
                OverlayDiagnosticReason::UnsupportedState,
                Some(binding.tracker_id.clone()),
                Some(binding.field),
                "overlay state source cannot drive this visual",
            );
        };
        visual_snapshots.push(snapshot);
    }

    OverlaySnapshot {
        overlay_id: overlay.id.clone(),
        provider: overlay.provider,
        lifecycle: OverlayLifecycleState::Active,
        visuals: visual_snapshots,
        diagnostic: None,
    }
}

fn visual_snapshot(
    visual: &VisualDefinition,
    state: &TrackerState,
    _now_ms: u64,
) -> Option<VisualSnapshot> {
    match (visual, state) {
        (
            VisualDefinition::ProgressBar(visual),
            TrackerState::HorizontalProgressBar {
                visible,
                progress_percent,
                ..
            },
        ) if visual.binding.field == StateField::ProgressPercent => {
            let mut snapshot = base_visual_snapshot(
                visual,
                f32::from(*progress_percent).clamp(0.0, 100.0) / 100.0,
                *visible,
                false,
            );
            if !visible {
                apply_inactive_style(visual, &mut snapshot);
            }
            Some(snapshot)
        }
        (
            VisualDefinition::ProgressBar(visual),
            TrackerState::RadialCooldown {
                phase,
                ready,
                cooldown_fraction,
                ..
            },
        ) if visual.binding.field == StateField::RemainingMs => {
            let fill_fraction = match phase {
                RadialCooldownPhase::Ready => 1.0,
                RadialCooldownPhase::Recovering => {
                    1.0 - (f32::from(*cooldown_fraction).clamp(0.0, 100.0) / 100.0)
                }
                RadialCooldownPhase::Activated
                | RadialCooldownPhase::Active
                | RadialCooldownPhase::Unknown => 0.0,
            };
            let mut snapshot = base_visual_snapshot(visual, fill_fraction, true, *ready);
            match phase {
                RadialCooldownPhase::Ready => {
                    if let Some(style) = &visual.ready_style {
                        style.apply_to(&mut snapshot);
                    }
                }
                RadialCooldownPhase::Activated => {
                    snapshot.fill = "#f97316".to_string();
                    snapshot.background = "#7f1d1d".to_string();
                    snapshot.opacity = snapshot.opacity.max(0.85);
                }
                RadialCooldownPhase::Active => {
                    snapshot.fill = "#38bdf8".to_string();
                    snapshot.background = "#082f49".to_string();
                    snapshot.opacity = snapshot.opacity.max(0.8);
                }
                RadialCooldownPhase::Unknown => {
                    apply_inactive_style(visual, &mut snapshot);
                }
                RadialCooldownPhase::Recovering => {}
            }
            Some(snapshot)
        }
        (VisualDefinition::ProgressBar(visual), TrackerState::Inactive { .. }) => {
            let mut snapshot = base_visual_snapshot(visual, 0.0, false, false);
            apply_inactive_style(visual, &mut snapshot);
            Some(snapshot)
        }
        _ => None,
    }
}

fn base_visual_snapshot(
    visual: &ProgressBarVisualDefinition,
    fill_fraction: f32,
    active: bool,
    ready: bool,
) -> VisualSnapshot {
    VisualSnapshot {
        visual_id: visual.id.clone(),
        shape: VisualShape::Rect,
        rect: visual.rect.clone(),
        opacity: visual.opacity,
        fill: visual.fill.clone(),
        background: visual.background.clone(),
        label_visible: visual.label_visible,
        fill_fraction: fill_fraction.clamp(0.0, 1.0),
        active,
        ready,
    }
}

fn apply_inactive_style(visual: &ProgressBarVisualDefinition, snapshot: &mut VisualSnapshot) {
    if let Some(style) = &visual.inactive_style {
        style.apply_to(snapshot);
    }
    snapshot.active = false;
}

fn closed_snapshot(
    overlay: &OverlayDefinition,
    lifecycle: OverlayLifecycleState,
    reason: OverlayDiagnosticReason,
    tracker_id: Option<String>,
    field: Option<StateField>,
    message: impl Into<String>,
) -> OverlaySnapshot {
    OverlaySnapshot {
        overlay_id: overlay.id.clone(),
        provider: overlay.provider,
        lifecycle,
        visuals: Vec::new(),
        diagnostic: Some(OverlayDiagnostic {
            overlay_id: overlay.id.clone(),
            provider: overlay.provider,
            lifecycle,
            reason,
            tracker_id,
            field,
            message: message.into(),
        }),
    }
}

fn validate_overlay_bindings(
    overlay: &OverlayDefinition,
    trackers: &StateTrackerDefinitionSet,
) -> Result<(), DiagnosableError> {
    for visual in &overlay.visuals {
        let binding = visual.binding();
        let tracker = trackers
            .trackers()
            .iter()
            .find(|tracker| tracker.id == binding.tracker_id)
            .ok_or_else(|| {
                DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!(
                        "overlay visual '{}' references missing state tracker '{}'",
                        visual.id(),
                        binding.tracker_id
                    ),
                )
            })?;
        let valid = matches!(
            (&tracker.detector, binding.field),
            (
                DetectorDefinition::HorizontalProgressBar { .. },
                StateField::ProgressPercent
            ) | (
                DetectorDefinition::RadialCooldown { .. },
                StateField::RemainingMs
            )
        );
        if !valid {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                format!(
                    "overlay visual '{}' cannot bind field '{}' from tracker '{}'",
                    visual.id(),
                    binding.field.as_str(),
                    binding.tracker_id
                ),
            ));
        }
    }
    Ok(())
}

fn state_freshness_ms(state: &TrackerState) -> u64 {
    match state {
        TrackerState::RadialCooldown { freshness_ms, .. }
        | TrackerState::HorizontalProgressBar { freshness_ms, .. }
        | TrackerState::Inactive { freshness_ms, .. } => *freshness_ms,
    }
}

fn validate_opacity(opacity: f32) -> Result<(), DiagnosableError> {
    if !(0.0..=1.0).contains(&opacity) {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "overlay opacity must be between 0.0 and 1.0",
        ));
    }
    Ok(())
}

fn validate_color(color: String) -> Result<String, DiagnosableError> {
    let valid = color.len() == 7
        && color.starts_with('#')
        && color[1..].chars().all(|ch| ch.is_ascii_hexdigit());
    if !valid {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("overlay color '{color}' must be #RRGGBB"),
        ));
    }
    Ok(color)
}

fn normalize_id(value: String, field: &'static str) -> Result<String, DiagnosableError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            format!("{field} cannot be empty"),
        ));
    }
    Ok(value.to_string())
}
