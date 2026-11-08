//! `mutastic run` on the samples, through the binary as a person runs it.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

fn fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
        .display()
        .to_string()
}

fn mutastic(args: &[&str]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args(args)
        .env_remove("MUTASTIC_FORMAT")
        .env_remove("MUTASTIC_AGENT")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.code() != Some(2),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.code().unwrap_or(-1), stdout)
}

/// `mutastic.events/v1`, pinned from the Java repository (`docs/public/schemas/events/v1.json` at
/// 9fb7433). Two differences, each until the Rust edition or the schema catches up: `run-finished`
/// requires `jdk`, the Java release the tests ran on, which a Rust run doesn't have (it reports
/// `rustc`); and a missed mutant requires `ignore` and `propose`, commands the Rust edition
/// doesn't have yet, and output never offers a command that doesn't exist.
fn schema() -> jsonschema::Validator {
    let mut schema: Value = serde_json::from_str(include_str!("schema/events-v1.json")).unwrap();
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

fn events(stdout: &str) -> Vec<Value> {
    stdout
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// The pricing card, as the Java edition's: the edge of `total >= 100` is the one gap, and the
/// inputs that show it come from running the tests.
#[test]
fn pricing_misses_only_the_edge_of_the_discount() {
    let dir = fixture("pricing");
    let (code, stdout) = mutastic(&["run", &dir, "--format=jsonl"]);
    assert_eq!(code, 0, "{stdout}");
    let events = events(&stdout);
    let mutants: Vec<&Value> = events
        .iter()
        .filter(|e| e["reason"] == "mutant-finished")
        .collect();
    assert_eq!(mutants.len(), 11, "the Java sample has 11 too");
    let missed: Vec<&&Value> = mutants
        .iter()
        .filter(|e| e["status"] == "Survived")
        .collect();
    assert_eq!(missed.len(), 1, "{stdout}");
    let missed = missed[0];
    assert_eq!(missed["operator"], "mutastic/boundary");
    assert_eq!(missed["location"]["line"], 26);
    assert_eq!(missed["inputs"]["total"], serde_json::json!([50, 250]));
    assert_eq!(
        missed["coveredBy"],
        serde_json::json!([
            "pricing::big_orders_get_ten_percent_off",
            "tests::small_orders_pay_full_price"
        ])
    );
    for event in &events {
        let errors: Vec<String> = schema().iter_errors(event).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{event}\n{}", errors.join("\n"));
    }
    let finished = events.last().unwrap();
    assert_eq!(finished["reason"], "run-finished");
    assert_eq!(finished["score"], 90.9);
    assert_eq!(finished["Killed"], 10);

    // The card, and the command it offers.
    let id = missed["id"].as_str().unwrap();
    let (_, card) = mutastic(&["run", &dir, "--format=full", &format!("--only={id}")]);
    let expected = format!(
        "  ✗ [MISSED]  no test checks discount() at total == 100
    ┌─ src/lib.rs:26
    │
 25 │     pub fn discount(&self, total: u32) -> u32 {{
 26 │         if total >= 100 {{
    │                  ^^ changed to >
 27 │             return total / 10;
    │
    = mutant  - if total >= 100 {{
              + if total > 100 {{
    = note: every test calls discount() away from the boundary
            inputs seen: 50, 250 (none is 100)
            covered by pricing::big_orders_get_ten_percent_off, tests::small_orders_pay_full_price (all 2 covering tests still pass)
    = help: test discount() with total = 100 and assert the result

    done when  mutastic verify {id} {dir} passes
    reproduce  mutastic run --only={id} {dir}
"
    );
    assert!(card.contains(&expected), "{card}");

    // verify fails while the gap is open.
    let (code, stdout) = mutastic(&["verify", id, &dir]);
    assert_eq!(code, 1);
    assert_eq!(stdout, format!("mutastic: {id} is still missed\n"));
}
