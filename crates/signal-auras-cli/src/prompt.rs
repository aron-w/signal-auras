use signal_auras_core::{ConsentDecision, DiagnosableError, ErrorPhase, ProcessName};
use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
};

pub trait ScopePrompt {
    fn resolve_missing_scope(&mut self) -> Result<ConsentDecision, DiagnosableError>;

    fn select_input_devices(
        &mut self,
        _reason: &str,
        _candidates: &[DevicePromptCandidate],
    ) -> Result<DeviceSelectionDecision, DiagnosableError> {
        Ok(DeviceSelectionDecision::NonInteractive)
    }

    fn confirm_input_permission_repair(
        &mut self,
        _paths: &[PathBuf],
        _uinput: bool,
    ) -> Result<bool, DiagnosableError> {
        Ok(false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevicePromptCandidate {
    pub path: PathBuf,
    pub label: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceSelectionDecision {
    Selected(Vec<PathBuf>),
    Cancel,
    NonInteractive,
}

pub struct TerminalPrompt<R, W> {
    reader: R,
    writer: W,
    interactive: bool,
}

impl<R, W> TerminalPrompt<R, W> {
    pub fn new(reader: R, writer: W, interactive: bool) -> Self {
        Self {
            reader,
            writer,
            interactive,
        }
    }
}

impl<R: io::BufRead, W: Write> ScopePrompt for TerminalPrompt<R, W> {
    fn resolve_missing_scope(&mut self) -> Result<ConsentDecision, DiagnosableError> {
        if !self.interactive {
            return Ok(ConsentDecision::NonInteractiveMissingScope);
        }

        writeln!(
            self.writer,
            "No scope declared by script.\nSelect scope for this run:\n1. Process names\n2. Global hotkeys for this run\n3. Cancel"
        )
        .map_err(prompt_io_error)?;
        let choice = read_line(&mut self.reader)?;
        match choice.trim() {
            "1" => {
                writeln!(self.writer, "Process names, comma separated:")
                    .map_err(prompt_io_error)?;
                let names = read_line(&mut self.reader)?;
                let processes = names
                    .split(',')
                    .map(ProcessName::parse)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ConsentDecision::ProcessScope(processes))
            }
            "2" => {
                writeln!(
                    self.writer,
                    "Type GLOBAL to confirm global hotkeys for this run:"
                )
                .map_err(prompt_io_error)?;
                let confirmation = read_line(&mut self.reader)?;
                if confirmation.trim() == "GLOBAL" {
                    Ok(ConsentDecision::ExplicitGlobalConfirmed)
                } else {
                    Ok(ConsentDecision::Cancel)
                }
            }
            "3" => Ok(ConsentDecision::Cancel),
            _ => Err(DiagnosableError::new(
                ErrorPhase::ScopePrompt,
                "invalid scope selection",
            )),
        }
    }

    fn select_input_devices(
        &mut self,
        reason: &str,
        candidates: &[DevicePromptCandidate],
    ) -> Result<DeviceSelectionDecision, DiagnosableError> {
        if !self.interactive {
            return Ok(DeviceSelectionDecision::NonInteractive);
        }
        if candidates.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScopePrompt,
                "interactive input selection found no eligible devices",
            ));
        }

        writeln!(
            self.writer,
            "Select input devices for this run ({reason}).\nEnter numbers separated by comma or space, or C to cancel:"
        )
        .map_err(prompt_io_error)?;
        for (index, candidate) in candidates.iter().enumerate() {
            let marker = if candidate.selected { "[x]" } else { "[ ]" };
            writeln!(
                self.writer,
                "{marker} {}. {} {}",
                index + 1,
                candidate.path.display(),
                candidate.label
            )
            .map_err(prompt_io_error)?;
        }

        let choice = read_line(&mut self.reader)?;
        let choice = choice.trim();
        if choice.eq_ignore_ascii_case("c") || choice.eq_ignore_ascii_case("cancel") {
            return Ok(DeviceSelectionDecision::Cancel);
        }
        let selected = choice
            .split(|ch: char| ch == ',' || ch.is_whitespace())
            .filter(|part| !part.is_empty())
            .map(|part| {
                part.parse::<usize>().map_err(|_| {
                    DiagnosableError::new(
                        ErrorPhase::ScopePrompt,
                        format!("invalid input device selection '{part}'"),
                    )
                })
            })
            .map(|result| {
                result.and_then(|index| {
                    candidates
                        .get(index.saturating_sub(1))
                        .map(|candidate| candidate.path.clone())
                        .ok_or_else(|| {
                            DiagnosableError::new(
                                ErrorPhase::ScopePrompt,
                                format!("input device selection index {index} is out of range"),
                            )
                        })
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if selected.is_empty() {
            return Err(DiagnosableError::new(
                ErrorPhase::ScopePrompt,
                "interactive input selection requires at least one device",
            ));
        }
        Ok(DeviceSelectionDecision::Selected(selected))
    }

    fn confirm_input_permission_repair(
        &mut self,
        paths: &[PathBuf],
        uinput: bool,
    ) -> Result<bool, DiagnosableError> {
        if !self.interactive {
            return Ok(false);
        }
        writeln!(
            self.writer,
            "Temporary input permissions are missing for the selected devices."
        )
        .map_err(prompt_io_error)?;
        for path in paths {
            writeln!(self.writer, "- {}", path.display()).map_err(prompt_io_error)?;
        }
        if uinput {
            writeln!(self.writer, "- /dev/uinput").map_err(prompt_io_error)?;
        }
        writeln!(
            self.writer,
            "Type GRANT to run a sudo ACL repair for these paths:"
        )
        .map_err(prompt_io_error)?;
        let confirmation = read_line(&mut self.reader)?;
        Ok(confirmation.trim() == "GRANT")
    }
}

fn read_line<R: io::BufRead>(reader: &mut R) -> Result<String, DiagnosableError> {
    let mut line = String::new();
    reader.read_line(&mut line).map_err(prompt_io_error)?;
    Ok(line)
}

fn prompt_io_error(error: io::Error) -> DiagnosableError {
    DiagnosableError::new(
        ErrorPhase::ScopePrompt,
        format!("prompt I/O failed: {error}"),
    )
}

pub fn stdin_is_interactive() -> bool {
    io::stdin().is_terminal()
}
