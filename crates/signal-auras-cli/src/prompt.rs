use signal_auras_core::{ConsentDecision, DiagnosableError, ErrorPhase, ProcessName};
use std::io::{self, IsTerminal, Write};

pub trait ScopePrompt {
    fn resolve_missing_scope(&mut self) -> Result<ConsentDecision, DiagnosableError>;
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
