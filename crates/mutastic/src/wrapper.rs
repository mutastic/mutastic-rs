//! mutastic as Cargo's `RUSTC_WORKSPACE_WRAPPER`: Cargo runs `mutastic <rustc> <args>` for the
//! copy's own crates only, so a flag added here never reaches a dependency, and the project's own
//! rustflags still apply.

use std::ffi::OsString;
use std::process::{Command, ExitCode};

/// Set in the environment of the builds mutastic runs, so the binary knows it's the wrapper.
pub const MARKER: &str = "MUTASTIC_RUSTC_WRAPPER";

/// Whether this process is the wrapper.
pub fn active(args: &[OsString]) -> bool {
    std::env::var_os(MARKER).is_some() && args.len() >= 2
}

/// Runs rustc with the copy's flags: lints capped, so a `deny(warnings)` in the project doesn't
/// fail a build whose rewritten code it was never meant to judge.
pub fn run(args: &[OsString]) -> ExitCode {
    let mut command = Command::new(&args[1]);
    command.args(&args[2..]).env_remove(MARKER);
    if args.iter().any(|arg| arg == "--crate-name") {
        command.arg("--cap-lints=allow");
    }
    match command.status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1).clamp(0, 255) as u8),
        Err(error) => {
            stderr_line!(
                "mutastic: couldn't run {}: {error}",
                args[1].to_string_lossy()
            );
            ExitCode::FAILURE
        }
    }
}
