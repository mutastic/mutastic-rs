//! An event stream's envelope, as `mutastic.events/v1` has it: each event an object on a line of
//! its own, which starts with the stream's `schema`, the `edition` that wrote it and the event's
//! `reason`, then the event's own fields in order. A reader keys on `reason`, and checks `schema`
//! once.

use serde_json::{Map, Value};

/// The event `reason`, with `fields`, an object, after the envelope's. A field of the same name as
/// one of the envelope's takes its place.
pub fn event(schema: &str, edition: &str, reason: &str, fields: Value) -> Value {
    let mut event = Map::new();
    event.insert("schema".to_string(), schema.into());
    event.insert("edition".to_string(), edition.into());
    event.insert("reason".to_string(), reason.into());
    if let Value::Object(fields) = fields {
        event.extend(fields);
    }
    Value::Object(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_event_starts_with_its_schema_edition_and_reason() {
        let event = event(
            "tool.events/v1",
            "rust",
            "progress",
            serde_json::json!({"phase": "coverage", "done": 3}),
        );
        assert_eq!(
            event.to_string(),
            r#"{"schema":"tool.events/v1","edition":"rust","reason":"progress","phase":"coverage","done":3}"#
        );
    }
}
