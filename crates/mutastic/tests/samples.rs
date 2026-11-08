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
        " [✗ MISSED] no test checks discount() at total == 100
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
    assert!(!card.contains('\x1b'), "NO_COLOR is plain: {card:?}");

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
    assert!(cards.contains("[✗ MISSED]"), "{cards}");
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

/// `mutastic run` on pricing with `env`, as a person runs it: stdout and stderr.
fn run_with(dir: &str, format: &str, colour: bool) -> (String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mutastic"));
    command
        .args(["run", dir, format])
        .env_remove("MUTASTIC_FORMAT")
        .env_remove("MUTASTIC_AGENT")
        .env_remove("NO_COLOR")
        .env_remove("FORCE_COLOR");
    if colour {
        command.env("FORCE_COLOR", "1");
    }
    let output = command.output().unwrap();
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The full view in colour is the Java edition's, element by element and code for code, from the
/// progress lines to the verdict; piped, without `FORCE_COLOR`, it's plain.
#[test]
fn the_full_view_is_coloured_as_the_java_editions() {
    let dir = fixture("pricing");
    let (out, err) = run_with(&dir, "--format=full", true);
    let shows = |text: &str, what: &str, styled: &str| {
        assert!(text.contains(styled), "{what}: no {styled:?} in\n{text:?}");
    };
    shows(
        &err,
        "the first line says what's read",
        &format!(
            "mutastic {} · reading the Cargo package…\n",
            env!("CARGO_PKG_VERSION")
        ),
    );
    shows(&err, "the coverage line", "coverage  3 tests on rustc ");
    shows(
        &err,
        "a file's line: red ❯, the package on cyan, dim counts with the misses red",
        " \x1b[1;31m❯\x1b[0m \x1b[46;30;1m pricing \x1b[0m src/lib.rs \x1b[2m(\x1b[0m\x1b[2m11 mutants\x1b[0m\x1b[2m | \x1b[0m\x1b[1;31m1 missed\x1b[0m\x1b[2m)\x1b[0m \x1b[2m",
    );
    shows(
        &err,
        "each miss under its file: red ×, dim line, the change's code bold",
        "\n   \x1b[1;31m×\x1b[0m discount() \x1b[2m:26\x1b[0m \x1b[1m>=\x1b[0m becomes \x1b[1m>\x1b[0m\n",
    );
    shows(
        &out,
        "the Missed title is bold red",
        "\n\x1b[1;31m\x1b[1m Missed \x1b[0m\x1b[0m\x1b[2m─",
    );
    shows(
        &out,
        "the file's ❯ is blue",
        "\n \x1b[1;34m❯\x1b[0m \x1b[1msrc/lib.rs\x1b[0m  \x1b[2m1 missed in 1 function\x1b[0m\n",
    );
    shows(
        &out,
        "✗ MISSED is one red label, the headline plain with its code bold",
        "\n \x1b[41;30;1m ✗ MISSED \x1b[0m no test checks \x1b[1mdiscount()\x1b[0m at \x1b[1mtotal == 100\x1b[0m\n",
    );
    shows(
        &out,
        "done when's outcome is dim, so it doesn't read as part of the command",
        " \x1b[2mpasses\x1b[0m\n",
    );
    shows(
        &out,
        "the marker is bold red",
        "\x1b[1;31m^^ changed to >\x1b[0m\n",
    );
    shows(
        &out,
        "= mutant: only the - is red",
        "\x1b[2m=\x1b[0m mutant  \x1b[1;31m- \x1b[0mif total >= 100 {\n",
    );
    shows(
        &out,
        "only the + is green",
        "\x1b[32m+ \x1b[0mif total > 100 {\n",
    );
    shows(
        &out,
        "note is bold blue, its code bold",
        "\x1b[2m=\x1b[0m \x1b[1;34m\x1b[1mnote\x1b[0m\x1b[0m: every test calls \x1b[1mdiscount()\x1b[0m away from the boundary\n",
    );
    shows(
        &out,
        "inputs seen is dim",
        "        \x1b[2minputs seen: 50, 250 (none is 100)\x1b[0m\n",
    );
    shows(
        &out,
        "covered by is dim",
        "        \x1b[2mcovered by pricing::big_orders_get_ten_percent_off",
    );
    shows(
        &out,
        "help is bold green, its code bold",
        "\x1b[2m=\x1b[0m \x1b[32m\x1b[1mhelp\x1b[0m\x1b[0m: test \x1b[1mdiscount()\x1b[0m with \x1b[1mtotal = 100\x1b[0m and assert the result\n",
    );
    shows(
        &out,
        "Files: clean bold green, with misses bold red, the total dim",
        " \x1b[2m    Files\x1b[0m  \x1b[32m\x1b[1m0 clean\x1b[0m\x1b[0m\x1b[2m | \x1b[0m\x1b[1;31m\x1b[1m1 with misses\x1b[0m\x1b[0m\x1b[2m (1)\x1b[0m\n",
    );
    shows(
        &out,
        "Mutants: caught bold green, missed bold red, the rest dim",
        " \x1b[2m  Mutants\x1b[0m  \x1b[32m✓ \x1b[1m10 caught\x1b[0m\x1b[0m\x1b[2m · \x1b[0m\x1b[1;31m✗ \x1b[1m1 missed\x1b[0m\x1b[0m\x1b[2m (11)\x1b[0m\n",
    );
    shows(
        &out,
        "Worst at",
        " \x1b[2m Worst at\x1b[0m  Pricing::discount() 1 ✗\n",
    );
    // How many tests ran follows timing: a mutant tries the fastest binary's tests first, and
    // stops at the first that fails.
    let runs = regex::Regex::new(
        r" \x1b\[2mTest runs\x1b\[0m  \d+\x1b\[2m \(33 without coverage selection\)\x1b\[0m\n",
    )
    .unwrap();
    assert!(
        runs.is_match(&out),
        "Test runs, and without coverage selection dim: {out:?}"
    );
    shows(&out, "Start at", " \x1b[2m Start at\x1b[0m  ");
    shows(&out, "Duration's parts are dim", "\x1b[2m(setup ");
    shows(
        &out,
        "a passing score is bold green, its threshold dim",
        " \x1b[2m    Score\x1b[0m  \x1b[32m\x1b[1m90.9%\x1b[0m\x1b[0m\x1b[2m  threshold 80\x1b[0m\n",
    );
    shows(
        &out,
        "PASS on green",
        "\n \x1b[42;30;1m PASS \x1b[0m \x1b[32m1 missed, above the threshold.\x1b[0m\n",
    );

    let (out, err) = run_with(&dir, "--format=concise", false);
    assert!(!out.contains('\x1b'), "piped is plain: {out:?}");
    assert!(!err.contains('\x1b'), "piped is plain: {err:?}");
    shows(
        &err,
        "a file's line, plain",
        " ❯ [pricing] src/lib.rs (11 mutants | 1 missed) ",
    );
    shows(
        &out,
        "Worst at, plain",
        "  Worst at  Pricing::discount() 1 ✗\n",
    );
}

/// The agent's list is ranked as the Java edition ranks it: a condition or a removal before
/// arithmetic, whatever reaches it; among equals, the one more tests run first; then the order found.
/// The ranking fixture finds its three misses in the opposite order.
#[test]
fn missed_mutants_are_ranked_as_the_java_edition_ranks_them() {
    let (_, stdout) = mutastic(&["run", &fixture("ranking"), "--format=agent"]);
    let places: Vec<&str> = stdout
        .lines()
        .filter_map(|line| line.strip_prefix("   at "))
        .collect();
    assert_eq!(
        places,
        ["src/lib.rs:15", "src/lib.rs:11", "src/lib.rs:6"],
        "is_small's condition (3 tests), is_big's (1 test), then doubled's arithmetic (3 tests):\n{stdout}"
    );
}
