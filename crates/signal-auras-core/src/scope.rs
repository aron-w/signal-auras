use crate::{AdapterDiagnostic, DiagnosableError, ErrorPhase};
use std::time::{Duration, Instant};

pub const DEFAULT_FOCUS_STALE_THRESHOLD: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessName(String);

impl ProcessName {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, DiagnosableError> {
        let value = value.as_ref().trim();
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "process name must be a non-empty printable string",
            ));
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptScope {
    Processes(Vec<ProcessName>),
}

impl ScriptScope {
    pub fn processes(values: Vec<ProcessName>) -> Result<Self, DiagnosableError> {
        if values.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScriptValidation,
                "scope.processes must contain at least one process",
            ));
        }
        Ok(Self::Processes(values))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeSelection {
    ProcessList { processes: Vec<ProcessName> },
    ExplicitGlobal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeDecision {
    Allowed,
    Denied {
        reason: String,
        diagnostic: ScopeDenial,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusFreshnessPolicy {
    pub stale_threshold: Duration,
}

impl Default for FocusFreshnessPolicy {
    fn default() -> Self {
        Self {
            stale_threshold: DEFAULT_FOCUS_STALE_THRESHOLD,
        }
    }
}

impl FocusFreshnessPolicy {
    pub fn new(stale_threshold: Duration) -> Self {
        Self { stale_threshold }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusFreshness {
    Fresh { age: Duration },
    Stale { age: Duration, threshold: Duration },
    UntrustedTimestamp { threshold: Duration },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeDenialKind {
    StaleFocus,
    FocusUnavailable,
    FocusPermissionDenied,
    AmbiguousFocus,
    UntrustedFocusTimestamp,
    ProcessMismatch,
}

impl ScopeDenialKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StaleFocus => "stale_focus",
            Self::FocusUnavailable => "focus_unavailable",
            Self::FocusPermissionDenied => "focus_permission_denied",
            Self::AmbiguousFocus => "ambiguous_focus",
            Self::UntrustedFocusTimestamp => "untrusted_focus_timestamp",
            Self::ProcessMismatch => "process_mismatch",
        }
    }

    pub fn counts_as_metadata_unavailable(self) -> bool {
        matches!(
            self,
            Self::StaleFocus
                | Self::FocusUnavailable
                | Self::FocusPermissionDenied
                | Self::AmbiguousFocus
                | Self::UntrustedFocusTimestamp
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeDenial {
    pub kind: ScopeDenialKind,
    pub rule: String,
    pub metadata_age: Option<Duration>,
    pub stale_threshold: Option<Duration>,
}

impl ScopeDenial {
    fn new(kind: ScopeDenialKind, rule: String) -> Self {
        Self {
            kind,
            rule,
            metadata_age: None,
            stale_threshold: None,
        }
    }

    fn with_freshness(mut self, age: Option<Duration>, threshold: Duration) -> Self {
        self.metadata_age = age;
        self.stale_threshold = Some(threshold);
        self
    }

    pub fn counts_as_metadata_unavailable(&self) -> bool {
        self.kind.counts_as_metadata_unavailable()
    }

    pub fn render_fields(&self) -> String {
        let mut fields = format!(
            "denial_reason={} configured_rule={}",
            self.kind.as_str(),
            sanitize_diagnostic_value(&self.rule)
        );
        if let Some(age) = self.metadata_age {
            fields.push_str(&format!(" metadata_age_ms={}", age.as_millis()));
        }
        if let Some(threshold) = self.stale_threshold {
            fields.push_str(&format!(" stale_threshold_ms={}", threshold.as_millis()));
        }
        fields
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveProcessConfidence {
    Exact,
    NameOnly,
    Ambiguous,
    Unavailable,
    Denied,
}

#[derive(Debug, Clone)]
pub struct ActiveProcessContext {
    pub visible_name: Option<ProcessName>,
    pub process_id: Option<u32>,
    pub app_id: Option<String>,
    pub window_class: Option<String>,
    pub confidence: ActiveProcessConfidence,
    pub captured_at: Instant,
    pub diagnostic: Option<AdapterDiagnostic>,
}

impl ActiveProcessContext {
    pub fn exact(visible_name: ProcessName, process_id: Option<u32>) -> Self {
        Self {
            visible_name: Some(visible_name),
            process_id,
            app_id: None,
            window_class: None,
            confidence: ActiveProcessConfidence::Exact,
            captured_at: Instant::now(),
            diagnostic: None,
        }
    }

    pub fn name_only(visible_name: ProcessName) -> Self {
        Self {
            visible_name: Some(visible_name),
            process_id: None,
            app_id: None,
            window_class: None,
            confidence: ActiveProcessConfidence::NameOnly,
            captured_at: Instant::now(),
            diagnostic: None,
        }
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            visible_name: None,
            process_id: None,
            app_id: None,
            window_class: None,
            confidence: ActiveProcessConfidence::Unavailable,
            captured_at: Instant::now(),
            diagnostic: Some(AdapterDiagnostic::new(
                ErrorPhase::CapabilityProbe,
                reason.into(),
            )),
        }
    }

    pub fn denied(reason: impl Into<String>) -> Self {
        Self {
            visible_name: None,
            process_id: None,
            app_id: None,
            window_class: None,
            confidence: ActiveProcessConfidence::Denied,
            captured_at: Instant::now(),
            diagnostic: Some(AdapterDiagnostic::new(
                ErrorPhase::CapabilityProbe,
                reason.into(),
            )),
        }
    }

    pub fn ambiguous(reason: impl Into<String>) -> Self {
        Self {
            visible_name: None,
            process_id: None,
            app_id: None,
            window_class: None,
            confidence: ActiveProcessConfidence::Ambiguous,
            captured_at: Instant::now(),
            diagnostic: Some(AdapterDiagnostic::new(ErrorPhase::Trigger, reason.into())),
        }
    }

    pub fn is_stale(&self, max_age: Duration) -> bool {
        matches!(
            self.freshness_at(Instant::now(), FocusFreshnessPolicy::new(max_age)),
            FocusFreshness::Stale { .. }
        )
    }

    pub fn freshness_at(&self, now: Instant, policy: FocusFreshnessPolicy) -> FocusFreshness {
        let Some(age) = now.checked_duration_since(self.captured_at) else {
            return FocusFreshness::UntrustedTimestamp {
                threshold: policy.stale_threshold,
            };
        };
        if age > policy.stale_threshold {
            FocusFreshness::Stale {
                age,
                threshold: policy.stale_threshold,
            }
        } else {
            FocusFreshness::Fresh { age }
        }
    }

    pub fn with_app_id(mut self, app_id: impl Into<String>) -> Self {
        self.app_id = Some(app_id.into());
        self
    }

    pub fn with_window_class(mut self, window_class: impl Into<String>) -> Self {
        self.window_class = Some(window_class.into());
        self
    }

    pub fn matchable_name(&self) -> Option<&ProcessName> {
        match self.confidence {
            ActiveProcessConfidence::Exact | ActiveProcessConfidence::NameOnly => {
                self.visible_name.as_ref()
            }
            ActiveProcessConfidence::Ambiguous
            | ActiveProcessConfidence::Unavailable
            | ActiveProcessConfidence::Denied => None,
        }
    }
}

impl ScopeSelection {
    pub fn from_script(scope: ScriptScope) -> Self {
        match scope {
            ScriptScope::Processes(processes) => Self::ProcessList { processes },
        }
    }

    pub fn process_list(processes: Vec<ProcessName>) -> Result<Self, DiagnosableError> {
        if processes.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScopePrompt,
                "process scope requires at least one process",
            ));
        }
        Ok(Self::ProcessList { processes })
    }

    pub fn explicit_global_from_prompt(confirmed: bool) -> Result<Self, DiagnosableError> {
        if confirmed {
            Ok(Self::ExplicitGlobal)
        } else {
            Err(DiagnosableError::new(
                ErrorPhase::ScopePrompt,
                "global scope requires explicit confirmation",
            ))
        }
    }

    pub fn decide(&self, active_process: Option<&ProcessName>) -> ScopeDecision {
        match self {
            Self::ExplicitGlobal => ScopeDecision::Allowed,
            Self::ProcessList { processes } => match active_process {
                Some(active) if processes.iter().any(|allowed| allowed == active) => {
                    ScopeDecision::Allowed
                }
                Some(active) => self.denied(
                    ScopeDenialKind::ProcessMismatch,
                    format!(
                        "active process '{}' is outside configured scope",
                        active.as_str()
                    ),
                ),
                None => self.denied(
                    ScopeDenialKind::FocusUnavailable,
                    "active process is unavailable".to_string(),
                ),
            },
        }
    }

    pub fn decide_context(&self, active_context: &ActiveProcessContext) -> ScopeDecision {
        self.decide_context_with_policy(active_context, FocusFreshnessPolicy::default())
    }

    pub fn decide_context_at(
        &self,
        active_context: &ActiveProcessContext,
        now: Instant,
    ) -> ScopeDecision {
        self.decide_context_at_with_policy(active_context, now, FocusFreshnessPolicy::default())
    }

    pub fn decide_context_with_policy(
        &self,
        active_context: &ActiveProcessContext,
        policy: FocusFreshnessPolicy,
    ) -> ScopeDecision {
        self.decide_context_at_with_policy(active_context, Instant::now(), policy)
    }

    pub fn decide_context_at_with_policy(
        &self,
        active_context: &ActiveProcessContext,
        now: Instant,
        policy: FocusFreshnessPolicy,
    ) -> ScopeDecision {
        if matches!(self, Self::ExplicitGlobal) {
            return ScopeDecision::Allowed;
        }
        match active_context.freshness_at(now, policy) {
            FocusFreshness::Fresh { .. } => {}
            FocusFreshness::Stale { age, threshold } => {
                return self.denied_with_freshness(
                    ScopeDenialKind::StaleFocus,
                    "active process metadata is stale".to_string(),
                    Some(age),
                    threshold,
                );
            }
            FocusFreshness::UntrustedTimestamp { threshold } => {
                return self.denied_with_freshness(
                    ScopeDenialKind::UntrustedFocusTimestamp,
                    "active process metadata timestamp is untrusted".to_string(),
                    None,
                    threshold,
                );
            }
        }
        match active_context.confidence {
            ActiveProcessConfidence::Ambiguous => self.denied(
                ScopeDenialKind::AmbiguousFocus,
                "active process metadata is ambiguous".to_string(),
            ),
            ActiveProcessConfidence::Unavailable => self.denied(
                ScopeDenialKind::FocusUnavailable,
                "active process metadata is unavailable".to_string(),
            ),
            ActiveProcessConfidence::Denied => self.denied(
                ScopeDenialKind::FocusPermissionDenied,
                "active process metadata permission was denied".to_string(),
            ),
            ActiveProcessConfidence::Exact | ActiveProcessConfidence::NameOnly => {
                self.decide(active_context.matchable_name())
            }
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Self::ExplicitGlobal => "global (explicit current run)".to_string(),
            Self::ProcessList { processes } => format!(
                "processes: {}",
                processes
                    .iter()
                    .map(ProcessName::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    fn denied(&self, kind: ScopeDenialKind, reason: String) -> ScopeDecision {
        ScopeDecision::Denied {
            reason,
            diagnostic: ScopeDenial::new(kind, self.diagnostic_rule()),
        }
    }

    fn denied_with_freshness(
        &self,
        kind: ScopeDenialKind,
        reason: String,
        age: Option<Duration>,
        threshold: Duration,
    ) -> ScopeDecision {
        ScopeDecision::Denied {
            reason,
            diagnostic: ScopeDenial::new(kind, self.diagnostic_rule())
                .with_freshness(age, threshold),
        }
    }

    fn diagnostic_rule(&self) -> String {
        match self {
            Self::ExplicitGlobal => "global".to_string(),
            Self::ProcessList { processes } => format!(
                "processes:{}",
                processes
                    .iter()
                    .map(ProcessName::as_str)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

fn sanitize_diagnostic_value(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_graphic() { ch } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_scope_allows_matching_process() {
        let scope =
            ScopeSelection::process_list(vec![ProcessName::parse("poe2.exe").unwrap()]).unwrap();
        assert_eq!(
            scope.decide(Some(&ProcessName::parse("poe2.exe").unwrap())),
            ScopeDecision::Allowed
        );
    }

    #[test]
    fn process_scope_denies_unknown_process() {
        let scope =
            ScopeSelection::process_list(vec![ProcessName::parse("poe2.exe").unwrap()]).unwrap();
        assert!(matches!(scope.decide(None), ScopeDecision::Denied { .. }));
    }
}
