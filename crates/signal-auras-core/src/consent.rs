use crate::{DiagnosableError, ErrorPhase, ProcessName, ScopeSelection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsentDecision {
    ProcessScope(Vec<ProcessName>),
    ExplicitGlobalConfirmed,
    Cancel,
    NonInteractiveMissingScope,
}

impl ConsentDecision {
    pub fn into_scope(self) -> Result<Option<ScopeSelection>, DiagnosableError> {
        match self {
            Self::ProcessScope(processes) => ScopeSelection::process_list(processes).map(Some),
            Self::ExplicitGlobalConfirmed => {
                ScopeSelection::explicit_global_from_prompt(true).map(Some)
            }
            Self::Cancel => Ok(None),
            Self::NonInteractiveMissingScope => Err(DiagnosableError::new(
                ErrorPhase::ScopePrompt,
                "missing scope cannot be resolved without interactive stdin",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_has_no_scope() {
        assert_eq!(ConsentDecision::Cancel.into_scope().unwrap(), None);
    }

    #[test]
    fn non_interactive_missing_scope_errors() {
        assert!(ConsentDecision::NonInteractiveMissingScope
            .into_scope()
            .is_err());
    }
}
