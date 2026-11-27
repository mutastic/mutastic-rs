//! The lines a change touched, from git, for `--since`, as `mutastic-core`'s
//! [`since`](mutastic_core::since) reads them for every edition: here, in Rust's source files.

pub use mutastic_core::since::*;

use std::path::Path;

use crate::failure::Failure;

/// The Rust edition's source files, as git's pathspecs.
pub const SOURCES: &[&str] = &["*.rs"];

/// The Rust lines changed in the repository holding `dir` since `since`, or since its default
/// branch for `""`.
pub fn changed_since(dir: &Path, since: &str) -> Result<ChangedLines, Failure> {
    changed_since_in(dir, since, SOURCES)
}
