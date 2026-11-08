//! mutastic for Rust: mutation testing that compiles every mutant into one build, switches them
//! at run time, and says, for each change no test notices, what test would.
//!
//! The run, in order: [`sites`] finds the mutants with rust-analyzer's parser; [`schema`] writes
//! them all into a copy of the source, each behind a switch; [`compile`] builds that copy once,
//! withdrawing mutants that don't compile; [`engine`] records what each test reaches and runs
//! each mutant against those tests; [`report`] prints the result.

pub mod cli;
pub mod compile;
pub mod engine;
pub mod id;
pub mod report;
pub mod schema;
pub mod sites;
pub mod testrun;
pub mod words;
pub mod workspace;
pub mod wrapper;
