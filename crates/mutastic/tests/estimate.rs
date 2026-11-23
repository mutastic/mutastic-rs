//! `mutastic mutants` estimates what a set of operators adds to a run's tests' time, on the line
//! before its count: from the last whole run, or from a coverage pass `--estimate` runs, and never
//! from a record of other code or other operators.

#![allow(
    clippy::unwrap_used,
    reason = "a helper here asserts as the tests it serves do: a value that isn't there fails the test"
)]

use std::path::Path;
use std::process::Command;

mod common;

fn mutastic(dir: &Path, args: &[&str]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args(args)
        .current_dir(dir)
        .env_remove("MUTASTIC_FORMAT")
        .env_remove("MUTASTIC_AGENT")
        .env("MUTASTIC_PROGRESS", "none")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

/// What `mutants` says after its last mutant's line: the estimates, then the count.
fn after_the_mutants(dir: &Path, args: &[&str]) -> Vec<String> {
    let listed: Vec<&str> = ["mutants", "."]
        .into_iter()
        .chain(args.iter().copied())
        .collect();
    let (code, stdout) = mutastic(dir, &listed);
    assert_eq!(code, 0, "{stdout}");
    stdout
        .lines()
        .skip_while(|line| line.starts_with('m'))
        .map(str::to_string)
        .collect()
}

/// Every case on one copy of the pricing fixture, so it's built once: with no record, after a run,
/// after `--estimate`, and once the code has changed.
#[test]
fn mutants_estimate_from_the_last_measure_of_the_same_code_and_operators() {
    let dir = common::own_copy("pricing", "estimate");

    assert_eq!(
        after_the_mutants(&dir, &[]),
        ["11 mutants"],
        "no estimate unasked"
    );
    assert_eq!(
        after_the_mutants(&dir, &["--operators=stronger"]),
        [
            "no estimate: no run is recorded here yet; run once to get estimates, or add --estimate",
            "35 mutants"
        ]
    );

    let (code, out) = mutastic(&dir, &["run", "."]);
    assert!(code == 0 || code == 1, "{out}");
    let defaults = after_the_mutants(&dir, &["--operators=defaults"]);
    assert!(
        defaults[0].starts_with("estimate: no more mutants than defaults, from the run of "),
        "{defaults:?}"
    );
    assert!(defaults[0].ends_with(" UTC"), "{defaults:?}");
    assert_eq!(after_the_mutants(&dir, &[]), ["11 mutants"]);
    // The run's switches were only at the defaults' sites: a stronger mutant elsewhere is in code
    // it didn't measure.
    let stronger = after_the_mutants(&dir, &["--operators=stronger"]);
    assert!(
        stronger[0].starts_with("no estimate: 12 of the 24 more mutants are in code the run of ")
            && stronger[0].ends_with(" didn't measure; add --estimate to measure it all"),
        "{stronger:?}"
    );

    let estimated = after_the_mutants(&dir, &["--estimate"]);
    assert_eq!(estimated.len(), 4, "{estimated:?}");
    for (line, set) in estimated.iter().zip([
        "defaults, 11 mutants",
        "stronger, 35 mutants",
        "all, 40 mutants",
    ]) {
        assert!(
            line.starts_with(&format!("estimate: {set}, up to ")),
            "{line}"
        );
        assert!(
            line.contains(" of tests, from the coverage pass of "),
            "{line}"
        );
    }
    assert_eq!(estimated[3], "11 mutants");
    let stronger = after_the_mutants(&dir, &["--operators=stronger"]);
    assert!(
        stronger[0].starts_with("estimate: 24 more mutants than defaults, up to ")
            && stronger[0].contains(" more of tests, from the coverage pass of "),
        "{stronger:?}"
    );

    // A run under other operators is no measure of this project's.
    let (code, out) = mutastic(&dir, &["run", ".", "--operators=stronger"]);
    assert!(code == 0 || code == 1, "{out}");
    let other = after_the_mutants(&dir, &["--operators=all"]);
    assert!(
        other[0].starts_with("no estimate: the run of ")
            && other[0].ends_with(" was under stronger, not this project's defaults"),
        "{other:?}"
    );

    let lib = dir.join("src/lib.rs");
    let text = std::fs::read_to_string(&lib).unwrap();
    std::fs::write(&lib, format!("{text}\n// changed\n")).unwrap();
    let changed = after_the_mutants(&dir, &["--operators=all"]);
    std::fs::write(&lib, text).unwrap();
    assert!(
        changed[0].starts_with("no estimate: the code has changed since the run of "),
        "{changed:?}"
    );
}
