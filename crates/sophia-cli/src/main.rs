mod commands;

use sophia_runtime::{TraceLevel, init_tracing};

fn session_stdout(line: &str) {
    println!("{line}");
}

fn session_stderr(line: &str) {
    eprintln!("{line}");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let verbose = args.iter().any(|arg| arg == "-v" || arg == "--verbose");
    let level = if verbose {
        TraceLevel::Debug
    } else {
        TraceLevel::Info
    };

    init_tracing(level)?;
    sophia_session::install_session_output(sophia_session::SessionOutput::new(
        session_stdout,
        session_stderr,
    ))
    .map_err(std::io::Error::other)?;
    commands::run(&args, verbose)
}
