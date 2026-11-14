//! Tables of cases and property tests, on `fixtures/cases`: rstest's `#[case]` and `#[values]`,
//! test-case's rows, a loop over rows and a proptest. libtest lists each case as a test of its
//! own, `rstest_tests::adds::case_1`, and no function has that name, so wherever mutastic maps a
//! test back to its source, it must map it to the function the case is written as.

#![allow(
    clippy::unwrap_used,
    reason = "a helper here asserts as the tests it serves do: a value that isn't there fails the test"
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

mod common;

/// The fixture as it's checked in, in a copy this test process has to itself.
fn copy(label: &str) -> PathBuf {
    common::own_copy("cases", &format!("cases-{label}"))
}

fn mutastic(dir: &Path, args: &[&str]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args(args)
        .current_dir(dir)
        .env_remove("MUTASTIC_FORMAT")
        .env_remove("MUTASTIC_AGENT")
        .env_remove("PROPTEST_RNG_SEED")
        .env("MUTASTIC_PROGRESS", "none")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned()
            + &String::from_utf8_lossy(&output.stderr),
    )
}

/// The mutants a jsonl run finished, by id.
fn finished(stdout: &str) -> Vec<Value> {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|event| event["reason"] == "mutant-finished")
        .collect()
}

/// A git repository of `dir`, its tree committed: where the gate starts a task.
fn committed(dir: &Path) -> impl Fn(&[&str]) + '_ {
    let git = move |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}");
    };
    std::fs::remove_dir_all(dir.join(".git")).ok();
    git(&["init", "-q"]);
    std::fs::write(dir.join(".gitignore"), "target\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "start"]);
    git
}

fn edit(file: &Path, from: &str, to: &str) {
    let text = std::fs::read_to_string(file).unwrap();
    assert!(text.contains(from), "{from} in {}", file.display());
    std::fs::write(file, text.replacen(from, to, 1)).unwrap();
}

fn append(file: &Path, more: &str) {
    let text = std::fs::read_to_string(file).unwrap();
    std::fs::write(file, text + more).unwrap();
}

/// Each case is run, covers and catches by the name libtest lists it by, and the command a red
/// case offers runs that case alone: `--exact`, so `case_1` doesn't also run `case_10`.
#[test]
fn cases_are_selected_by_their_exact_names() {
    let dir = copy("exact");
    let (code, stdout) = mutastic(&dir, &["run", ".", "--format=jsonl"]);
    assert_eq!(code, 1, "three rows are too weak: {stdout}");
    let mutants = finished(&stdout);
    let covered: Vec<&str> = mutants
        .iter()
        .flat_map(|m| m["coveredBy"].as_array().unwrap())
        .filter_map(Value::as_str)
        .collect();
    for case in [
        "rstest_tests::discounts::case_2_at_the_edge",
        "rstest_tests::multiplying_by_one_keeps_it::a_1_2::b_1_1",
        "rows::multiplies::test_case_1_one_by_one",
        "table_tests::subtracting_nothing",
        "prop_tests::adding_zero_keeps_it",
    ] {
        assert!(covered.contains(&case), "{case} in {covered:?}");
    }
    // `>=` becoming `>` is caught by the case on the edge alone.
    let edge = mutants
        .iter()
        .find(|m| m["mutation"]["applied"]["text"] == "total >= 100 -> (total) > (100)")
        .unwrap();
    assert_eq!(edge["status"], "Killed", "{edge}");
    assert_eq!(
        edge["caughtBy"],
        "rstest_tests::discounts::case_2_at_the_edge"
    );

    // A red case: the command offered runs it, and only it.
    edit(
        &dir.join("src/lib.rs"),
        "#[case(100, 0, 100)]",
        "#[case(100, 0, 99)]",
    );
    let (code, out) = mutastic(&dir, &["run", "."]);
    assert_eq!(code, 2, "{out}");
    let command = out
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("cargo test") && line.contains("--exact"))
        .unwrap_or_else(|| panic!("a command to run it: {out}"));
    assert!(
        command.ends_with("-- --exact rstest_tests::adds::case_1"),
        "{command}"
    );
    let words: Vec<&str> = command.split_whitespace().collect();
    let ran = Command::new(words[0])
        .args(&words[1..])
        .current_dir(&dir)
        .env("CARGO_TARGET_DIR", dir.join("target/rerun"))
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&ran.stdout);
    assert!(
        said.contains(
            "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 9 filtered out"
        ),
        "{said}"
    );
}

/// A case that failed at a check, then passes, is still there: the gate looks it up by the name
/// libtest lists, not by a function called `case_1`, which no crate has. Before, the gate said it
/// was gone, at every check, until the task was closed. The same for a test-case row in an
/// integration test, whose name a run gives with its target's.
#[test]
fn a_red_case_fixed_is_not_gone() {
    let dir = copy("gate-red");
    let _git = committed(&dir);
    let (code, out) = mutastic(&dir, &["gate", "open"]);
    assert_eq!(code, 0, "{out}");
    let lib = dir.join("src/lib.rs");
    append(
        &lib,
        "\npub fn half(x: i64) -> i64 {\n    let _ = x;\n    0\n}\n\n\
         #[cfg(test)]\nmod halving {\n    use super::*;\n    use rstest::rstest;\n\n    \
         #[rstest]\n    #[case(4, 2)]\n    #[case(0, 0)]\n    \
         fn halves(#[case] x: i64, #[case] half_of_it: i64) {\n        \
         assert_eq!(half(x), half_of_it);\n    }\n}\n",
    );
    let (code, out) = mutastic(&dir, &["gate", "check"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("\nhalving::halves::case_1\n"), "{out}");
    edit(&lib, "let _ = x;\n    0\n", "x / 2\n");
    let (code, out) = mutastic(&dir, &["gate", "check"]);
    assert!(!out.contains("gone now"), "{out}");
    assert_eq!(code, 0, "{out}");

    let rows = dir.join("tests/rows.rs");
    append(
        &lib,
        "\npub fn third(x: i64) -> i64 {\n    let _ = x;\n    0\n}\n",
    );
    append(
        &rows,
        "\n#[test_case(9, 3 ; \"nine\")]\nfn thirds(x: i64, third: i64) {\n    \
         assert_eq!(cases::third(x), third);\n}\n",
    );
    let (code, out) = mutastic(&dir, &["gate", "check"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("\nrows::thirds::test_case_1_nine\n"), "{out}");
    edit(&lib, "let _ = x;\n    0\n", "x / 3\n");
    let (code, out) = mutastic(&dir, &["gate", "check"]);
    assert!(!out.contains("gone now"), "{out}");
    assert_eq!(code, 0, "{out}");

    // Taking the red row away is still taking a failing test away.
    edit(
        &rows,
        "#[test_case(9, 3 ; \"nine\")]\n",
        "#[test_case(3, 1 ; \"three\")]\n",
    );
    let (code, out) = mutastic(&dir, &["gate", "check"]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("it's gone now") && out.contains("\nrows::thirds::test_case_1_nine\n"),
        "{out}"
    );
}

/// A `#[case]` row taken away is a test taken away, as a test-case row is: the gate counts the
/// tests libtest lists, every row and matrix cell, not the attributes that mark a test.
#[test]
fn removing_a_case_row_is_fewer_tests() {
    let dir = copy("gate-rows");
    let _git = committed(&dir);
    let (code, out) = mutastic(&dir, &["gate", "open"]);
    assert_eq!(code, 0, "{out}");
    edit(
        &dir.join("src/lib.rs"),
        "    #[case::just_under(99, 0)]\n",
        "",
    );
    let (code, out) = mutastic(&dir, &["gate", "check"]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains(
            "mutastic gate: fewer tests run than when the task started (13, 0 ignored; now 12, 0 ignored)"
        ),
        "{out}"
    );
}

/// What a jsonl run says of each mutant that can change from one run to the next: its status,
/// what caught it, the inputs seen and the test suggested.
fn outcomes(stdout: &str) -> Vec<String> {
    finished(stdout)
        .iter()
        .map(|m| {
            format!(
                "{} {} {} {} {}",
                m["id"], m["status"], m["caughtBy"], m["inputs"], m["suggestedTest"]["name"]
            )
        })
        .collect()
}

/// Every saved proptest case under `dir`, but its build's.
fn saved_cases(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if name == "proptest-regressions" {
                found.push(path);
            } else if path.is_dir() && name != "target" && name != "build" {
                stack.push(path);
            }
        }
    }
    found.sort();
    found
}

/// A proptest that fails for about half the cases it could draw, one case a run, as a property
/// with a rare counterexample does. Its cases are drawn from a fixed seed, so every run is the
/// same run: it can't fail in one, be caught as flaky in the next, and stop a third. Before, its
/// first failure saved the case, the check's reruns replayed it and failed, and the run stopped
/// on a test that passes most of the time. Nothing is saved: not in the copy, where a later run
/// would replay it, and never in the person's tree, where a case they saved is left as it was.
#[test]
fn proptest_runs_are_deterministic() {
    let dir = copy("proptest");
    append(
        &dir.join("src/lib.rs"),
        "\n#[cfg(test)]\nmod rare {\n    use super::*;\n    use proptest::prelude::*;\n\n    \
         proptest! {\n        #![proptest_config(ProptestConfig::with_cases(1))]\n\n        \
         #[test]\n        fn discounts_stay_small(total in 0i64..2000) {\n            \
         prop_assert!(discount(total) < 90);\n        }\n    }\n}\n",
    );
    let kept = "# Seeds for failure cases proptest has generated in the past.\n\
                cc 0000000000000000000000000000000000000000000000000000000000000001\n";
    std::fs::create_dir_all(dir.join("proptest-regressions")).unwrap();
    std::fs::write(dir.join("proptest-regressions/lib.txt"), kept).unwrap();
    let runs: Vec<(i32, Vec<String>)> = (0..3)
        .map(|_| {
            let (code, stdout) = mutastic(&dir, &["run", ".", "--format=jsonl"]);
            assert!(code == 1 || code == 2, "{stdout}");
            let said = if code == 2 {
                stdout
                    .lines()
                    .filter(|line| !line.starts_with('{'))
                    .map(str::to_string)
                    .collect()
            } else {
                outcomes(&stdout)
            };
            (code, said)
        })
        .collect();
    assert_eq!(runs[0], runs[1]);
    assert_eq!(runs[1], runs[2]);
    assert_eq!(
        std::fs::read_to_string(dir.join("proptest-regressions/lib.txt")).unwrap(),
        kept,
        "the person's own"
    );
    assert_eq!(saved_cases(&dir), [dir.join("proptest-regressions")]);
    assert_eq!(
        saved_cases(&dir.join("target/mutastic/copy")),
        Vec::<PathBuf>::new()
    );
}

/// A property that fails at random, whatever the code does, can't catch a mutant: with its cases
/// drawn from a fixed seed it fails every run or none, and the guard checks each catch. So the
/// mutants only it reaches are never caught by it, in any run.
#[test]
fn a_property_failing_at_random_catches_nothing() {
    let dir = copy("random");
    append(
        &dir.join("src/lib.rs"),
        "\npub fn triple(x: i64) -> i64 {\n    x * 3\n}\n\n\
         #[cfg(test)]\nmod tripling {\n    use super::*;\n    use proptest::prelude::*;\n\n    \
         proptest! {\n        #![proptest_config(ProptestConfig::with_cases(1))]\n\n        \
         #[test]\n        fn triples_stay_small(x in 0i64..2000) {\n            \
         prop_assert!(triple(x) >= 0 && x >= 1000);\n        }\n    }\n}\n",
    );
    let mut seen = Vec::new();
    for _ in 0..3 {
        let (code, stdout) = mutastic(&dir, &["run", ".", "--format=jsonl"]);
        assert!(code == 1 || code == 2, "{stdout}");
        let tripled: Vec<String> = finished(&stdout)
            .iter()
            .filter(|m| m["coveredBy"] == serde_json::json!(["tripling::triples_stay_small"]))
            .map(|m| format!("{} {} {}", m["id"], m["status"], m["caughtBy"]))
            .collect();
        assert!(
            !tripled.is_empty(),
            "the seed draws a case it passes: {stdout}"
        );
        for said in &tripled {
            assert!(!said.contains("\"Killed\""), "{said}");
        }
        seen.push((code, tripled));
    }
    assert_eq!(seen[0], seen[1]);
    assert_eq!(seen[1], seen[2]);
}

/// Where several cases catch a mutant, as cases written alike do, the one named is the same in
/// every run: the first as the tests are listed, the library's before the other targets', not the
/// first of the threads to finish.
#[test]
fn caught_by_is_stable_across_runs() {
    let dir = copy("stable");
    let runs: Vec<Vec<String>> = (0..3)
        .map(|_| {
            let (code, stdout) = mutastic(&dir, &["run", ".", "--format=jsonl"]);
            assert_eq!(code, 1, "{stdout}");
            finished(&stdout)
                .iter()
                .map(|m| format!("{} {} {}", m["id"], m["status"], m["caughtBy"]))
                .collect()
        })
        .collect();
    assert_eq!(runs[0], runs[1]);
    assert_eq!(runs[1], runs[2]);
    // Changing `add` fails the rstest case, the test-case row and the property alike: the
    // library's first, as listed, is named.
    let (_, stdout) = mutastic(&dir, &["run", ".", "--format=jsonl"]);
    let minus = finished(&stdout)
        .into_iter()
        .find(|m| m["location"]["method"] == "add" && m["status"] == "Killed")
        .unwrap();
    assert_eq!(
        minus["coveredBy"],
        serde_json::json!([
            "prop_tests::adding_zero_keeps_it",
            "rows::adds::test_case_1_a_hundred_and_nothing",
            "rstest_tests::adds::case_1"
        ])
    );
    assert_eq!(
        minus["caughtBy"], "prop_tests::adding_zero_keeps_it",
        "{minus}"
    );
}
