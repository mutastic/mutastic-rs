//! What every edition of mutastic shares and no language's: the errors and their codes
//! ([`failure`]), the command line written back to copy ([`command_line`]), mutant ids ([`id`]),
//! the project's decisions not to test a mutant ([`ignores`]), the lines a change touched
//! ([`since`]), a mutant's fate ([`status`]), what a run found kept for the next to compare with
//! ([`record`], [`records`]), the tests watched since a run named them likely flaky
//! ([`watch`]), what the last `next` left uncaught ([`nextrun`]), the misses a project has taken as its baseline ([`baseline`]), and amounts and
//! times as mutastic writes them ([`amounts`], [`slept`]).
//!
//! It knows no build tool, compiler or parser: where an edition's own words go in, such as the
//! source files' pathspecs for `--since` or the edition's name in an id, the edition gives them,
//! as an [`edition::Edition`] names them.
//! The Rust edition, `mutastic`, re-exports these modules, or wraps them with its own words; another
//! edition depends on this crate at a `vX.Y.Z` tag of mutastic-rs (ARCHITECTURE.md says how).

pub mod amounts;
pub mod baseline;
pub mod command_line;
pub mod edition;
pub mod failure;
pub mod id;
pub mod ignores;
pub mod nextrun;
pub mod record;
pub mod records;
pub mod since;
pub mod slept;
pub mod status;
pub mod watch;

use mutastic_output::similar;
