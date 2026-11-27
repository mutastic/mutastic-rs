//! What every edition of mutastic shares and no language's: the errors and their codes
//! ([`failure`]), the command line written back to copy ([`command_line`]), mutant ids ([`id`]),
//! the project's decisions not to test a mutant ([`ignores`]), the lines a change touched
//! ([`since`]), and amounts and times as mutastic writes them ([`amounts`], [`slept`]).
//!
//! It knows no build tool, compiler or parser: where an edition's own words go in, such as the
//! source files' pathspecs for `--since` or the edition's name in an id, the edition gives them.
//! The Rust edition, `mutastic`, re-exports these modules, or wraps them with its own words; another
//! edition depends on this crate at a `vX.Y.Z` tag of mutastic-rs (ARCHITECTURE.md says how).

pub mod amounts;
pub mod command_line;
pub mod failure;
pub mod id;
pub mod ignores;
pub mod since;
pub mod slept;

use mutastic_output::similar;
