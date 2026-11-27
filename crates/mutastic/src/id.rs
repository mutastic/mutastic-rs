//! Mutant ids, as `mutastic-core`'s [`id`](mutastic_core::id) makes them for every edition, for
//! the Rust edition's mutants.

/// The id (`m` and 8 hex digits) and the fingerprint (16 hex digits) of the parts given.
pub fn of(parts: &[&str]) -> (String, String) {
    mutastic_core::id::of("rust", parts)
}
