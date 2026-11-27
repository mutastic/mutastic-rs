//! The lines a change touched, from git, for `--since`, as `mutastic-core`'s
//! [`since`](mutastic_core::since) reads them for every edition: here, in Rust's source files.

pub use mutastic_core::since::*;

use std::path::Path;

use mutastic_core::edition::Edition as _;

use crate::failure::Failure;
use crate::rust_edition::Rust;

/// The Rust lines changed in the repository holding `dir` since `since`, or since its default
/// branch for `""`.
pub fn changed_since(dir: &Path, since: &str) -> Result<ChangedLines, Failure> {
    changed_since_in(dir, since, Rust::SOURCES)
}
