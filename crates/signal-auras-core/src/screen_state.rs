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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectorDefinition {
    RadialCooldown {
        roi: Roi,
        mask: Option<CircularMask>,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateTrackerDefinition {
    pub id: String,
    pub scope: ScopeSelection,
    pub capabilities: CapabilitySet,
    pub poll_ms: u64,
    pub detector: DetectorDefinition,
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
        })
    }

    pub fn required_capabilities(&self) -> CapabilitySet {
        let mut required = self.capabilities.iter().collect::<Vec<_>>();
        if matches!(self.scope, ScopeSelection::ProcessList { .. }) {
            required.push(CapabilityKind::ActiveProcessMetadata);
        }
        CapabilitySet::new(required)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
        ready: bool,
        cooldown_fraction: u8,
        remaining_ms: Option<u64>,
        total_estimated_ms: Option<u64>,
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
                ready,
                cooldown_fraction,
                remaining_ms,
                confidence,
                ..
            } => format!(
                "radial_cooldown ready={ready} fraction={cooldown_fraction} remaining_ms={remaining_ms:?} confidence={confidence}"
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
    NoReadableSample,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RadialCooldownHistory {
    observations: Vec<CooldownObservation>,
    last_total_estimate_ms: Option<u64>,
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
    let Some(fraction) = observed_radial_cooldown_fraction(sample, detector) else {
        return TrackerState::Inactive {
            reason: TrackerInactiveReason::NoReadableSample,
            confidence: 0,
            freshness_ms: 0,
        };
    };
    let fraction = fraction.min(100);
    history.push(sample.captured_at_ms, fraction);
    let total_estimated_ms = history.estimate_total_ms();
    let ready = fraction <= 2;
    let remaining_ms = if ready {
        Some(0)
    } else {
        total_estimated_ms.map(|total| total.saturating_mul(u64::from(fraction)) / 100)
    };
    TrackerState::RadialCooldown {
        ready,
        cooldown_fraction: if ready { 0 } else { fraction },
        remaining_ms,
        total_estimated_ms,
        confidence: confidence_for_sample(sample),
        freshness_ms: 0,
    }
}

fn observed_radial_cooldown_fraction(
    sample: &ScreenSample,
    detector: Option<&DetectorDefinition>,
) -> Option<u8> {
    let observed = observed_percent(sample, detector)?;
    if sample.pixel_format == ScreenPixelFormat::Luma8 && sample.pixels.len() == 1 {
        return Some(observed);
    }
    if observed >= RADIAL_READY_LUMINANCE_PERCENT {
        return Some(0);
    }
    if observed <= RADIAL_FULL_COOLDOWN_LUMINANCE_PERCENT {
        return Some(100);
    }
    let cooldown_range = RADIAL_READY_LUMINANCE_PERCENT - RADIAL_FULL_COOLDOWN_LUMINANCE_PERCENT;
    let dark_delta = RADIAL_READY_LUMINANCE_PERCENT - observed;
    Some(((u64::from(dark_delta) * 100) / u64::from(cooldown_range)).min(100) as u8)
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
    let mut count = 0u64;
    for y in roi.y..y_end {
        for x in roi.x..x_end {
            if !pixel_in_detector_mask(detector, roi, x, y) {
                continue;
            }
            let offset = y as usize * stride + x as usize * bytes_per_pixel;
            let pixel = sample.pixels.get(offset..offset + bytes_per_pixel)?;
            sum = sum.saturating_add(pixel_percent(pixel, sample.pixel_format)?);
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    Some((sum / count).min(100) as u8)
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

fn pixel_percent(pixel: &[u8], format: ScreenPixelFormat) -> Option<u64> {
    let luminance = match format {
        ScreenPixelFormat::Luma8 => u64::from(*pixel.first()?),
        ScreenPixelFormat::Rgb888 | ScreenPixelFormat::Rgba8888 | ScreenPixelFormat::Rgbx8888 => {
            rgb_luminance(pixel[0], pixel[1], pixel[2])
        }
        ScreenPixelFormat::Bgr888 | ScreenPixelFormat::Bgra8888 | ScreenPixelFormat::Bgrx8888 => {
            rgb_luminance(pixel[2], pixel[1], pixel[0])
        }
    };
    if format == ScreenPixelFormat::Luma8 {
        Some(luminance.min(100))
    } else {
        Some((luminance * 100 / 255).min(100))
    }
}

fn rgb_luminance(r: u8, g: u8, b: u8) -> u64 {
    (u64::from(r) * 299 + u64::from(g) * 587 + u64::from(b) * 114) / 1000
}

pub trait ScreenSampleProvider {
    fn capture_screen_sample(&mut self) -> Result<ScreenSample, DiagnosableError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollOutcome {
    pub due_trackers: usize,
    pub screen_samples: usize,
    pub updated: Vec<String>,
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
                };
            }
        };
        for id in &active_due_ids {
            let Some(tracker) = self
                .trackers
                .trackers()
                .iter()
                .find(|tracker| tracker.id == *id)
            else {
                continue;
            };
            let state = match tracker.detector {
                DetectorDefinition::RadialCooldown { .. } => {
                    let history = self.cooldown_history.entry(id.clone()).or_default();
                    detect_radial_cooldown_with_roi(&sample, Some(&tracker.detector), history)
                }
                DetectorDefinition::HorizontalProgressBar { .. } => {
                    detect_horizontal_progress_bar_with_roi(&sample, Some(&tracker.detector))
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
        }
    }

    fn radial_detector_with_roi(roi: Roi) -> DetectorDefinition {
        DetectorDefinition::RadialCooldown {
            roi,
            mask: Some(CircularMask::new(0)),
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
                ready: false,
                cooldown_fraction: 20,
                remaining_ms: Some(500),
                total_estimated_ms: Some(2500),
                confidence: 90,
                freshness_ms: 0,
            }
        );
    }

    #[test]
    fn radial_cooldown_reports_ready_with_zero_remaining() {
        let mut history = RadialCooldownHistory::default();
        detect_radial_cooldown(&ScreenSample::synthetic_percent(0, 20), &mut history);
        let state = detect_radial_cooldown(&ScreenSample::synthetic_percent(500, 0), &mut history);

        assert!(matches!(
            state,
            TrackerState::RadialCooldown {
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
                ready: true,
                cooldown_fraction: 0,
                remaining_ms: Some(0),
                total_estimated_ms: None,
                confidence: 95,
                freshness_ms: 0,
            }
        );
    }

    #[test]
    fn radial_cooldown_normalizes_brightening_overlay_to_remaining_fraction() {
        let detector = radial_detector_with_roi(Roi::new(0, 0, 8, 8).unwrap());
        let samples = [
            luma_frame(0, 8, 8, 11),
            luma_frame(500, 8, 8, 21),
            luma_frame(1000, 8, 8, 27),
        ];
        let mut history = RadialCooldownHistory::default();
        let states = samples
            .iter()
            .map(|sample| detect_radial_cooldown_with_roi(sample, Some(&detector), &mut history))
            .collect::<Vec<_>>();

        assert!(matches!(
            states[0],
            TrackerState::RadialCooldown {
                ready: false,
                cooldown_fraction: 96,
                remaining_ms: None,
                ..
            }
        ));
        assert!(matches!(
            states[1],
            TrackerState::RadialCooldown {
                ready: false,
                cooldown_fraction: 63,
                remaining_ms: Some(_),
                ..
            }
        ));
        assert!(matches!(
            states[2],
            TrackerState::RadialCooldown {
                ready: false,
                cooldown_fraction: 43,
                remaining_ms: Some(_),
                ..
            }
        ));
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
        assert_eq!(provider.captures, 1);
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
