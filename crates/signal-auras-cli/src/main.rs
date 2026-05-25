use signal_auras_cli::runner::{run_cli, StdioPrompt};

fn main() {
    let code = match run_cli(std::env::args().skip(1), &mut StdioPrompt) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("error {error}");
            1
        }
    };
    std::process::exit(code);
}
