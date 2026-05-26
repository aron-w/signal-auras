use crate::{AdapterDiagnostic, DiagnosableError, ErrorPhase};
use std::time::{Duration, Instant};

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
    Denied { reason: String },
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
            confidence: ActiveProcessConfidence::Ambiguous,
            captured_at: Instant::now(),
            diagnostic: Some(AdapterDiagnostic::new(ErrorPhase::Trigger, reason.into())),
        }
    }

    pub fn is_stale(&self, max_age: Duration) -> bool {
        self.captured_at.elapsed() > max_age
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
                Some(active) => ScopeDecision::Denied {
                    reason: format!(
                        "active process '{}' is outside configured scope",
                        active.as_str()
                    ),
                },
                None => ScopeDecision::Denied {
                    reason: "active process is unavailable".to_string(),
                },
            },
        }
    }

    pub fn decide_context(&self, active_context: &ActiveProcessContext) -> ScopeDecision {
        if matches!(self, Self::ExplicitGlobal) {
            return ScopeDecision::Allowed;
        }
        if active_context.is_stale(Duration::from_secs(2)) {
            return ScopeDecision::Denied {
                reason: "active process metadata is stale".to_string(),
            };
        }
        match active_context.confidence {
            ActiveProcessConfidence::Ambiguous => ScopeDecision::Denied {
                reason: "active process metadata is ambiguous".to_string(),
            },
            ActiveProcessConfidence::Unavailable => ScopeDecision::Denied {
                reason: "active process metadata is unavailable".to_string(),
            },
            ActiveProcessConfidence::Denied => ScopeDecision::Denied {
                reason: "active process metadata permission was denied".to_string(),
            },
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
