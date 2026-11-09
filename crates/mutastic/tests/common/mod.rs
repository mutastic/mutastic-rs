//! What the integration tests share.

use serde_json::Value;

/// `mutastic.events/v1`, pinned from the Java repository (`docs/public/schemas/events/v1.json` at
/// 211a43b). It asks `jdk`, a mutation's `bytecode`, and a missed mutant's `ignore` and `propose`
/// only of the Java edition's events, and every Rust event says `"edition": "rust"`, so it's taken
/// as it is.
pub fn schema() -> jsonschema::Validator {
    jsonschema::validator_for(&pinned()).unwrap()
}

/// What the schema says a mutation's `applied` is. The schema leaves objects open, so an event
/// with a field it doesn't name still passes [`schema`]: this checks `applied` is one it names, and
/// as it describes it.
#[allow(dead_code)]
pub fn applied_schema() -> jsonschema::Validator {
    let applied = pinned()["$defs"]["mutantFinished"]["properties"]["mutation"]["properties"]
        ["applied"]
        .clone();
    assert!(applied.is_object(), "the pinned schema names no `applied`");
    jsonschema::validator_for(&applied).unwrap()
}

fn pinned() -> Value {
    serde_json::from_str(include_str!("../schema/events-v1.json")).unwrap()
}
