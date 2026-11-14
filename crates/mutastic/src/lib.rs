//! mutastic for Rust: mutation testing that compiles every mutant into one build, switches them
//! at run time, and says, for each change no test notices, what test would.
//!
//! The run, in order: [`sites`] finds the mutants with rust-analyzer's parser; [`schema`] writes
//! them all into a copy of the source, each behind a switch; [`compile`] builds that copy once,
//! withdrawing mutants that don't compile; [`engine`] records what each test reaches and runs
//! each mutant against those tests; [`report`] prints the result.

/// A line to stderr that can't panic. Progress and notes go to stderr, and stderr can close mid-run
/// (`mutastic run … 2>&1 | head`): `eprintln!` would panic there, in a worker thread, and take its
/// results with it. What can't be shown is dropped; the run goes on.
macro_rules! stderr_line {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

/// A line to stdout that can't panic: `mutastic run | head` closes stdout early, and `println!`
/// would panic there and exit 101. What can't be shown is dropped, and the run ends as it would.
macro_rules! stdout_line {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stdout(), $($arg)*);
    }};
}

/// [`stdout_line`] without the line's end.
macro_rules! stdout_text {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let mut stdout = std::io::stdout();
        let _ = write!(stdout, $($arg)*);
        let _ = stdout.flush();
    }};
}

pub mod baseline;
pub mod callrules;
pub mod cfg;
pub mod cli;
pub mod commands;
pub mod compile;
pub mod config;
pub mod edition;
pub mod engine;
pub mod failure;
pub mod flaky;
pub mod gate;
pub mod guard;
pub mod id;
pub mod ignores;
pub mod java;
pub mod locate;
pub mod mistyped;
pub mod nextrun;
pub mod operator_docs;
pub mod progress;
pub mod record;
pub mod records;
pub mod report;
pub mod schema;
pub mod shares;
pub mod similar;
pub mod since;
pub mod sites;
pub mod suggest;
pub mod testrun;
pub mod words;
pub mod workspace;
pub mod wrapper;

pub use mutastic_terminal as terminal;
