//! proptastic's properties, on `fixtures/proptastic`. proptastic isn't published yet, so the
//! fixture's tests use `surface/`, a stand-in for the surface mutastic depends on: its
//! environment variables, a seed only as `0x` and hex digits, a stream keyed on the test's name,
//! and the report its failure panics with, stacked when a value is wide. The tests reach it
//! through `testkit`, a crate that wraps it, so mutastic finds it in Cargo.lock, not the manifest.
//! With `MUTASTIC_PROPTASTIC` naming proptastic (a version, or a path to its crate), the same
//! runs go against the library.

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

/// `reversed` returning `Vec::new()`: every list but the empty one fails.
const EMPTY: &str = "ma076681c";
/// `sorted` returning `Vec::new()`: every list but the empty one fails, and the first drawn is
/// wide.
const SORTED_EMPTY: &str = "mc8346ed7";
/// `is_adult` returning `false`: every age from 18 fails.
const NEVER_ADULT: &str = "m23fadaeb";

fn copy(label: &str) -> PathBuf {
    common::own_copy("proptastic", &format!("proptastic-{label}"))
}

/// mutastic's exit code, stdout and stderr, with `env` set besides.
fn mutastic(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mutastic"));
    command
        .args(args)
        .current_dir(dir)
        .env_remove("MUTASTIC_FORMAT")
        .env_remove("MUTASTIC_AGENT")
        .env_remove("PROPTEST_RNG_SEED")
        .env_remove("PROPTASTIC_SEED")
        .env("MUTASTIC_PROGRESS", "none")
        .env("NO_COLOR", "1")
        .envs(env.iter().copied());
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

fn started(events: &[Value]) -> &Value {
    &events
        .iter()
        .find(|event| event["reason"] == "run-started")
        .unwrap()["propertyTests"]
}

/// Each mutant's status and the input that caught it.
fn said(events: &[Value]) -> Vec<String> {
    finished(events)
        .values()
        .map(|mutant| {
            format!(
                "{} {} {}",
                mutant["id"], mutant["status"], mutant["caughtInput"]
            )
        })
        .collect()
}

/// What proptastic writes in a project, anywhere under `dir`: its events and saved failures under
/// `target/proptastic/`, its corpus under `.proptastic/`.
fn written(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut next = vec![dir.to_path_buf()];
    while let Some(dir) = next.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let parent = dir.file_name().unwrap_or_default();
            if name == ".proptastic" || (name == "proptastic" && parent == "target") {
                found.push(path);
            } else if name != "debug" && name != "release" && name != ".fingerprint" {
                next.push(path);
            }
        }
    }
    found
}

/// The input without its draw's name: `[0]` for `xs = [0]`.
fn value(mutant: &Value) -> String {
    let input = mutant["caughtInput"].as_str().unwrap();
    input
        .split_once(" = ")
        .map_or(input, |(_, value)| value)
        .to_string()
}

/// Two runs are one run, with proptastic found through the crate that wraps it, seeded from
/// `0x0` (the stand-in refuses a bare number, so nothing would pass), and its first
/// counterexample read, unshrunk, from the stacked form when it's wide. No run writes the
/// person's files: no events, no saved failure, no corpus.
#[test]
fn a_run_with_the_same_seed_is_the_same_run() {
    let dir = copy("twice");
    let runs: Vec<(Vec<Value>, String)> = (0..2)
        .map(|_| {
            let (code, stdout, stderr) = mutastic(&dir, &["run", ".", "--format=jsonl"], &[]);
            assert_eq!(code, 0, "{stdout}{stderr}");
            (events(&stdout), stderr)
        })
        .collect();
    assert_eq!(said(&runs[0].0), said(&runs[1].0));
    let (events, stderr) = &runs[0];
    assert_eq!(
        started(events),
        &json!({"frameworks": ["proptastic"], "seed": "0", "seeded": ["proptastic"]})
    );
    assert!(!stderr.contains("note: property tests"), "{stderr}");
    let wide = finished(events)[SORTED_EMPTY]["caughtInput"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        wide.starts_with("a = [") && wide.matches(',').count() > 5,
        "the first list that failed, stacked: {wide}"
    );
    assert_eq!(written(&dir), Vec::<PathBuf>::new());
}

/// A mutant run by id has its input shrunk once, for its card. The person's seed, `0x` and hex
/// digits, is the run's, said in decimal, and their colour setting doesn't reach the report
/// mutastic reads.
#[test]
fn a_named_mutants_input_is_shrunk_for_its_card() {
    let dir = copy("named");
    let only = format!("--only={EMPTY},{SORTED_EMPTY},{NEVER_ADULT}");
    let theirs = [("PROPTASTIC_SEED", "0x2a"), ("PROPTASTIC_COLOR", "always")];
    let (code, stdout, stderr) = mutastic(&dir, &["run", ".", "--format=jsonl", &only], &theirs);
    assert_eq!(code, 0, "{stdout}{stderr}");
    let all = events(&stdout);
    let mutants = finished(&all);
    assert_eq!(mutants[EMPTY]["caughtInput"], "a = [0]");
    assert_eq!(mutants[SORTED_EMPTY]["caughtInput"], "a = [0]");
    assert_eq!(mutants[NEVER_ADULT]["caughtInput"], "a = 18");
    assert_eq!(started(&all)["seed"], "42");

    let (code, out, err) = mutastic(&dir, &["verify", SORTED_EMPTY, "."], &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        format!("{out}{err}").contains(&format!(
            "{SORTED_EMPTY} is caught by props::sorts_as_std_does with `a = [0]`"
        )),
        "{out}{err}"
    );
}

/// A seed proptastic can't read is the person's to fix: it's passed on as they set it, proptastic
/// refuses it, and the run names proptastic as one mutastic couldn't seed.
#[test]
fn a_seed_proptastic_cannot_read_is_left_as_it_is() {
    let dir = copy("bare");
    let (code, stdout, stderr) = mutastic(
        &dir,
        &["run", ".", "--format=jsonl"],
        &[("PROPTASTIC_SEED", "12345")],
    );
    assert_ne!(code, 0, "{stdout}{stderr}");
    assert!(
        stderr.contains("PROPTASTIC_SEED is 12345, which isn't a seed"),
        "{stderr}"
    );
}

/// The same runs against proptastic itself, once it can be had: `MUTASTIC_PROPTASTIC` names it, a
/// version on crates.io or the path to its crate. It names its draws as the closure does, `xs =
/// [0]`, where the stand-in says `a`.
#[test]
fn the_library_itself_is_seeded_and_read_the_same() {
    let Ok(library) = std::env::var("MUTASTIC_PROPTASTIC") else {
        eprintln!("MUTASTIC_PROPTASTIC isn't set: proptastic itself isn't run");
        return;
    };
    let dir = copy("library");
    let spec = if library.starts_with(['/', '.']) {
        format!("{{ path = {library:?} }}")
    } else {
        format!("{library:?}")
    };
    let testkit = dir.join("testkit/Cargo.toml");
    let manifest = std::fs::read_to_string(&testkit).unwrap();
    let manifest = manifest.replace(r#"{ path = "../surface" }"#, &spec);
    std::fs::write(&testkit, manifest).unwrap();
    let root = dir.join("Cargo.toml");
    let manifest = std::fs::read_to_string(&root).unwrap();
    std::fs::write(&root, manifest.replace(r#", "surface""#, "")).unwrap();
    let _ = std::fs::remove_file(dir.join("Cargo.lock"));

    let runs: Vec<Vec<Value>> = (0..2)
        .map(|_| {
            let (code, stdout, stderr) = mutastic(&dir, &["run", ".", "--format=jsonl"], &[]);
            assert_eq!(code, 0, "{stdout}{stderr}");
            events(&stdout)
        })
        .collect();
    assert_eq!(said(&runs[0]), said(&runs[1]));
    assert_eq!(started(&runs[0])["seeded"], json!(["proptastic"]));
    let only = format!("--only={EMPTY},{SORTED_EMPTY},{NEVER_ADULT}");
    let (code, stdout, stderr) = mutastic(&dir, &["run", ".", "--format=jsonl", &only], &[]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    let mutants = finished(&events(&stdout));
    assert_eq!(value(&mutants[EMPTY]), "[0]");
    assert_eq!(value(&mutants[SORTED_EMPTY]), "[0]");
    assert_eq!(value(&mutants[NEVER_ADULT]), "18");
    assert_eq!(written(&dir), Vec::<PathBuf>::new());
}
