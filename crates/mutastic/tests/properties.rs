//! Property tests, on `fixtures/properties`: proptest's and quickcheck's. A property draws its
//! inputs at random, so mutastic seeds the frameworks it can, runs a mutant's properties without
//! shrinking, names the input that caught each mutant, and says which frameworks it can't seed.

#![expect(clippy::disallowed_methods, reason = "runs mutastic as a user does")]
#![allow(
    clippy::unwrap_used,
    reason = "a helper here asserts as the tests it serves do: a value that isn't there fails the test"
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

mod common;

/// `is_adult`'s `>=` made `>`: only an age of 18 catches it.
const AT_18: &str = "mcd6c982a";
/// `reversed` pushing nothing: any list but the empty one catches it.
const PUSHES_NOTHING: &str = "m0ff90af7";

fn copy(label: &str) -> PathBuf {
    common::own_copy("properties", &format!("properties-{label}"))
}

/// mutastic's exit code, stdout and stderr, with proptest's seed as `seed` sets it.
fn mutastic(dir: &Path, args: &[&str], seed: Option<&str>) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mutastic"));
    command
        .args(args)
        .current_dir(dir)
        .env_remove("MUTASTIC_FORMAT")
        .env_remove("MUTASTIC_AGENT")
        .env_remove("PROPTEST_RNG_SEED")
        .env("MUTASTIC_PROGRESS", "none")
        .env("NO_COLOR", "1");
    if let Some(seed) = seed {
        command.env("PROPTEST_RNG_SEED", seed);
    }
    let output = command.output().unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn events(stdout: &str) -> Vec<Value> {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect()
}

fn finished(events: &[Value]) -> BTreeMap<String, Value> {
    events
        .iter()
        .filter(|event| event["reason"] == "mutant-finished")
        .map(|event| (event["id"].as_str().unwrap().to_string(), event.clone()))
        .collect()
}

/// Two runs are one run: every mutant's status is the same, and so is the input each proptest
/// catch names, the first case its seed draws that fails, unshrunk. quickcheck's inputs are its
/// own draw's, and its properties here catch whatever it draws. `run-started` says which
/// frameworks were seeded, with what, and the run ends with a note on the one that wasn't.
#[test]
fn a_run_with_the_same_seed_is_the_same_run() {
    let dir = copy("twice");
    let runs: Vec<(Vec<Value>, String)> = (0..2)
        .map(|_| {
            let (code, stdout, stderr) = mutastic(&dir, &["run", ".", "--format=jsonl"], None);
            assert_eq!(code, 0, "{stdout}{stderr}");
            (events(&stdout), stderr)
        })
        .collect();
    let said = |events: &[Value]| -> Vec<String> {
        finished(events)
            .values()
            .map(|mutant| {
                let by_proptest = mutant["caughtBy"]
                    .as_str()
                    .is_some_and(|test| test.starts_with("props::"));
                let input = if by_proptest {
                    mutant["caughtInput"].clone()
                } else {
                    Value::Null
                };
                format!("{} {} {}", mutant["id"], mutant["status"], input)
            })
            .collect()
    };
    assert_eq!(said(&runs[0].0), said(&runs[1].0));
    let (events, stderr) = &runs[0];
    let started = events
        .iter()
        .find(|event| event["reason"] == "run-started")
        .unwrap();
    assert_eq!(
        started["propertyTests"],
        json!({
            "frameworks": ["proptest", "quickcheck"],
            "seed": "0",
            "seeded": ["proptest"],
            "unseeded": ["quickcheck"],
        })
    );
    assert!(
        stderr.lines().any(|line| line
            == "note: property tests in quickcheck run with their own seeds, which mutastic \
                can't set: a mutant only some of their inputs catch can be caught in one run and \
                missed in the next"),
        "{stderr}"
    );
    let mutants = finished(events);
    // Seed 0 draws no 18 for the property on ages, in any run.
    assert_eq!(mutants[AT_18]["status"], "Survived");
    let pushes_nothing = &mutants[PUSHES_NOTHING];
    assert_eq!(pushes_nothing["caughtBy"], "props::reverses_twice");
    let input = pushes_nothing["caughtInput"].as_str().unwrap();
    assert!(
        input.starts_with("xs = [") && input != "xs = [0]",
        "the first case that fails, not shrunk: {input}"
    );
    let doubled: Vec<&Value> = mutants
        .values()
        .filter(|mutant| mutant["location"]["method"] == "double")
        .collect();
    assert!(!doubled.is_empty());
    for mutant in doubled {
        assert_eq!(mutant["caughtBy"], "checks::doubles", "{mutant}");
    }
}

/// Another seed is another draw: seed 2 draws an 18, which catches the boundary that seed 0's
/// draw misses. A mutant run by id has its input shrunk once, for its card: `xs = [0]`, not the
/// long list the run caught it with.
#[test]
fn a_named_mutants_input_is_shrunk_for_its_card() {
    let dir = copy("named");
    let (code, stdout, stderr) = mutastic(
        &dir,
        &[
            "run",
            ".",
            "--format=jsonl",
            &format!("--only={AT_18},{PUSHES_NOTHING}"),
        ],
        Some("2"),
    );
    assert_eq!(code, 0, "{stdout}{stderr}");
    let all = events(&stdout);
    let mutants = finished(&all);
    assert_eq!(mutants[AT_18]["status"], "Killed");
    assert_eq!(mutants[AT_18]["caughtInput"], "age = 18");
    assert_eq!(mutants[PUSHES_NOTHING]["caughtInput"], "xs = [0]");
    let started = all
        .iter()
        .find(|event| event["reason"] == "run-started")
        .unwrap();
    assert_eq!(started["propertyTests"]["seed"], "2");

    let (code, out, err) = mutastic(&dir, &["verify", PUSHES_NOTHING, "."], None);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        format!("{out}{err}").contains(&format!(
            "{PUSHES_NOTHING} is caught by props::reverses_twice with `xs = [0]`"
        )),
        "{out}{err}"
    );
}
