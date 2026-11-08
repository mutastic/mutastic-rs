//! What the integration tests share.

use serde_json::Value;

/// `mutastic.events/v1`, pinned from the Java repository (`docs/public/schemas/events/v1.json` at
/// 327fcc5). Three differences, each until the Rust edition or the schema catches up: `run-finished`
/// requires `jdk`, the Java release the tests ran on, which a Rust run doesn't have (it reports
/// `rustc`); a missed mutant requires `ignore` and `propose`, commands the Rust edition doesn't
/// have yet, and output never offers a command that doesn't exist; and the statuses lack
/// `RuntimeError`, the report schema's status for a crash that proves nothing, which only the Rust
/// edition writes so far.
pub fn schema() -> jsonschema::Validator {
    let mut schema: Value = serde_json::from_str(include_str!("../schema/events-v1.json")).unwrap();
    schema["$defs"]["status"]["enum"]
        .as_array_mut()
        .unwrap()
        .push("RuntimeError".into());
    let required = schema["$defs"]["runFinished"]["required"]
        .as_array_mut()
        .unwrap();
    required.retain(|field| field != "jdk");
    let missed = schema["$defs"]["mutantFinished"]["allOf"][0]["then"]["required"]
        .as_array_mut()
        .unwrap();
    missed.retain(|field| field != "ignore" && field != "propose");
    jsonschema::validator_for(&schema).unwrap()
}
