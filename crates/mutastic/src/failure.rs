//! Why mutastic couldn't run, as `mutastic-core`'s [`failure`](mutastic_core::failure) has it for
//! every edition, with the Rust edition's own words: where to report a bug in it, and the tools a
//! command it offers runs.

pub use mutastic_core::failure::*;

/// Where a bug in the Rust edition is reported.
pub const ISSUES: &str = "https://github.com/mutastic/mutastic-rs/issues";

/// The tools a command an error ends in runs: mutastic, Cargo or git.
pub const TOOLS: &[&str] = &["mutastic ", "cargo ", "git "];

/// The failure a panic is: a bug in mutastic, with where it panicked and where to report it.
pub fn internal(panic: &std::panic::PanicHookInfo<'_>) -> Failure {
    internal_at(panic, ISSUES)
}

/// The commands `message` ends in: each line of it set in by two spaces that runs mutastic, Cargo
/// or git.
pub fn help(message: &str) -> Vec<String> {
    help_among(message, TOOLS)
}
