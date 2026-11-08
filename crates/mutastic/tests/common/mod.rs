//! What the integration tests share.

use serde_json::Value;

/// `mutastic.events/v1`, pinned from the Java repository (`docs/public/schemas/events/v1.json` at
/// 208fd4d). It asks `jdk`, a mutation's `bytecode`, and a missed mutant's `ignore` and `propose`
/// only of the Java edition's events, and every Rust event says `"edition": "rust"`, so it's taken
/// as it is.
pub fn schema() -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(include_str!("../schema/events-v1.json")).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}
