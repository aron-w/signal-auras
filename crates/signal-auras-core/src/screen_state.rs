use crate::{
    ActiveProcessContext, AdapterDiagnostic, CapabilityAvailability, CapabilityKind,
    CapabilityReport, CapabilitySet, CapabilityStatus, DiagnosableError, ErrorPhase, ScopeDecision,
    ScopeSelection, ScreenPixelColor,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roi {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Roi {
    pub fn new(x: u32, y: u32, w: u32, h: u32) -> Result<Self, DiagnosableError> {
        if w == 0 || h == 0 {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "screen state tracker ROI width and height must be positive",
            ));
        }
        Ok(Self { x, y, w, h })
    }

    pub fn pixels(&self) -> u32 {
        self.w.saturating_mul(self.h)
    }

    pub fn scaled(&self, scale: ScreenCoordinateScale) -> Self {
        Self {
            x: scale.scale_x_u32(self.x),
            y: scale.scale_y_u32(self.y),
            w: scale.scale_x_u32(self.w).max(1),
            h: scale.scale_y_u32(self.h).max(1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenCoordinateScale {
    pub x: f64,
    pub y: f64,
}

impl ScreenCoordinateScale {
    pub fn identity() -> Self {
        Self { x: 1.0, y: 1.0 }
    }

    pub fn new(x: f64, y: f64) -> Option<Self> {
        if x.is_finite() && y.is_finite() && x > 0.0 && y > 0.0 {
            Some(Self { x, y })
        } else {
            None
        }
    }

    pub fn is_identity(self) -> bool {
        (self.x - 1.0).abs() < f64::EPSILON && (self.y - 1.0).abs() < f64::EPSILON
    }

    fn scale_x_u32(self, value: u32) -> u32 {
        (f64::from(value) * self.x)
            .round()
            .clamp(0.0, f64::from(u32::MAX)) as u32
    }

    fn scale_y_u32(self, value: u32) -> u32 {
        (f64::from(value) * self.y)
            .round()
            .clamp(0.0, f64::from(u32::MAX)) as u32
    }

    fn scale_uniform_u32(self, value: u32) -> u32 {
        (f64::from(value) * ((self.x + self.y) / 2.0))
            .round()
            .clamp(0.0, f64::from(u32::MAX)) as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CircularMask {
    pub inset: u32,
}

impl CircularMask {
    pub fn new(inset: u32) -> Self {
        Self { inset }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressFillDirection {
    LeftToRight,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DetectorDefinition {
    RadialCooldown {
        roi: Roi,
        mask: Option<CircularMask>,
        phases: RadialCooldownPhases,
    },
    HorizontalProgressBar {
        roi: Roi,
        fill_direction: ProgressFillDirection,
    },
}

impl DetectorDefinition {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::RadialCooldown { .. } => "radial_cooldown",
            Self::HorizontalProgressBar { .. } => "horizontal_progress_bar",
        }
    }

    pub fn roi(&self) -> &Roi {
        match self {
            Self::RadialCooldown { roi, .. } | Self::HorizontalProgressBar { roi, .. } => roi,
        }
    }

    pub fn scaled_for_sample(&self, scale: ScreenCoordinateScale) -> Self {
        if scale.is_identity() {
            return self.clone();
        }
        match self {
            Self::RadialCooldown { roi, mask, phases } => Self::RadialCooldown {
                roi: roi.scaled(scale),
                mask: mask.as_ref().map(|mask| CircularMask {
                    inset: scale.scale_uniform_u32(mask.inset),
                }),
                phases: phases.scaled(scale),
            },
            Self::HorizontalProgressBar {
                roi,
                fill_direction,
            } => Self::HorizontalProgressBar {
                roi: roi.scaled(scale),
                fill_direction: *fill_direction,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadialCooldownPhases {
    pub order: Vec<RadialPhaseRule>,
    pub fallback: RadialCooldownPhase,
    pub prediction: Option<RadialCooldownPrediction>,
}

impl RadialCooldownPhases {
    pub fn new(
        order: impl IntoIterator<Item = RadialPhaseRule>,
        fallback: RadialCooldownPhase,
    ) -> Result<Self, DiagnosableError> {
        let order = order.into_iter().collect::<Vec<_>>();
        if order.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "radial_cooldown phases order cannot be empty",
            ));
        }
        if fallback != RadialCooldownPhase::Unknown {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "radial_cooldown phases fallback must be \"unknown\"",
            ));
        }
        Ok(Self {
            order,
            fallback,
            prediction: None,
        })
    }

    pub fn with_prediction(
        mut self,
        prediction: RadialCooldownPrediction,
    ) -> Result<Self, DiagnosableError> {
        prediction.validate()?;
        self.prediction = Some(prediction);
        Ok(self)
    }

    pub fn refutation_default() -> Self {
        Self::new(
            [
                RadialPhaseRule {
                    phase: RadialCooldownPhase::Ready,
                    sample: RadialSampleRegion::ClockProbe {
                        angle_deg: 352.0,
                        radius_px: 15,
                        w: 3,
                        h: 3,
                    },
                    min_luminance_percent: Some(44),
                    max_luminance_percent: None,
                    min_saturation: Some(85),
                    max_saturation: None,
                    metric: RadialRuleMetric::Average,
                    metric_scale: None,
                    progress_fill: RadialProgressFill::Full,
                    max_fill_until_ready: None,
                    fill: None,
                    background: None,
                    opacity: None,
                },
                RadialPhaseRule {
                    phase: RadialCooldownPhase::Activated,
                    sample: RadialSampleRegion::ClockProbe {
                        angle_deg: 8.0,
                        radius_px: 15,
                        w: 3,
                        h: 3,
                    },
                    min_luminance_percent: None,
                    max_luminance_percent: Some(12),
                    min_saturation: None,
                    max_saturation: Some(20),
                    metric: RadialRuleMetric::Average,
                    metric_scale: None,
                    progress_fill: RadialProgressFill::Empty,
                    max_fill_until_ready: None,
                    fill: Some("#f97316".to_string()),
                    background: Some("#7f1d1d".to_string()),
                    opacity: None,
                },
                RadialPhaseRule {
                    phase: RadialCooldownPhase::Active,
                    sample: RadialSampleRegion::ClockProbe {
                        angle_deg: 8.0,
                        radius_px: 15,
                        w: 3,
                        h: 3,
                    },
                    min_luminance_percent: None,
                    max_luminance_percent: Some(34),
                    min_saturation: None,
                    max_saturation: Some(75),
                    metric: RadialRuleMetric::Average,
                    metric_scale: None,
                    progress_fill: RadialProgressFill::Empty,
                    max_fill_until_ready: None,
                    fill: None,
                    background: None,
                    opacity: None,
                },
                RadialPhaseRule {
                    phase: RadialCooldownPhase::Recovering,
                    sample: RadialSampleRegion::AnnulusArc {
                        inner_radius_px: 13,
                        outer_radius_px: 17,
                        start_deg: 20.0,
                        end_deg: 340.0,
                    },
                    min_luminance_percent: Some(40),
                    max_luminance_percent: None,
                    min_saturation: Some(80),
                    max_saturation: None,
                    metric: RadialRuleMetric::BrightRatio,
                    metric_scale: Some(1.5),
                    progress_fill: RadialProgressFill::Fraction,
                    max_fill_until_ready: Some(0.95),
                    fill: None,
                    background: None,
                    opacity: None,
                },
            ],
            RadialCooldownPhase::Unknown,
        )
        .expect("default radial cooldown phase rules are valid")
    }

    fn scaled(&self, scale: ScreenCoordinateScale) -> Self {
        Self {
            order: self.order.iter().map(|rule| rule.scaled(scale)).collect(),
            fallback: self.fallback,
            prediction: self.prediction,
        }
    }

    pub fn validate_for_roi(&self, roi: &Roi) -> Result<(), DiagnosableError> {
        for rule in &self.order {
            rule.validate_for_roi(roi)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadialCooldownPrediction {
    pub duration_ms: u64,
    pub stable_after_ms: u64,
}

impl RadialCooldownPrediction {
    pub fn new(duration_ms: u64, stable_after_ms: u64) -> Result<Self, DiagnosableError> {
        let prediction = Self {
            duration_ms,
            stable_after_ms,
        };
        prediction.validate()?;
        Ok(prediction)
    }

    fn validate(self) -> Result<(), DiagnosableError> {
        if self.duration_ms == 0 {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "radial_cooldown prediction duration_ms must be positive",
            ));
        }
        if self.stable_after_ms > self.duration_ms {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "radial_cooldown prediction stable_after_ms cannot exceed duration_ms",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadialPhaseRule {
    pub phase: RadialCooldownPhase,
    pub sample: RadialSampleRegion,
    pub min_luminance_percent: Option<u8>,
    pub max_luminance_percent: Option<u8>,
    pub min_saturation: Option<u8>,
    pub max_saturation: Option<u8>,
    pub metric: RadialRuleMetric,
    pub metric_scale: Option<f32>,
    pub progress_fill: RadialProgressFill,
    pub max_fill_until_ready: Option<f32>,
    pub fill: Option<String>,
    pub background: Option<String>,
    pub opacity: Option<f32>,
}

impl RadialPhaseRule {
    fn scaled(&self, scale: ScreenCoordinateScale) -> Self {
        let mut rule = self.clone();
        rule.sample = self.sample.scaled(scale);
        rule
    }

    fn validate_for_roi(&self, roi: &Roi) -> Result<(), DiagnosableError> {
        if let Some(value) = self.min_luminance_percent {
            validate_luminance_threshold(value)?;
        }
        if let Some(value) = self.max_luminance_percent {
            validate_luminance_threshold(value)?;
        }
        if let Some(value) = self.min_saturation {
            validate_saturation_threshold(value)?;
        }
        if let Some(value) = self.max_saturation {
            validate_saturation_threshold(value)?;
        }
        if let Some(opacity) = self.opacity {
            if !(0.0..=1.0).contains(&opacity) {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    "radial_cooldown phase opacity must be between 0 and 1",
                ));
            }
        }
        if let Some(scale) = self.metric_scale {
            if !scale.is_finite() || scale <= 0.0 {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    "radial_cooldown phase metric_scale must be positive",
                ));
            }
        }
        self.sample.validate_for_roi(roi)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RadialSampleRegion {
    ClockProbe {
        angle_deg: f32,
        radius_px: u32,
        w: u32,
        h: u32,
    },
    AnnulusArc {
        inner_radius_px: u32,
        outer_radius_px: u32,
        start_deg: f32,
        end_deg: f32,
    },
    AggregateMask,
}

impl RadialSampleRegion {
    fn scaled(self, scale: ScreenCoordinateScale) -> Self {
        match self {
            Self::ClockProbe {
                angle_deg,
                radius_px,
                w,
                h,
            } => Self::ClockProbe {
                angle_deg,
                radius_px: scale.scale_uniform_u32(radius_px).max(1),
                w: scale.scale_x_u32(w).max(1),
                h: scale.scale_y_u32(h).max(1),
            },
            Self::AnnulusArc {
                inner_radius_px,
                outer_radius_px,
                start_deg,
                end_deg,
            } => Self::AnnulusArc {
                inner_radius_px: scale.scale_uniform_u32(inner_radius_px).max(1),
                outer_radius_px: scale.scale_uniform_u32(outer_radius_px).max(1),
                start_deg,
                end_deg,
            },
            Self::AggregateMask => Self::AggregateMask,
        }
    }

    fn validate_for_roi(self, roi: &Roi) -> Result<(), DiagnosableError> {
        match self {
            Self::ClockProbe {
                angle_deg,
                radius_px,
                w,
                h,
            } => {
                if !angle_deg.is_finite() {
                    return Err(DiagnosableError::new(
                        ErrorPhase::ScriptValidation,
                        "clock_probe angle_deg must be finite",
                    ));
                }
                if radius_px == 0 || w == 0 || h == 0 {
                    return Err(DiagnosableError::new(
                        ErrorPhase::ScriptValidation,
                        "clock_probe radius_px, w, and h must be positive",
                    ));
                }
                if clock_probe_rect(roi, angle_deg, radius_px, w, h).is_none() {
                    return Err(DiagnosableError::new(
                        ErrorPhase::ScriptValidation,
                        "clock_probe must fit inside the radial_cooldown ROI",
                    ));
                }
                Ok(())
            }
            Self::AnnulusArc {
                inner_radius_px,
                outer_radius_px,
                start_deg,
                end_deg,
            } => {
                if inner_radius_px >= outer_radius_px {
                    return Err(DiagnosableError::new(
                        ErrorPhase::ScriptValidation,
                        "annulus_arc inner_radius_px must be less than outer_radius_px",
                    ));
                }
                if !start_deg.is_finite() || !end_deg.is_finite() {
                    return Err(DiagnosableError::new(
                        ErrorPhase::ScriptValidation,
                        "annulus_arc start_deg and end_deg must be finite",
                    ));
                }
                let max_radius = (roi.w.min(roi.h) as f32) / 2.0;
                if outer_radius_px as f32 > max_radius {
                    return Err(DiagnosableError::new(
                        ErrorPhase::ScriptValidation,
                        "annulus_arc outer_radius_px must fit inside the radial_cooldown ROI",
                    ));
                }
                Ok(())
            }
            Self::AggregateMask => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadialRuleMetric {
    Average,
    BrightRatio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadialProgressFill {
    Empty,
    Fraction,
    Full,
}

fn validate_luminance_threshold(value: u8) -> Result<(), DiagnosableError> {
    if value <= 100 {
        Ok(())
    } else {
        Err(DiagnosableError::new(
            ErrorPhase::ScriptValidation,
            "radial_cooldown luminance threshold must be between 0 and 100",
        ))
    }
}

fn validate_saturation_threshold(_value: u8) -> Result<(), DiagnosableError> {
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateTrackerDefinition {
    pub id: String,
    pub scope: ScopeSelection,
    pub capabilities: CapabilitySet,
    pub poll_ms: u64,
    pub detector: DetectorDefinition,
    pub condition: Option<StateTrackerCondition>,
}

impl StateTrackerDefinition {
    pub fn new(
        id: impl Into<String>,
        scope: ScopeSelection,
        capabilities: CapabilitySet,
        poll_ms: u64,
        detector: DetectorDefinition,
    ) -> Result<Self, DiagnosableError> {
        let id = id.into().trim().to_string();
        if id.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "state tracker id cannot be empty",
            ));
        }
        if poll_ms == 0 {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "state tracker poll_ms must be positive",
            ));
        }
        if !capabilities.contains(CapabilityKind::ScreenRead) {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "state tracker requires screen_read capability",
            ));
        }
        Ok(Self {
            id,
            scope,
            capabilities,
            poll_ms,
            detector,
            condition: None,
        })
    }

    pub fn only_when(mut self, condition: StateTrackerCondition) -> Self {
        self.condition = Some(condition);
        self
    }

    pub fn required_capabilities(&self) -> CapabilitySet {
        let mut required = self.capabilities.iter().collect::<Vec<_>>();
        if matches!(self.scope, ScopeSelection::ProcessList { .. }) {
            required.push(CapabilityKind::ActiveProcessMetadata);
        }
        CapabilitySet::new(required)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateTrackerCondition {
    pub tracker_id: String,
    pub phase: RadialCooldownPhase,
}

impl StateTrackerCondition {
    pub fn radial_phase(
        tracker_id: impl Into<String>,
        phase: RadialCooldownPhase,
    ) -> Result<Self, DiagnosableError> {
        let tracker_id = tracker_id.into().trim().to_string();
        if tracker_id.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "state tracker condition tracker cannot be empty",
            ));
        }
        Ok(Self { tracker_id, phase })
    }

    fn matches(&self, states: &BTreeMap<String, TrackerState>) -> bool {
        matches!(
            states.get(&self.tracker_id),
            Some(TrackerState::RadialCooldown { phase, .. }) if *phase == self.phase
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StateTrackerDefinitionSet {
    trackers: Vec<StateTrackerDefinition>,
    required_capabilities: CapabilitySet,
}

impl StateTrackerDefinitionSet {
    pub fn new(
        trackers: impl IntoIterator<Item = StateTrackerDefinition>,
    ) -> Result<Self, DiagnosableError> {
        let trackers = trackers.into_iter().collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        let mut required = Vec::new();
        for tracker in &trackers {
            if !seen.insert(tracker.id.clone()) {
                return Err(DiagnosableError::new(
                    ErrorPhase::ScriptValidation,
                    format!("duplicate state tracker id '{}'", tracker.id),
                ));
            }
            required.extend(tracker.required_capabilities().iter());
        }
        let mut declared_before = BTreeSet::new();
        for tracker in &trackers {
            if let Some(condition) = &tracker.condition {
                if condition.tracker_id == tracker.id {
                    return Err(DiagnosableError::new(
                        ErrorPhase::ScriptValidation,
                        format!("state tracker '{}' cannot depend on itself", tracker.id),
                    ));
                }
                if !seen.contains(&condition.tracker_id) {
                    return Err(DiagnosableError::new(
                        ErrorPhase::ScriptValidation,
                        format!(
                            "state tracker '{}' condition references missing tracker '{}'",
                            tracker.id, condition.tracker_id
                        ),
                    ));
                }
                if !declared_before.contains(&condition.tracker_id) {
                    return Err(DiagnosableError::new(
                        ErrorPhase::ScriptValidation,
                        format!(
                            "state tracker '{}' condition source '{}' must be declared first",
                            tracker.id, condition.tracker_id
                        ),
                    ));
                }
            }
            declared_before.insert(tracker.id.clone());
        }
        Ok(Self {
            trackers,
            required_capabilities: CapabilitySet::new(required),
        })
    }

    pub fn trackers(&self) -> &[StateTrackerDefinition] {
        &self.trackers
    }

    pub fn required_capabilities(&self) -> &CapabilitySet {
        &self.required_capabilities
    }

    pub fn is_empty(&self) -> bool {
        self.trackers.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenPixelFormat {
    Luma8,
    Rgb888,
    Bgr888,
    Rgba8888,
    Bgra8888,
    Rgbx8888,
    Bgrx8888,
}

impl ScreenPixelFormat {
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Luma8 => 1,
            Self::Rgb888 | Self::Bgr888 => 3,
            Self::Rgba8888 | Self::Bgra8888 | Self::Rgbx8888 | Self::Bgrx8888 => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenSample {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixel_format: ScreenPixelFormat,
    pub captured_at_ms: u64,
    pub pixels: Vec<u8>,
}

impl ScreenSample {
    pub fn new(captured_at_ms: u64, bytes: impl Into<Vec<u8>>) -> Self {
        let pixels = bytes.into();
        Self {
            width: pixels.len() as u32,
            height: if pixels.is_empty() { 0 } else { 1 },
            stride: pixels.len() as u32,
            pixel_format: ScreenPixelFormat::Luma8,
            captured_at_ms,
            pixels,
        }
    }

    pub fn from_pixels(
        width: u32,
        height: u32,
        stride: u32,
        pixel_format: ScreenPixelFormat,
        captured_at_ms: u64,
        pixels: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            width,
            height,
            stride,
            pixel_format,
            captured_at_ms,
            pixels: pixels.into(),
        }
    }

    pub fn synthetic_percent(captured_at_ms: u64, percent: u8) -> Self {
        Self::new(captured_at_ms, [percent.min(100)])
    }

    pub fn pixel_color(&self, x: u32, y: u32) -> Option<ScreenPixelColor> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let bytes_per_pixel = self.pixel_format.bytes_per_pixel();
        let stride = self.stride as usize;
        if stride < self.width as usize * bytes_per_pixel {
            return None;
        }
        let offset = y as usize * stride + x as usize * bytes_per_pixel;
        let pixel = self.pixels.get(offset..offset + bytes_per_pixel)?;
        match self.pixel_format {
            ScreenPixelFormat::Luma8 => {
                let value = pixel[0];
                Some(ScreenPixelColor::rgb(value, value, value))
            }
            ScreenPixelFormat::Rgb888 | ScreenPixelFormat::Rgbx8888 => {
                Some(ScreenPixelColor::rgb(pixel[0], pixel[1], pixel[2]))
            }
            ScreenPixelFormat::Bgr888 | ScreenPixelFormat::Bgrx8888 => {
                Some(ScreenPixelColor::rgb(pixel[2], pixel[1], pixel[0]))
            }
            ScreenPixelFormat::Rgba8888 => Some(ScreenPixelColor::rgba(
                pixel[0], pixel[1], pixel[2], pixel[3],
            )),
            ScreenPixelFormat::Bgra8888 => Some(ScreenPixelColor::rgba(
                pixel[2], pixel[1], pixel[0], pixel[3],
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackerState {
    RadialCooldown {
        phase: RadialCooldownPhase,
        ready: bool,
        cooldown_fraction: u8,
        remaining_ms: Option<u64>,
        total_estimated_ms: Option<u64>,
        predicted_remaining_ms: Option<u64>,
        predicted_duration_ms: Option<u64>,
        confidence: u8,
        freshness_ms: u64,
    },
    HorizontalProgressBar {
        visible: bool,
        progress_percent: u8,
        confidence: u8,
        freshness_ms: u64,
    },
    Inactive {
        reason: TrackerInactiveReason,
        confidence: u8,
        freshness_ms: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadialCooldownPhase {
    Ready,
    Activated,
    Active,
    Recovering,
    Unknown,
}

impl TrackerState {
    pub fn confidence(&self) -> u8 {
        match self {
            Self::RadialCooldown { confidence, .. }
            | Self::HorizontalProgressBar { confidence, .. }
            | Self::Inactive { confidence, .. } => *confidence,
        }
    }

    pub fn summary(&self) -> String {
        match self {
            Self::RadialCooldown {
                phase,
                ready,
                cooldown_fraction,
                remaining_ms,
                predicted_remaining_ms,
                confidence,
                ..
            } => format!(
                "radial_cooldown phase={phase:?} ready={ready} fraction={cooldown_fraction} remaining_ms={remaining_ms:?} predicted_remaining_ms={predicted_remaining_ms:?} confidence={confidence}"
            ),
            Self::HorizontalProgressBar {
                visible,
                progress_percent,
                confidence,
                ..
            } => format!(
                "horizontal_progress_bar visible={visible} progress={progress_percent} confidence={confidence}"
            ),
            Self::Inactive {
                reason,
                confidence,
                ..
            } => format!("inactive reason={reason:?} confidence={confidence}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackerInactiveReason {
    ScreenReadDenied,
    ScreenReadUnsupported,
    FocusInactive,
    ConditionInactive,
    NoReadableSample,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RadialCooldownHistory {
    observations: Vec<CooldownObservation>,
    last_total_estimate_ms: Option<u64>,
    last_phase: Option<RadialCooldownPhase>,
    active_started_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CooldownObservation {
    at_ms: u64,
    fraction: u8,
}

const RADIAL_READY_LUMINANCE_PERCENT: u8 = 40;
const RADIAL_FULL_COOLDOWN_LUMINANCE_PERCENT: u8 = 10;

impl RadialCooldownHistory {
    fn push(&mut self, at_ms: u64, fraction: u8) {
        self.observations
            .push(CooldownObservation { at_ms, fraction });
        if self.observations.len() > 16 {
            self.observations.remove(0);
        }
    }

    fn observe_phase(&mut self, at_ms: u64, phase: RadialCooldownPhase) {
        if phase == RadialCooldownPhase::Active {
            if self.last_phase != Some(RadialCooldownPhase::Active) {
                self.active_started_at_ms = Some(at_ms);
            }
        } else {
            self.active_started_at_ms = None;
        }
        self.last_phase = Some(phase);
    }

    fn estimate_total_ms(&mut self) -> Option<u64> {
        let latest = *self.observations.last()?;
        let earliest = self
            .observations
            .iter()
            .copied()
            .find(|observation| observation.fraction > latest.fraction)?;
        let elapsed = latest.at_ms.checked_sub(earliest.at_ms)?;
        let progressed = u64::from(earliest.fraction.saturating_sub(latest.fraction));
        if elapsed == 0 || progressed == 0 {
            return self.last_total_estimate_ms;
        }
        let estimate = elapsed.saturating_mul(100) / progressed;
        self.last_total_estimate_ms = Some(estimate);
        Some(estimate)
    }

    fn predicted_remaining_ms(
        &self,
        now_ms: u64,
        prediction: Option<RadialCooldownPrediction>,
    ) -> Option<u64> {
        let prediction = prediction?;
        let active_started_at_ms = self.active_started_at_ms?;
        let elapsed = now_ms.checked_sub(active_started_at_ms)?;
        if elapsed < prediction.stable_after_ms || elapsed > prediction.duration_ms {
            return None;
        }
        Some(prediction.duration_ms.saturating_sub(elapsed))
    }
}

pub fn detect_radial_cooldown(
    sample: &ScreenSample,
    history: &mut RadialCooldownHistory,
) -> TrackerState {
    detect_radial_cooldown_with_roi(sample, None, history)
}

fn detect_radial_cooldown_with_roi(
    sample: &ScreenSample,
    detector: Option<&DetectorDefinition>,
    history: &mut RadialCooldownHistory,
) -> TrackerState {
    let Some(observation) = observe_radial_cooldown(sample, detector) else {
        return TrackerState::Inactive {
            reason: TrackerInactiveReason::NoReadableSample,
            confidence: 0,
            freshness_ms: 0,
        };
    };
    let phase = observation.phase;
    let fraction = observation.cooldown_fraction.min(100);
    history.observe_phase(sample.captured_at_ms, phase);
    history.push(sample.captured_at_ms, fraction);
    let total_estimated_ms = history.estimate_total_ms();
    let prediction = detector.and_then(|detector| match detector {
        DetectorDefinition::RadialCooldown { phases, .. } => phases.prediction,
        DetectorDefinition::HorizontalProgressBar { .. } => None,
    });
    let predicted_remaining_ms = if phase == RadialCooldownPhase::Active {
        history.predicted_remaining_ms(sample.captured_at_ms, prediction)
    } else {
        None
    };
    let ready = phase == RadialCooldownPhase::Ready;
    let remaining_ms = if ready {
        Some(0)
    } else if phase == RadialCooldownPhase::Unknown {
        None
    } else {
        total_estimated_ms.map(|total| total.saturating_mul(u64::from(fraction)) / 100)
    };
    TrackerState::RadialCooldown {
        phase,
        ready,
        cooldown_fraction: if ready { 0 } else { fraction },
        remaining_ms,
        total_estimated_ms,
        predicted_remaining_ms,
        predicted_duration_ms: predicted_remaining_ms
            .and_then(|_| prediction.map(|p| p.duration_ms)),
        confidence: confidence_for_sample(sample),
        freshness_ms: 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RadialCooldownObservation {
    cooldown_fraction: u8,
    phase: RadialCooldownPhase,
}

fn observe_radial_cooldown(
    sample: &ScreenSample,
    detector: Option<&DetectorDefinition>,
) -> Option<RadialCooldownObservation> {
    if sample.pixel_format == ScreenPixelFormat::Luma8 && sample.pixels.len() == 1 {
        let cooldown_fraction = observed_percent(sample, detector)?.min(100);
        let phase = if cooldown_fraction <= 2 {
            RadialCooldownPhase::Ready
        } else if cooldown_fraction >= 98 {
            RadialCooldownPhase::Activated
        } else {
            RadialCooldownPhase::Active
        };
        return Some(RadialCooldownObservation {
            cooldown_fraction,
            phase,
        });
    }

    let Some(DetectorDefinition::RadialCooldown { roi, mask, phases }) = detector else {
        let stats = observed_radial_stats(sample, detector)?;
        return Some(classify_grayscale_radial_observation(
            stats.luminance_percent,
        ));
    };

    observe_configured_radial_cooldown(sample, roi, mask.as_ref(), phases)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RadialPixelStats {
    luminance_percent: u8,
    saturation: u8,
    bright_ratio_percent: u8,
}

fn observed_radial_stats(
    sample: &ScreenSample,
    detector: Option<&DetectorDefinition>,
) -> Option<RadialPixelStats> {
    if sample.pixels.is_empty() || sample.width == 0 || sample.height == 0 {
        return None;
    }
    let default_roi = Roi {
        x: 0,
        y: 0,
        w: sample.width,
        h: sample.height,
    };
    let roi = detector.map_or(&default_roi, DetectorDefinition::roi);
    let x_end = roi.x.checked_add(roi.w)?;
    let y_end = roi.y.checked_add(roi.h)?;
    if x_end > sample.width || y_end > sample.height {
        return None;
    }
    let bytes_per_pixel = sample.pixel_format.bytes_per_pixel();
    let min_stride = sample.width as usize * bytes_per_pixel;
    let stride = sample.stride as usize;
    if stride < min_stride {
        return None;
    }

    let mut luminance_sum = 0u64;
    let mut saturation_sum = 0u64;
    let mut count = 0u64;
    for y in roi.y..y_end {
        for x in roi.x..x_end {
            if !pixel_in_detector_mask(detector, roi, x, y) {
                continue;
            }
            let offset = y as usize * stride + x as usize * bytes_per_pixel;
            let pixel = sample.pixels.get(offset..offset + bytes_per_pixel)?;
            let (r, g, b) = pixel_rgb(pixel, sample.pixel_format)?;
            luminance_sum = luminance_sum.saturating_add(rgb_luminance(r, g, b));
            saturation_sum =
                saturation_sum.saturating_add(u64::from(r.max(g).max(b) - r.min(g).min(b)));
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    let luminance = luminance_sum / count;
    let luminance_percent = if sample.pixel_format == ScreenPixelFormat::Luma8 {
        luminance.min(100)
    } else {
        (luminance * 100 / 255).min(100)
    };
    Some(RadialPixelStats {
        luminance_percent: luminance_percent as u8,
        saturation: (saturation_sum / count).min(255) as u8,
        bright_ratio_percent: 0,
    })
}

fn observe_configured_radial_cooldown(
    sample: &ScreenSample,
    roi: &Roi,
    mask: Option<&CircularMask>,
    phases: &RadialCooldownPhases,
) -> Option<RadialCooldownObservation> {
    for rule in &phases.order {
        let stats = observed_radial_rule_stats(sample, roi, mask, rule)?;
        if !radial_rule_matches(rule, stats) {
            continue;
        }
        return Some(RadialCooldownObservation {
            cooldown_fraction: cooldown_fraction_for_rule(rule, stats),
            phase: rule.phase,
        });
    }
    Some(RadialCooldownObservation {
        cooldown_fraction: 100,
        phase: phases.fallback,
    })
}

fn radial_rule_matches(rule: &RadialPhaseRule, stats: RadialPixelStats) -> bool {
    if rule.metric == RadialRuleMetric::BrightRatio {
        return stats.bright_ratio_percent > 0;
    }
    if let Some(minimum) = rule.min_luminance_percent {
        if stats.luminance_percent < minimum {
            return false;
        }
    }
    if let Some(maximum) = rule.max_luminance_percent {
        if stats.luminance_percent > maximum {
            return false;
        }
    }
    if let Some(minimum) = rule.min_saturation {
        if stats.saturation < minimum {
            return false;
        }
    }
    if let Some(maximum) = rule.max_saturation {
        if stats.saturation > maximum {
            return false;
        }
    }
    true
}

fn cooldown_fraction_for_rule(rule: &RadialPhaseRule, stats: RadialPixelStats) -> u8 {
    match rule.progress_fill {
        RadialProgressFill::Empty => 100,
        RadialProgressFill::Full => 0,
        RadialProgressFill::Fraction => {
            let scale = rule.metric_scale.unwrap_or(1.0);
            let mut fill_percent =
                (f32::from(stats.bright_ratio_percent.min(100)) * scale).round() as u8;
            fill_percent = fill_percent.min(100);
            if let Some(max_fill) = rule.max_fill_until_ready {
                let max_percent = (max_fill.clamp(0.0, 1.0) * 100.0).floor() as u8;
                fill_percent = fill_percent.min(max_percent);
            }
            100u8.saturating_sub(fill_percent)
        }
    }
}

fn observed_radial_rule_stats(
    sample: &ScreenSample,
    roi: &Roi,
    mask: Option<&CircularMask>,
    rule: &RadialPhaseRule,
) -> Option<RadialPixelStats> {
    if sample.pixels.is_empty() || sample.width == 0 || sample.height == 0 {
        return None;
    }
    let x_end = roi.x.checked_add(roi.w)?;
    let y_end = roi.y.checked_add(roi.h)?;
    if x_end > sample.width || y_end > sample.height {
        return None;
    }
    let bytes_per_pixel = sample.pixel_format.bytes_per_pixel();
    let min_stride = sample.width as usize * bytes_per_pixel;
    let stride = sample.stride as usize;
    if stride < min_stride {
        return None;
    }

    let mut luminance_sum = 0u64;
    let mut saturation_sum = 0u64;
    let mut bright_count = 0u64;
    let mut count = 0u64;
    for y in roi.y..y_end {
        for x in roi.x..x_end {
            if !pixel_in_sample_region(&rule.sample, roi, x, y) {
                continue;
            }
            if rule.sample == RadialSampleRegion::AggregateMask
                && !pixel_in_circular_mask(mask, roi, x, y)
            {
                continue;
            }
            let offset = y as usize * stride + x as usize * bytes_per_pixel;
            let pixel = sample.pixels.get(offset..offset + bytes_per_pixel)?;
            let (r, g, b) = pixel_rgb(pixel, sample.pixel_format)?;
            let luminance = if sample.pixel_format == ScreenPixelFormat::Luma8 {
                rgb_luminance(r, g, b).min(100)
            } else {
                (rgb_luminance(r, g, b) * 100 / 255).min(100)
            };
            let saturation = u64::from(r.max(g).max(b) - r.min(g).min(b));
            luminance_sum = luminance_sum.saturating_add(luminance);
            saturation_sum = saturation_sum.saturating_add(saturation);
            if rule.metric == RadialRuleMetric::BrightRatio {
                let bright_enough = rule
                    .min_luminance_percent
                    .is_none_or(|minimum| luminance >= u64::from(minimum))
                    && rule
                        .min_saturation
                        .is_none_or(|minimum| saturation >= u64::from(minimum));
                if bright_enough {
                    bright_count += 1;
                }
            }
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    Some(RadialPixelStats {
        luminance_percent: (luminance_sum / count).min(100) as u8,
        saturation: (saturation_sum / count).min(255) as u8,
        bright_ratio_percent: ((bright_count * 100) / count).min(100) as u8,
    })
}

fn classify_grayscale_radial_observation(observed: u8) -> RadialCooldownObservation {
    if observed >= RADIAL_READY_LUMINANCE_PERCENT {
        return RadialCooldownObservation {
            cooldown_fraction: 0,
            phase: RadialCooldownPhase::Ready,
        };
    }
    if observed <= RADIAL_FULL_COOLDOWN_LUMINANCE_PERCENT {
        return RadialCooldownObservation {
            cooldown_fraction: 100,
            phase: RadialCooldownPhase::Activated,
        };
    }
    let cooldown_range = RADIAL_READY_LUMINANCE_PERCENT - RADIAL_FULL_COOLDOWN_LUMINANCE_PERCENT;
    let dark_delta = RADIAL_READY_LUMINANCE_PERCENT - observed;
    RadialCooldownObservation {
        cooldown_fraction: ((u64::from(dark_delta) * 100) / u64::from(cooldown_range)).min(100)
            as u8,
        phase: RadialCooldownPhase::Active,
    }
}

pub fn detect_horizontal_progress_bar(sample: &ScreenSample) -> TrackerState {
    detect_horizontal_progress_bar_with_roi(sample, None)
}

fn detect_horizontal_progress_bar_with_roi(
    sample: &ScreenSample,
    detector: Option<&DetectorDefinition>,
) -> TrackerState {
    let Some(progress) = observed_percent(sample, detector) else {
        return TrackerState::HorizontalProgressBar {
            visible: false,
            progress_percent: 0,
            confidence: 0,
            freshness_ms: 0,
        };
    };
    let confidence = confidence_for_sample(sample);
    TrackerState::HorizontalProgressBar {
        visible: confidence >= 50,
        progress_percent: progress.min(100),
        confidence,
        freshness_ms: 0,
    }
}

fn observed_percent(sample: &ScreenSample, detector: Option<&DetectorDefinition>) -> Option<u8> {
    if sample.pixels.is_empty() || sample.width == 0 || sample.height == 0 {
        return None;
    }
    let default_roi = Roi {
        x: 0,
        y: 0,
        w: sample.width,
        h: sample.height,
    };
    let roi = detector.map_or(&default_roi, DetectorDefinition::roi);
    let x_end = roi.x.checked_add(roi.w)?;
    let y_end = roi.y.checked_add(roi.h)?;
    if x_end > sample.width || y_end > sample.height {
        return None;
    }
    let bytes_per_pixel = sample.pixel_format.bytes_per_pixel();
    let min_stride = sample.width as usize * bytes_per_pixel;
    let stride = sample.stride as usize;
    if stride < min_stride {
        return None;
    }

    let mut sum = 0u64;
    let mut filled = 0u64;
    let mut count = 0u64;
    for y in roi.y..y_end {
        for x in roi.x..x_end {
            if !pixel_in_detector_mask(detector, roi, x, y) {
                continue;
            }
            let offset = y as usize * stride + x as usize * bytes_per_pixel;
            let pixel = sample.pixels.get(offset..offset + bytes_per_pixel)?;
            if sample.pixel_format == ScreenPixelFormat::Luma8 {
                sum = sum.saturating_add(pixel_percent(pixel, sample.pixel_format)?);
            } else if horizontal_progress_pixel_filled(pixel, sample.pixel_format)? {
                filled += 1;
            }
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    if sample.pixel_format == ScreenPixelFormat::Luma8 {
        Some((sum / count).min(100) as u8)
    } else {
        Some(((filled * 100) / count).min(100) as u8)
    }
}

fn confidence_for_sample(sample: &ScreenSample) -> u8 {
    if sample.pixels.is_empty() {
        0
    } else if sample.pixel_format == ScreenPixelFormat::Luma8 && sample.pixels.len() == 1 {
        90
    } else {
        95
    }
}

fn pixel_in_detector_mask(
    detector: Option<&DetectorDefinition>,
    roi: &Roi,
    x: u32,
    y: u32,
) -> bool {
    let Some(DetectorDefinition::RadialCooldown {
        mask: Some(mask), ..
    }) = detector
    else {
        return true;
    };
    pixel_in_circular_mask(Some(mask), roi, x, y)
}

fn pixel_in_circular_mask(mask: Option<&CircularMask>, roi: &Roi, x: u32, y: u32) -> bool {
    let Some(mask) = mask else {
        return true;
    };
    let inset = mask.inset.min(roi.w / 2).min(roi.h / 2);
    let inner_w = roi.w.saturating_sub(inset * 2);
    let inner_h = roi.h.saturating_sub(inset * 2);
    if inner_w == 0 || inner_h == 0 {
        return false;
    }
    let local_x = x.saturating_sub(roi.x);
    let local_y = y.saturating_sub(roi.y);
    let center_x2 = (roi.w - 1) as i64;
    let center_y2 = (roi.h - 1) as i64;
    let dx2 = local_x as i64 * 2 - center_x2;
    let dy2 = local_y as i64 * 2 - center_y2;
    let radius_x2 = inner_w as i64;
    let radius_y2 = inner_h as i64;
    dx2 * dx2 * radius_y2 * radius_y2 + dy2 * dy2 * radius_x2 * radius_x2
        <= radius_x2 * radius_x2 * radius_y2 * radius_y2
}

fn pixel_in_sample_region(region: &RadialSampleRegion, roi: &Roi, x: u32, y: u32) -> bool {
    match *region {
        RadialSampleRegion::ClockProbe {
            angle_deg,
            radius_px,
            w,
            h,
        } => clock_probe_rect(roi, angle_deg, radius_px, w, h).is_some_and(
            |(left, top, right, bottom)| x >= left && x < right && y >= top && y < bottom,
        ),
        RadialSampleRegion::AnnulusArc {
            inner_radius_px,
            outer_radius_px,
            start_deg,
            end_deg,
        } => {
            let (dx, dy) = local_center_delta(roi, x, y);
            let distance = (dx * dx + dy * dy).sqrt();
            if distance < inner_radius_px as f32 || distance > outer_radius_px as f32 {
                return false;
            }
            angle_in_clock_arc(clock_degrees(dx, dy), start_deg, end_deg)
        }
        RadialSampleRegion::AggregateMask => true,
    }
}

fn clock_probe_rect(
    roi: &Roi,
    angle_deg: f32,
    radius_px: u32,
    w: u32,
    h: u32,
) -> Option<(u32, u32, u32, u32)> {
    if radius_px == 0 || w == 0 || h == 0 || !angle_deg.is_finite() {
        return None;
    }
    let angle = angle_deg.to_radians();
    let center_x = roi.x as f32 + (roi.w as f32 - 1.0) / 2.0;
    let center_y = roi.y as f32 + (roi.h as f32 - 1.0) / 2.0;
    let probe_center_x = center_x + angle.sin() * radius_px as f32;
    let probe_center_y = center_y - angle.cos() * radius_px as f32;
    let left = (probe_center_x - (w as f32 - 1.0) / 2.0).round() as i64;
    let top = (probe_center_y - (h as f32 - 1.0) / 2.0).round() as i64;
    let right = left + i64::from(w);
    let bottom = top + i64::from(h);
    let roi_left = i64::from(roi.x);
    let roi_top = i64::from(roi.y);
    let roi_right = i64::from(roi.x.checked_add(roi.w)?);
    let roi_bottom = i64::from(roi.y.checked_add(roi.h)?);
    if left < roi_left || top < roi_top || right > roi_right || bottom > roi_bottom {
        return None;
    }
    Some((left as u32, top as u32, right as u32, bottom as u32))
}

fn local_center_delta(roi: &Roi, x: u32, y: u32) -> (f32, f32) {
    let center_x = roi.x as f32 + (roi.w as f32 - 1.0) / 2.0;
    let center_y = roi.y as f32 + (roi.h as f32 - 1.0) / 2.0;
    (x as f32 - center_x, y as f32 - center_y)
}

fn clock_degrees(dx: f32, dy: f32) -> f32 {
    let degrees = dx.atan2(-dy).to_degrees();
    if degrees < 0.0 {
        degrees + 360.0
    } else {
        degrees
    }
}

fn angle_in_clock_arc(angle: f32, start: f32, end: f32) -> bool {
    let angle = normalize_degrees(angle);
    let start = normalize_degrees(start);
    let end = normalize_degrees(end);
    if start <= end {
        angle >= start && angle <= end
    } else {
        angle >= start || angle <= end
    }
}

fn normalize_degrees(value: f32) -> f32 {
    value.rem_euclid(360.0)
}

fn pixel_percent(pixel: &[u8], format: ScreenPixelFormat) -> Option<u64> {
    let (r, g, b) = pixel_rgb(pixel, format)?;
    let luminance = rgb_luminance(r, g, b);
    if format == ScreenPixelFormat::Luma8 {
        Some(luminance.min(100))
    } else {
        Some((luminance * 100 / 255).min(100))
    }
}

fn horizontal_progress_pixel_filled(pixel: &[u8], format: ScreenPixelFormat) -> Option<bool> {
    let (r, g, b) = pixel_rgb(pixel, format)?;
    let luminance = rgb_luminance(r, g, b);
    let warm_bias = ((u64::from(r) + u64::from(g)) / 2).saturating_sub(u64::from(b));
    Some(luminance >= 220 || (luminance > 35 && warm_bias > 8))
}

fn pixel_rgb(pixel: &[u8], format: ScreenPixelFormat) -> Option<(u8, u8, u8)> {
    match format {
        ScreenPixelFormat::Luma8 => {
            let value = *pixel.first()?;
            Some((value, value, value))
        }
        ScreenPixelFormat::Rgb888 | ScreenPixelFormat::Rgba8888 | ScreenPixelFormat::Rgbx8888 => {
            Some((pixel[0], pixel[1], pixel[2]))
        }
        ScreenPixelFormat::Bgr888 | ScreenPixelFormat::Bgra8888 | ScreenPixelFormat::Bgrx8888 => {
            Some((pixel[2], pixel[1], pixel[0]))
        }
    }
}

fn rgb_luminance(r: u8, g: u8, b: u8) -> u64 {
    (u64::from(r) * 299 + u64::from(g) * 587 + u64::from(b) * 114) / 1000
}

pub trait ScreenSampleProvider {
    fn capture_screen_sample(&mut self) -> Result<ScreenSample, DiagnosableError>;

    fn coordinate_scale_for_sample(
        &mut self,
        _sample: &ScreenSample,
        _active_context: &ActiveProcessContext,
    ) -> ScreenCoordinateScale {
        ScreenCoordinateScale::identity()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenSampleDiagnostic {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixel_format: ScreenPixelFormat,
}

impl From<&ScreenSample> for ScreenSampleDiagnostic {
    fn from(sample: &ScreenSample) -> Self {
        Self {
            width: sample.width,
            height: sample.height,
            stride: sample.stride,
            pixel_format: sample.pixel_format,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollOutcome {
    pub due_trackers: usize,
    pub screen_samples: usize,
    pub updated: Vec<String>,
    pub sample: Option<ScreenSampleDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct StateTrackerPoller {
    trackers: StateTrackerDefinitionSet,
    last_poll_ms: BTreeMap<String, u64>,
    latest: BTreeMap<String, TrackerState>,
    cooldown_history: BTreeMap<String, RadialCooldownHistory>,
}

impl StateTrackerPoller {
    pub fn new(trackers: StateTrackerDefinitionSet) -> Self {
        Self {
            trackers,
            last_poll_ms: BTreeMap::new(),
            latest: BTreeMap::new(),
            cooldown_history: BTreeMap::new(),
        }
    }

    pub fn latest_state(&self, id: &str) -> Option<&TrackerState> {
        self.latest.get(id)
    }

    pub fn latest_states(&self) -> &BTreeMap<String, TrackerState> {
        &self.latest
    }

    pub fn next_due_in_ms(&self, now_ms: u64) -> Option<u64> {
        self.trackers
            .trackers()
            .iter()
            .map(|tracker| {
                self.last_poll_ms.get(&tracker.id).map_or(0, |last| {
                    tracker.poll_ms.saturating_sub(now_ms.saturating_sub(*last))
                })
            })
            .min()
    }

    pub fn poll_due(
        &mut self,
        now_ms: u64,
        capabilities: &CapabilityReport,
        active_context: &ActiveProcessContext,
        sample_provider: &mut impl ScreenSampleProvider,
    ) -> PollOutcome {
        let due_ids = self
            .trackers
            .trackers()
            .iter()
            .filter(|tracker| {
                self.last_poll_ms
                    .get(&tracker.id)
                    .is_none_or(|last| now_ms.saturating_sub(*last) >= tracker.poll_ms)
            })
            .map(|tracker| tracker.id.clone())
            .collect::<Vec<_>>();
        if due_ids.is_empty() {
            return PollOutcome {
                due_trackers: 0,
                screen_samples: 0,
                updated: Vec::new(),
                sample: None,
            };
        }

        let screen_required = CapabilitySet::new([CapabilityKind::ScreenRead]);
        if let Some(error) = capabilities.first_blocking_error(&screen_required) {
            let reason = if error.message.contains("unsupported") {
                TrackerInactiveReason::ScreenReadUnsupported
            } else {
                TrackerInactiveReason::ScreenReadDenied
            };
            return self.mark_inactive(now_ms, &due_ids, reason);
        }

        let mut active_due_ids = Vec::new();
        let mut inactive_due_ids = Vec::new();
        for id in &due_ids {
            let Some(tracker) = self
                .trackers
                .trackers()
                .iter()
                .find(|tracker| tracker.id == *id)
            else {
                continue;
            };
            match tracker.scope.decide_context(active_context) {
                ScopeDecision::Allowed => active_due_ids.push(id.clone()),
                ScopeDecision::Denied { .. } => inactive_due_ids.push(id.clone()),
            }
        }

        let mut updated = Vec::new();
        if !inactive_due_ids.is_empty() {
            let inactive = self.mark_inactive(
                now_ms,
                &inactive_due_ids,
                TrackerInactiveReason::FocusInactive,
            );
            updated.extend(inactive.updated);
        }
        if active_due_ids.is_empty() {
            return PollOutcome {
                due_trackers: due_ids.len(),
                screen_samples: 0,
                updated,
                sample: None,
            };
        }

        let sample = match sample_provider.capture_screen_sample() {
            Ok(sample) => sample,
            Err(_) => {
                let inactive = self.mark_inactive(
                    now_ms,
                    &active_due_ids,
                    TrackerInactiveReason::NoReadableSample,
                );
                updated.extend(inactive.updated);
                return PollOutcome {
                    due_trackers: due_ids.len(),
                    screen_samples: 0,
                    updated,
                    sample: None,
                };
            }
        };
        let sample_diagnostic = ScreenSampleDiagnostic::from(&sample);
        let coordinate_scale = sample_provider.coordinate_scale_for_sample(&sample, active_context);
        for id in &active_due_ids {
            let Some(tracker) = self
                .trackers
                .trackers()
                .iter()
                .find(|tracker| tracker.id == *id)
            else {
                continue;
            };
            if tracker
                .condition
                .as_ref()
                .is_some_and(|condition| !condition.matches(&self.latest))
            {
                self.latest.insert(
                    id.clone(),
                    TrackerState::Inactive {
                        reason: TrackerInactiveReason::ConditionInactive,
                        confidence: 0,
                        freshness_ms: 0,
                    },
                );
                self.last_poll_ms.insert(id.clone(), now_ms);
                updated.push(id.clone());
                continue;
            }
            let detector = tracker.detector.scaled_for_sample(coordinate_scale);
            let state = match &detector {
                DetectorDefinition::RadialCooldown { .. } => {
                    let history = self.cooldown_history.entry(id.clone()).or_default();
                    detect_radial_cooldown_with_roi(&sample, Some(&detector), history)
                }
                DetectorDefinition::HorizontalProgressBar { .. } => {
                    detect_horizontal_progress_bar_with_roi(&sample, Some(&detector))
                }
            };
            self.latest.insert(id.clone(), state);
            self.last_poll_ms.insert(id.clone(), now_ms);
            updated.push(id.clone());
        }
        PollOutcome {
            due_trackers: due_ids.len(),
            screen_samples: 1,
            updated,
            sample: Some(sample_diagnostic),
        }
    }

    fn mark_inactive(
        &mut self,
        now_ms: u64,
        due_ids: &[String],
        reason: TrackerInactiveReason,
    ) -> PollOutcome {
        for id in due_ids {
            self.latest.insert(
                id.clone(),
                TrackerState::Inactive {
                    reason,
                    confidence: 0,
                    freshness_ms: 0,
                },
            );
            self.last_poll_ms.insert(id.clone(), now_ms);
        }
        PollOutcome {
            due_trackers: due_ids.len(),
            screen_samples: 0,
            updated: due_ids.to_vec(),
            sample: None,
        }
    }
}

pub fn screen_read_denied_report(message: &str) -> CapabilityReport {
    CapabilityReport::from_statuses([CapabilityStatus::unavailable(
        CapabilityKind::ScreenRead,
        CapabilityAvailability::Denied,
        AdapterDiagnostic::new(ErrorPhase::CapabilityProbe, message)
            .with_capability(CapabilityKind::ScreenRead),
    )])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{available_capability_report, ProcessName};

    fn radial_detector() -> DetectorDefinition {
        DetectorDefinition::RadialCooldown {
            roi: Roi::new(2850, 2030, 96, 92).unwrap(),
            mask: Some(CircularMask::new(10)),
            phases: RadialCooldownPhases::refutation_default(),
        }
    }

    fn radial_detector_with_roi(roi: Roi) -> DetectorDefinition {
        DetectorDefinition::RadialCooldown {
            roi,
            mask: Some(CircularMask::new(0)),
            phases: RadialCooldownPhases::new(
                [
                    RadialPhaseRule {
                        phase: RadialCooldownPhase::Ready,
                        sample: RadialSampleRegion::AggregateMask,
                        min_luminance_percent: Some(40),
                        max_luminance_percent: None,
                        min_saturation: None,
                        max_saturation: None,
                        metric: RadialRuleMetric::Average,
                        metric_scale: None,
                        progress_fill: RadialProgressFill::Full,
                        max_fill_until_ready: None,
                        fill: None,
                        background: None,
                        opacity: None,
                    },
                    RadialPhaseRule {
                        phase: RadialCooldownPhase::Activated,
                        sample: RadialSampleRegion::AggregateMask,
                        min_luminance_percent: None,
                        max_luminance_percent: Some(10),
                        min_saturation: None,
                        max_saturation: None,
                        metric: RadialRuleMetric::Average,
                        metric_scale: None,
                        progress_fill: RadialProgressFill::Empty,
                        max_fill_until_ready: None,
                        fill: None,
                        background: None,
                        opacity: None,
                    },
                    RadialPhaseRule {
                        phase: RadialCooldownPhase::Active,
                        sample: RadialSampleRegion::AggregateMask,
                        min_luminance_percent: None,
                        max_luminance_percent: Some(39),
                        min_saturation: None,
                        max_saturation: None,
                        metric: RadialRuleMetric::Average,
                        metric_scale: None,
                        progress_fill: RadialProgressFill::Empty,
                        max_fill_until_ready: None,
                        fill: None,
                        background: None,
                        opacity: None,
                    },
                ],
                RadialCooldownPhase::Unknown,
            )
            .unwrap(),
        }
    }

    fn progress_detector() -> DetectorDefinition {
        DetectorDefinition::HorizontalProgressBar {
            roi: Roi::new(1828, 702, 190, 58).unwrap(),
            fill_direction: ProgressFillDirection::LeftToRight,
        }
    }

    fn progress_detector_with_roi(roi: Roi) -> DetectorDefinition {
        DetectorDefinition::HorizontalProgressBar {
            roi,
            fill_direction: ProgressFillDirection::LeftToRight,
        }
    }

    fn single_pixel_radial_detector() -> DetectorDefinition {
        DetectorDefinition::RadialCooldown {
            roi: Roi::new(0, 0, 1, 1).unwrap(),
            mask: None,
            phases: RadialCooldownPhases::refutation_default(),
        }
    }

    fn configured_refutation_detector() -> DetectorDefinition {
        DetectorDefinition::RadialCooldown {
            roi: Roi::new(0, 0, 36, 36).unwrap(),
            mask: None,
            phases: RadialCooldownPhases::refutation_default(),
        }
    }

    #[test]
    fn screen_sample_returns_pixel_color_for_supported_formats() {
        let sample = ScreenSample::from_pixels(
            2,
            2,
            8,
            ScreenPixelFormat::Bgra8888,
            0,
            vec![
                0, 0, 0, 255, 10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 128,
            ],
        );

        assert_eq!(
            sample.pixel_color(1, 1),
            Some(ScreenPixelColor::rgba(90, 80, 70, 128))
        );
        assert_eq!(sample.pixel_color(3, 1), None);
    }

    fn tracker(id: &str, detector: DetectorDefinition) -> StateTrackerDefinition {
        StateTrackerDefinition::new(
            id,
            ScopeSelection::process_list(vec![ProcessName::parse("steam_app_2694490").unwrap()])
                .unwrap(),
            CapabilitySet::new([CapabilityKind::ScreenRead]),
            50,
            detector,
        )
        .unwrap()
    }

    fn luma_frame(captured_at_ms: u64, width: u32, height: u32, value: u8) -> ScreenSample {
        ScreenSample::from_pixels(
            width,
            height,
            width,
            ScreenPixelFormat::Luma8,
            captured_at_ms,
            vec![value; width as usize * height as usize],
        )
    }

    #[test]
    fn radial_cooldown_estimates_total_and_remaining_time() {
        let mut history = RadialCooldownHistory::default();
        let samples = [
            ScreenSample::synthetic_percent(0, 80),
            ScreenSample::synthetic_percent(500, 60),
            ScreenSample::synthetic_percent(1000, 40),
            ScreenSample::synthetic_percent(1500, 20),
        ];

        let state = samples
            .iter()
            .map(|sample| detect_radial_cooldown(sample, &mut history))
            .last()
            .unwrap();

        assert_eq!(
            state,
            TrackerState::RadialCooldown {
                phase: RadialCooldownPhase::Active,
                ready: false,
                cooldown_fraction: 20,
                remaining_ms: Some(500),
                total_estimated_ms: Some(2500),
                predicted_remaining_ms: None,
                predicted_duration_ms: None,
                confidence: 90,
                freshness_ms: 0,
            }
        );
    }

    #[test]
    fn radial_cooldown_predicts_stable_active_duration_until_overstepped() {
        let mut phases = RadialCooldownPhases::refutation_default();
        phases = phases
            .with_prediction(RadialCooldownPrediction::new(8_000, 1_000).unwrap())
            .unwrap();
        let detector = DetectorDefinition::RadialCooldown {
            roi: Roi::new(0, 0, 36, 36).unwrap(),
            mask: None,
            phases,
        };
        let mut history = RadialCooldownHistory::default();

        let early = detect_radial_cooldown_with_roi(
            &active_refutation_sample_at(500),
            Some(&detector),
            &mut history,
        );
        assert!(matches!(
            early,
            TrackerState::RadialCooldown {
                phase: RadialCooldownPhase::Active,
                predicted_remaining_ms: None,
                ..
            }
        ));

        let stable = detect_radial_cooldown_with_roi(
            &active_refutation_sample_at(1_500),
            Some(&detector),
            &mut history,
        );
        assert!(matches!(
            stable,
            TrackerState::RadialCooldown {
                phase: RadialCooldownPhase::Active,
                predicted_remaining_ms: Some(7_000),
                predicted_duration_ms: Some(8_000),
                ..
            }
        ));

        let overstepped = detect_radial_cooldown_with_roi(
            &active_refutation_sample_at(9_000),
            Some(&detector),
            &mut history,
        );
        assert!(matches!(
            overstepped,
            TrackerState::RadialCooldown {
                phase: RadialCooldownPhase::Active,
                predicted_remaining_ms: None,
                predicted_duration_ms: None,
                ..
            }
        ));
    }

    fn active_refutation_sample_at(captured_at_ms: u64) -> ScreenSample {
        let mut sample = refutation_phase_sample(None, Some((65, 45, 35)), false);
        sample.captured_at_ms = captured_at_ms;
        sample
    }

    #[test]
    fn radial_cooldown_reports_ready_with_zero_remaining() {
        let mut history = RadialCooldownHistory::default();
        detect_radial_cooldown(&ScreenSample::synthetic_percent(0, 20), &mut history);
        let state = detect_radial_cooldown(&ScreenSample::synthetic_percent(500, 0), &mut history);

        assert!(matches!(
            state,
            TrackerState::RadialCooldown {
                phase: RadialCooldownPhase::Ready,
                ready: true,
                remaining_ms: Some(0),
                ..
            }
        ));
    }

    #[test]
    fn radial_cooldown_reports_ready_for_bright_refutation_icon_roi() {
        let detector = radial_detector_with_roi(Roi::new(0, 0, 8, 8).unwrap());
        let sample = luma_frame(0, 8, 8, 46);
        let mut history = RadialCooldownHistory::default();

        let state = detect_radial_cooldown_with_roi(&sample, Some(&detector), &mut history);

        assert_eq!(
            state,
            TrackerState::RadialCooldown {
                phase: RadialCooldownPhase::Ready,
                ready: true,
                cooldown_fraction: 0,
                remaining_ms: Some(0),
                total_estimated_ms: None,
                predicted_remaining_ms: None,
                predicted_duration_ms: None,
                confidence: 95,
                freshness_ms: 0,
            }
        );
    }

    #[test]
    fn radial_cooldown_normalizes_brightening_overlay_to_remaining_fraction() {
        let samples = [
            luma_frame(0, 8, 8, 11),
            luma_frame(500, 8, 8, 21),
            luma_frame(1000, 8, 8, 27),
        ];
        let mut history = RadialCooldownHistory::default();
        let states = samples
            .iter()
            .map(|sample| detect_radial_cooldown(sample, &mut history))
            .collect::<Vec<_>>();

        assert!(matches!(
            states[0],
            TrackerState::RadialCooldown {
                phase: RadialCooldownPhase::Active,
                ready: false,
                cooldown_fraction: 96,
                remaining_ms: None,
                ..
            }
        ));
        assert!(matches!(
            states[1],
            TrackerState::RadialCooldown {
                phase: RadialCooldownPhase::Active,
                ready: false,
                cooldown_fraction: 63,
                remaining_ms: Some(_),
                ..
            }
        ));
        assert!(matches!(
            states[2],
            TrackerState::RadialCooldown {
                phase: RadialCooldownPhase::Active,
                ready: false,
                cooldown_fraction: 43,
                remaining_ms: Some(_),
                ..
            }
        ));
    }

    #[test]
    fn radial_cooldown_phase_rules_classify_probe_gated_states() {
        let detector = configured_refutation_detector();

        let cases = [
            (
                refutation_phase_sample(Some((40, 130, 220)), Some((0, 0, 0)), false),
                RadialCooldownPhase::Ready,
                0,
            ),
            (
                refutation_phase_sample(None, Some((0, 0, 0)), false),
                RadialCooldownPhase::Activated,
                100,
            ),
            (
                refutation_phase_sample(None, Some((65, 45, 35)), false),
                RadialCooldownPhase::Active,
                100,
            ),
            (
                refutation_phase_sample(None, Some((140, 120, 100)), true),
                RadialCooldownPhase::Recovering,
                5,
            ),
            (
                refutation_phase_sample(None, Some((140, 120, 100)), false),
                RadialCooldownPhase::Unknown,
                100,
            ),
        ];

        for (index, (sample, expected_phase, expected_fraction)) in cases.into_iter().enumerate() {
            let mut history = RadialCooldownHistory::default();
            let state = detect_radial_cooldown_with_roi(&sample, Some(&detector), &mut history);
            assert!(
                matches!(
                    state,
                    TrackerState::RadialCooldown {
                        phase,
                        cooldown_fraction,
                        ..
                    } if phase == expected_phase && cooldown_fraction == expected_fraction
                ),
                "case {index} produced {state:?}"
            );
        }
    }

    #[test]
    fn radial_cooldown_active_probe_wins_over_bright_recovery_annulus() {
        let detector = configured_refutation_detector();
        let sample = refutation_phase_sample(None, Some((65, 45, 35)), true);
        let mut history = RadialCooldownHistory::default();

        let state = detect_radial_cooldown_with_roi(&sample, Some(&detector), &mut history);

        assert!(matches!(
            state,
            TrackerState::RadialCooldown {
                phase: RadialCooldownPhase::Active,
                cooldown_fraction: 100,
                ..
            }
        ));
    }

    #[test]
    fn radial_cooldown_recovery_metric_scale_normalizes_underreported_annulus() {
        let phases = RadialCooldownPhases::refutation_default();
        let recovering = phases
            .order
            .iter()
            .find(|rule| rule.phase == RadialCooldownPhase::Recovering)
            .unwrap();

        assert_eq!(
            cooldown_fraction_for_rule(
                recovering,
                RadialPixelStats {
                    luminance_percent: 0,
                    saturation: 0,
                    bright_ratio_percent: 33,
                },
            ),
            50
        );
        assert_eq!(
            cooldown_fraction_for_rule(
                recovering,
                RadialPixelStats {
                    luminance_percent: 0,
                    saturation: 0,
                    bright_ratio_percent: 66,
                },
            ),
            5
        );
    }

    fn refutation_phase_sample(
        before_probe: Option<(u8, u8, u8)>,
        after_probe: Option<(u8, u8, u8)>,
        bright_annulus: bool,
    ) -> ScreenSample {
        let roi = Roi::new(0, 0, 36, 36).unwrap();
        let mut pixels = vec![0u8; 36 * 36 * 3];
        if bright_annulus {
            paint_region(
                &mut pixels,
                &roi,
                RadialSampleRegion::AnnulusArc {
                    inner_radius_px: 13,
                    outer_radius_px: 17,
                    start_deg: 20.0,
                    end_deg: 340.0,
                },
                (230, 110, 20),
            );
        }
        if let Some(color) = before_probe {
            paint_region(
                &mut pixels,
                &roi,
                RadialSampleRegion::ClockProbe {
                    angle_deg: 352.0,
                    radius_px: 15,
                    w: 3,
                    h: 3,
                },
                color,
            );
        }
        if let Some(color) = after_probe {
            paint_region(
                &mut pixels,
                &roi,
                RadialSampleRegion::ClockProbe {
                    angle_deg: 8.0,
                    radius_px: 15,
                    w: 3,
                    h: 3,
                },
                color,
            );
        }
        ScreenSample::from_pixels(36, 36, 36 * 3, ScreenPixelFormat::Rgb888, 0, pixels)
    }

    fn paint_region(pixels: &mut [u8], roi: &Roi, region: RadialSampleRegion, color: (u8, u8, u8)) {
        for y in roi.y..roi.y + roi.h {
            for x in roi.x..roi.x + roi.w {
                if !pixel_in_sample_region(&region, roi, x, y) {
                    continue;
                }
                let offset = (y as usize * roi.w as usize + x as usize) * 3;
                pixels[offset] = color.0;
                pixels[offset + 1] = color.1;
                pixels[offset + 2] = color.2;
            }
        }
    }

    #[test]
    fn progress_bar_reports_visible_progress() {
        let state = detect_horizontal_progress_bar(&ScreenSample::synthetic_percent(0, 73));

        assert_eq!(
            state,
            TrackerState::HorizontalProgressBar {
                visible: true,
                progress_percent: 73,
                confidence: 90,
                freshness_ms: 0,
            }
        );
    }

    #[test]
    fn progress_bar_does_not_invent_progress_without_sample() {
        let state = detect_horizontal_progress_bar(&ScreenSample::new(0, Vec::<u8>::new()));

        assert_eq!(
            state,
            TrackerState::HorizontalProgressBar {
                visible: false,
                progress_percent: 0,
                confidence: 0,
                freshness_ms: 0,
            }
        );
    }

    #[test]
    fn tracker_set_rejects_duplicate_ids_and_missing_screen_read() {
        let duplicate = StateTrackerDefinitionSet::new([
            tracker("refutation", radial_detector()),
            tracker("refutation", progress_detector()),
        ])
        .unwrap_err();
        assert!(duplicate.message.contains("duplicate state tracker"));

        let missing = StateTrackerDefinition::new(
            "bad",
            ScopeSelection::ExplicitGlobal,
            CapabilitySet::default(),
            50,
            progress_detector(),
        )
        .unwrap_err();
        assert!(missing.message.contains("screen_read"));
    }

    struct CountingProvider {
        captures: usize,
        sample: ScreenSample,
    }

    impl ScreenSampleProvider for CountingProvider {
        fn capture_screen_sample(&mut self) -> Result<ScreenSample, DiagnosableError> {
            self.captures += 1;
            Ok(self.sample.clone())
        }
    }

    struct ScalingProvider {
        captures: usize,
        sample: ScreenSample,
        scale: ScreenCoordinateScale,
    }

    impl ScreenSampleProvider for ScalingProvider {
        fn capture_screen_sample(&mut self) -> Result<ScreenSample, DiagnosableError> {
            self.captures += 1;
            Ok(self.sample.clone())
        }

        fn coordinate_scale_for_sample(
            &mut self,
            _sample: &ScreenSample,
            _active_context: &ActiveProcessContext,
        ) -> ScreenCoordinateScale {
            self.scale
        }
    }

    #[test]
    fn poller_reports_time_until_next_due_tracker() {
        let set = StateTrackerDefinitionSet::new([
            tracker("refutation", radial_detector()),
            tracker("heavy_stun", progress_detector()),
        ])
        .unwrap();
        let required = set.required_capabilities().clone();
        let report = available_capability_report(&required, "test");
        let active_context =
            ActiveProcessContext::name_only(ProcessName::parse("steam_app_2694490").unwrap());
        let mut provider = CountingProvider {
            captures: 0,
            sample: ScreenSample::synthetic_percent(0, 50),
        };
        let mut poller = StateTrackerPoller::new(set);

        assert_eq!(poller.next_due_in_ms(0), Some(0));

        poller.poll_due(0, &report, &active_context, &mut provider);

        assert_eq!(poller.next_due_in_ms(20), Some(30));
        assert_eq!(poller.next_due_in_ms(50), Some(0));
    }

    #[test]
    fn poller_batches_due_trackers_against_one_screen_sample() {
        let set = StateTrackerDefinitionSet::new([
            tracker("refutation", radial_detector()),
            tracker("heavy_stun", progress_detector()),
        ])
        .unwrap();
        let required = set.required_capabilities().clone();
        let report = available_capability_report(&required, "test");
        let mut provider = CountingProvider {
            captures: 0,
            sample: ScreenSample::synthetic_percent(0, 50),
        };
        let mut poller = StateTrackerPoller::new(set);
        let active_context =
            ActiveProcessContext::name_only(ProcessName::parse("steam_app_2694490").unwrap());

        let outcome = poller.poll_due(0, &report, &active_context, &mut provider);

        assert_eq!(outcome.due_trackers, 2);
        assert_eq!(outcome.screen_samples, 1);
        assert_eq!(
            outcome.sample,
            Some(ScreenSampleDiagnostic {
                width: 1,
                height: 1,
                stride: 1,
                pixel_format: ScreenPixelFormat::Luma8,
            })
        );
        assert_eq!(provider.captures, 1);
    }

    #[test]
    fn poller_tracks_conditioned_progress_only_when_radial_source_phase_matches() {
        let heavy = tracker(
            "heavy_stun",
            progress_detector_with_roi(Roi::new(0, 0, 1, 1).unwrap()),
        )
        .only_when(
            StateTrackerCondition::radial_phase("refutation_cooldown", RadialCooldownPhase::Active)
                .unwrap(),
        );
        let set = StateTrackerDefinitionSet::new([
            tracker("refutation_cooldown", single_pixel_radial_detector()),
            heavy,
        ])
        .unwrap();
        let required = set.required_capabilities().clone();
        let report = available_capability_report(&required, "test");
        let active_context =
            ActiveProcessContext::name_only(ProcessName::parse("steam_app_2694490").unwrap());
        let mut poller = StateTrackerPoller::new(set);

        let mut ready_provider = CountingProvider {
            captures: 0,
            sample: ScreenSample::synthetic_percent(0, 0),
        };
        poller.poll_due(0, &report, &active_context, &mut ready_provider);
        assert!(matches!(
            poller.latest_state("refutation_cooldown"),
            Some(TrackerState::RadialCooldown {
                phase: RadialCooldownPhase::Ready,
                ..
            })
        ));
        assert!(matches!(
            poller.latest_state("heavy_stun"),
            Some(TrackerState::Inactive {
                reason: TrackerInactiveReason::ConditionInactive,
                ..
            })
        ));

        let mut active_provider = CountingProvider {
            captures: 0,
            sample: ScreenSample::synthetic_percent(50, 50),
        };
        poller.poll_due(50, &report, &active_context, &mut active_provider);
        assert!(matches!(
            poller.latest_state("refutation_cooldown"),
            Some(TrackerState::RadialCooldown {
                phase: RadialCooldownPhase::Active,
                ..
            })
        ));
        assert!(matches!(
            poller.latest_state("heavy_stun"),
            Some(TrackerState::HorizontalProgressBar {
                visible: true,
                progress_percent: 50,
                ..
            })
        ));
    }

    #[test]
    fn detector_crops_configured_roi_from_rgb_frame() {
        let detector = progress_detector_with_roi(Roi::new(1, 0, 2, 1).unwrap());
        let set = StateTrackerDefinitionSet::new([tracker("heavy_stun", detector)]).unwrap();
        let required = set.required_capabilities().clone();
        let report = available_capability_report(&required, "test");
        let mut provider = CountingProvider {
            captures: 0,
            sample: ScreenSample::from_pixels(
                4,
                1,
                12,
                ScreenPixelFormat::Rgb888,
                0,
                [
                    0, 0, 0, // outside ROI
                    255, 255, 255, // ROI
                    255, 255, 255, // ROI
                    0, 0, 0, // outside ROI
                ],
            ),
        };
        let mut poller = StateTrackerPoller::new(set);
        let active_context =
            ActiveProcessContext::name_only(ProcessName::parse("steam_app_2694490").unwrap());

        poller.poll_due(0, &report, &active_context, &mut provider);

        assert!(matches!(
            poller.latest_state("heavy_stun"),
            Some(TrackerState::HorizontalProgressBar {
                visible: true,
                progress_percent: 100,
                confidence: 95,
                ..
            })
        ));
    }

    #[test]
    fn poller_scales_logical_detector_roi_to_physical_screen_sample() {
        let detector = progress_detector_with_roi(Roi::new(2, 2, 2, 1).unwrap());
        let set = StateTrackerDefinitionSet::new([tracker("heavy_stun", detector)]).unwrap();
        let required = set.required_capabilities().clone();
        let report = available_capability_report(&required, "test");
        let mut pixels = vec![0u8; 6 * 6 * 3];
        for y in 3..5 {
            for x in 3..6 {
                let offset = y * 18 + x * 3;
                pixels[offset..offset + 3].copy_from_slice(&[255, 255, 255]);
            }
        }
        let mut provider = ScalingProvider {
            captures: 0,
            sample: ScreenSample::from_pixels(6, 6, 18, ScreenPixelFormat::Rgb888, 0, pixels),
            scale: ScreenCoordinateScale::new(1.5, 1.5).unwrap(),
        };
        let mut poller = StateTrackerPoller::new(set);
        let active_context =
            ActiveProcessContext::name_only(ProcessName::parse("steam_app_2694490").unwrap());

        poller.poll_due(0, &report, &active_context, &mut provider);

        assert!(matches!(
            poller.latest_state("heavy_stun"),
            Some(TrackerState::HorizontalProgressBar {
                visible: true,
                progress_percent: 100,
                confidence: 95,
                ..
            })
        ));
    }

    #[test]
    fn detector_fails_closed_when_roi_exceeds_frame_bounds() {
        let detector = progress_detector_with_roi(Roi::new(2, 0, 3, 1).unwrap());
        let set = StateTrackerDefinitionSet::new([tracker("heavy_stun", detector)]).unwrap();
        let required = set.required_capabilities().clone();
        let report = available_capability_report(&required, "test");
        let mut provider = CountingProvider {
            captures: 0,
            sample: ScreenSample::from_pixels(4, 1, 12, ScreenPixelFormat::Rgb888, 0, [255u8; 12]),
        };
        let mut poller = StateTrackerPoller::new(set);
        let active_context =
            ActiveProcessContext::name_only(ProcessName::parse("steam_app_2694490").unwrap());

        poller.poll_due(0, &report, &active_context, &mut provider);

        assert!(matches!(
            poller.latest_state("heavy_stun"),
            Some(TrackerState::HorizontalProgressBar {
                visible: false,
                progress_percent: 0,
                confidence: 0,
                ..
            })
        ));
    }

    #[test]
    fn poller_fails_closed_without_screen_read_or_active_scope() {
        let set =
            StateTrackerDefinitionSet::new([tracker("heavy_stun", progress_detector())]).unwrap();
        let mut provider = CountingProvider {
            captures: 0,
            sample: ScreenSample::synthetic_percent(0, 50),
        };
        let denied = screen_read_denied_report("screen read denied");
        let mut poller = StateTrackerPoller::new(set.clone());
        let active_context =
            ActiveProcessContext::name_only(ProcessName::parse("steam_app_2694490").unwrap());

        let denied_outcome = poller.poll_due(0, &denied, &active_context, &mut provider);
        assert_eq!(denied_outcome.screen_samples, 0);
        assert_eq!(provider.captures, 0);
        assert!(matches!(
            poller.latest_state("heavy_stun"),
            Some(TrackerState::Inactive {
                reason: TrackerInactiveReason::ScreenReadDenied,
                ..
            })
        ));

        let required = set.required_capabilities().clone();
        let available = available_capability_report(&required, "test");
        let non_matching_context =
            ActiveProcessContext::name_only(ProcessName::parse("konsole").unwrap());
        let inactive_outcome =
            poller.poll_due(50, &available, &non_matching_context, &mut provider);
        assert_eq!(inactive_outcome.screen_samples, 0);
        assert_eq!(provider.captures, 0);
        assert!(matches!(
            poller.latest_state("heavy_stun"),
            Some(TrackerState::Inactive {
                reason: TrackerInactiveReason::FocusInactive,
                ..
            })
        ));
    }
}
