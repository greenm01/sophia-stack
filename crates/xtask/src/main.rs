//! Development tooling that must not be assembled from strings in a shell.
//!
//! Two seams live here. Session arguments were built in bash and consumed by
//! `PersistentXtermSessionConfig::from_args`; evidence records are emitted by
//! Rust and were parsed by `grep` in the verifiers. Both were untyped, and
//! three physical runs died in the first because nothing asked whether a
//! vector was acceptable until the display manager was already down.
//!
//! Shell keeps `sudo sv`, `chvt`, traps, and process waits. That code has been
//! reliable; the string-building has not.

mod profile;
mod tests;
mod verify;

fn main() -> std::process::ExitCode {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    match run(&arguments) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run(arguments: &[String]) -> Result<(), String> {
    match arguments.first().map(String::as_str) {
        Some("session-args") => profile::resolve(&arguments[1..]).map(|vector| {
            for argument in &vector {
                println!("{argument}");
            }
        }),
        Some("check-profiles") => profile::check_every_profile(&arguments[1..]).map(|accepted| {
            for (name, arguments) in accepted {
                println!(
                    "sophia_xtask_profile schema=1 status=accepted profile={name} arguments={arguments}"
                );
            }
        }),
        Some("verify") => verify::run(&arguments[1..]).map(|report| {
            for line in report {
                println!("{line}");
            }
        }),
        Some("--help" | "-h") | None => {
            print!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command {other:?}\n\n{USAGE}")),
    }
}

const USAGE: &str = "\
usage: cargo xtask <command>

  session-args --profile=<name> [--display=<name>] [key=value ...]
      Print the live-session argument vector for a tool profile, one argument
      per line, after checking that the session would accept it.

  check-profiles
      Build and validate every profile's vector. This is the check none of the
      failed physical runs had.

  verify direct-scanout <log>...
      Verify direct-scanout evidence.

profiles: xmonad hagia native standalone kitty
";
