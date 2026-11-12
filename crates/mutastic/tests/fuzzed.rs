//! What the fuzz targets read, kept: each input in `tests/fuzzed/<target>/` is read as its target
//! reads it, and must give a result or an error, never a panic, which would end a whole run. An
//! input fuzzing found to panic goes here once it's fixed (fuzz/README.md), beside inputs that
//! probe each parser's corners.

#![allow(
    clippy::unwrap_used,
    reason = "a helper here asserts as the tests it serves do: a value that isn't there fails the test"
)]

use std::path::{Path, PathBuf};

use mutastic::sites::{self, UnsafeMode};

/// The inputs kept for `target`, each with its text. None kept would check nothing, so that fails.
fn kept(target: &str) -> Vec<(PathBuf, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fuzzed")
        .join(target);
    let mut inputs: Vec<(PathBuf, String)> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let text = std::fs::read_to_string(&path).unwrap();
            (path, text)
        })
        .collect();
    inputs.sort();
    assert!(!inputs.is_empty(), "{} keeps no inputs", dir.display());
    inputs
}

#[test]
fn every_kept_configuration_reads_without_a_panic() {
    for (_, text) in kept("config") {
        let _ = mutastic::config::parse(&text);
    }
}

#[test]
fn every_kept_rules_file_reads_without_a_panic() {
    for (_, text) in kept("rules") {
        let _ = mutastic::callrules::parse(&text, "fuzzed.yaml");
    }
}

#[test]
fn every_kept_ignores_file_reads_without_a_panic() {
    for (_, text) in kept("ignores") {
        let _ = mutastic::ignores::parse(&text, "ignores.yaml");
    }
}

/// Each source as a crate's only file, under every operator, unsafe code included, as the `sites`
/// target reads it.
#[test]
fn every_kept_source_gives_its_mutants_without_a_panic() {
    let dir = std::env::temp_dir().join(format!("mutastic-fuzzed-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    for (_, text) in kept("sites") {
        std::fs::write(dir.join("src/lib.rs"), text).unwrap();
        let _ = sites::find_with(
            &dir,
            "fuzzed",
            &[PathBuf::from("src/lib.rs")],
            sites::ALL,
            UnsafeMode::All,
            &mutastic::cfg::Cfg::unknown(),
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}
