//! The output kit: what a tool in the family prints with, whatever it tests. Colour roles, glyphs
//! and labels ([`paint`], [`glyphs`]); text from the project escaped and fenced for an agent
//! ([`untrusted`]); the one name close to a mistyped one ([`similar`]); help's layout
//! ([`layout`]); a live progress line's bar, width and heartbeat ([`progress`]); and an event
//! stream's envelope ([`envelope`]).
//!
//! It knows nothing of mutastic: no mutants, cards or scores, and no dependency on the `mutastic`
//! crate, which re-exports what it shares from here. `mutastic`'s `tests/layers.rs` holds it to
//! that. It moves to a library of the family's own when a sibling tool first uses it.

pub mod envelope;
pub mod glyphs;
pub mod layout;
pub mod paint;
pub mod progress;
pub mod similar;
pub mod untrusted;
