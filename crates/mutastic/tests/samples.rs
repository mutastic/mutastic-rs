//! `mutastic run` on the samples, through the binary as a person runs it.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

mod common;
use common::schema;

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
    assert_eq!(
        finished["killed"], 10,
        "the name from 0.6.0, beside the old"
    );
    assert_eq!(events[0]["rule"], events[0]["rules"]);
    assert!(events.iter().all(|e| e["edition"] == "rust"), "{stdout}");
    assert_eq!(finished["toolchain"]["name"], "rustc");
    assert!(
        finished.get("rustc").is_none(),
        "the schema doesn't name it"
    );

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

/// Every shape the truth fixture holds comes out as a card, an operator on a line of its own
/// shown on that line.
#[test]
fn every_shape_gets_a_card() {
    let dir = fixture("truth");
    let (_, cards) = mutastic(&["run", &dir, "--format=full"]);
    assert!(cards.contains("✗ [MISSED]"), "{cards}");
    assert!(
        cards.contains("    = mutant  - >= 4\n              + > 4\n"),
        "{cards}"
    );
}

/// Unsafe code is left alone, code near it is marked and runs isolated, and a crash counts as caught
/// only where it can't be undefined behaviour, and then not as a test's catch. The fixture turns
/// overflow checks and debug assertions off for tests, and mutastic turns them back on.
#[test]
fn unsafe_code_is_fenced_off_and_crashes_are_told_apart() {
    let dir = fixture("unsafety");
    let (_, stdout) = mutastic(&["run", &dir, "--format=jsonl"]);
    let events = events(&stdout);
    for event in &events {
        let errors: Vec<String> = schema().iter_errors(event).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{event}\n{}", errors.join("\n"));
    }
    let started = &events[0];
    assert_eq!(started["profile"]["overflowChecks"], true, "{started}");
    assert_eq!(started["profile"]["debugAssertions"], true, "{started}");
    assert_eq!(started["unsafe"], "skip");
    let finished = events.last().unwrap();
    assert_eq!(finished["notMutated"]["unsafe"], 4, "halve() and first()");
    let fate = |method: &str, replacement: &str| -> &Value {
        events
            .iter()
            .find(|e| {
                e["location"]["method"] == method && e["mutation"]["replacement"] == replacement
            })
            .unwrap_or_else(|| panic!("{method} {replacement}: {stdout}"))
    };
    let checked = fate("checked", "true");
    assert_eq!(checked["status"], "RuntimeError", "{checked}");
    assert_eq!(checked["nearUnsafe"], "in a module with unsafe code");
    let positive = fate("positive", "false");
    assert_eq!(positive["status"], "RuntimeError", "{positive}");
    assert!(
        positive["statusReason"]
            .as_str()
            .unwrap()
            .contains("std's debug check of an unsafe precondition"),
        "{positive}"
    );
    let guarded = fate("guarded", "true");
    assert_eq!(guarded["status"], "Killed", "{guarded}");
    assert_eq!(guarded["killedWith"], "crash: SIGABRT");
    assert!(
        guarded.get("killedBy").is_none(),
        "no test caught it: {guarded}"
    );
    let total = fate("total", "-");
    assert_eq!(total["caughtBy"], total["killedBy"]);
    assert_eq!(total["caughtWith"], total["killedWith"]);
    assert_eq!(
        total["killedWith"], "panic: attempt to subtract with overflow",
        "{total}"
    );
    assert!(events
        .iter()
        .all(|e| e["location"]["method"] != "halve" && e["location"]["method"] != "helper"));

    // checked() is called, by run::checks, so only its line that aborts is out of reach.
    let (_, full) = mutastic(&["run", &dir, "--format=full"]);
    assert!(
        full.contains("checked() line 25 (1 mutant): no test reaches that line"),
        "{full}"
    );

    let id = guarded["id"].as_str().unwrap();
    let (code, stdout) = mutastic(&["verify", id, &dir]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("caught only because the test process crashed (SIGABRT)"),
        "{stdout}"
    );
}

/// `done when`'s outcome is dimmed like its label, as in the Java edition, so `passes` doesn't
/// read as part of the command to type.
#[test]
fn done_when_dims_its_outcome() {
    let dir = fixture("pricing");
    let output = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args(["run", &dir, "--format=full"])
        .env_remove("MUTASTIC_FORMAT")
        .env_remove("MUTASTIC_AGENT")
        .env_remove("NO_COLOR")
        .env("FORCE_COLOR", "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|line| line.contains("done when"))
        .unwrap_or_else(|| panic!("no done-when line in {stdout}"));
    assert!(
        line.ends_with(" \x1b[2mpasses\x1b[0m"),
        "passes isn't dimmed: {line:?}"
    );
}
