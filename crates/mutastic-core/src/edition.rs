//! What an edition gives the core: its own words, where the core's shared behaviour needs a
//! language's. The Rust edition's are `mutastic`'s `rust_edition::Rust`.

/// An edition's own words, as the core's modules take them.
pub trait Edition {
    /// Its name, as `--edition` takes it and as its mutants' ids are made with: `rust`.
    const NAME: &'static str;
    /// Where a bug in it is reported: `https://github.com/mutastic/mutastic-rs/issues`.
    const ISSUES: &'static str;
    /// The tools a command an error ends in runs, each as a command line starts with it:
    /// `mutastic `, `git `, and its build tool's.
    const TOOLS: &'static [&'static str];
    /// Its source files, as git's pathspecs: what `--since` reads the changed lines of.
    const SOURCES: &'static [&'static str];
}
