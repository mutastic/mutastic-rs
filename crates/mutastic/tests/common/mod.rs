//! What the integration tests share.

use serde_json::Value;

/// `mutastic.events/v1`, pinned from the Java repository (`docs/public/schemas/events/v1.json` at
/// b0d888c, with `mutant-explained` and the optional `docs` the mutant events gained). It asks `jdk`
/// only of the Java edition's events, and every Rust event says `"edition": "rust"`, so it's taken
/// as it is.
pub fn schema() -> jsonschema::Validator {
    jsonschema::validator_for(&pinned()).unwrap()
}

/// What the schema says a mutation's `applied` is. The schema leaves objects open, so an event
/// with a field it doesn't name still passes [`schema`]: this checks `applied` is one it names, and
/// as it describes it.
#[allow(
    dead_code,
    reason = "only some test crates check `applied`, and an `expect` would fail in the others"
)]
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
