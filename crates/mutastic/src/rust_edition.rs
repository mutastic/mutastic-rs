//! The Rust edition's own words, as the editions' core takes them.

use mutastic_core::edition::Edition;

/// The Rust edition.
pub struct Rust;

impl Edition for Rust {
    const NAME: &'static str = "rust";
    const ISSUES: &'static str = "https://github.com/mutastic/mutastic-rs/issues";
    const TOOLS: &'static [&'static str] = &["mutastic ", "cargo ", "git "];
    const SOURCES: &'static [&'static str] = &["*.rs"];
}
