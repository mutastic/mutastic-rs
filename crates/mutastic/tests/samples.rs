//! `mutastic run` on the samples, through the binary as a person runs it.

#![allow(
    clippy::unwrap_used,
    reason = "a helper here asserts as the tests it serves do: a value that isn't there fails the test"
)]

use std::path::Path;
use std::process::{Command, Stdio};

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
            "tests::small_orders_pay_full_price",
            "pricing::big_orders_get_ten_percent_off"
        ])
    );
    for event in &events {
        let errors: Vec<String> = schema().iter_errors(event).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{event}\n{}", errors.join("\n"));
    }
    // The change as the engine made it, to the source, as the schema describes `applied`.
    let applied = common::applied_schema();
    for mutant in &mutants {
        let mutation = &mutant["mutation"];
        assert!(applied.is_valid(&mutation["applied"]), "{mutant}");
        assert_eq!(mutation["applied"]["level"], "source", "{mutant}");
    }
    assert_eq!(
        missed["mutation"]["applied"]["text"],
        "total >= 100 -> (total) > (100)"
    );
    let finished = events.last().unwrap();
    assert_eq!(finished["reason"], "run-finished");
    assert_eq!(finished["score"], 90.9);
    assert_eq!(finished["killed"], 10);
    assert_eq!(events[0]["rule"], mutastic::report::RULE);
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
    │                  ^^ >= changed to >
 27 │             return total / 10;
    │
    = mutant  - if total >= 100 {{
              + if total > 100 {{
    = note: every test calls discount() away from the boundary
            inputs seen: 50, 250 (none is 100)
            covered by tests::small_orders_pay_full_price, pricing::big_orders_get_ten_percent_off (all 2 covering tests still pass)
    = help: test discount() with total = 100 and assert the result

            // in mod tests
            #[test]
            fn discount_at_the_total_edge() {{
                assert_eq!(Pricing::new().discount(100), 10); // today 10 · with this mutant 0
            }}
            check today's value is the one you intend: the test pins what the code does now

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
        cards.contains(
            "    = mutant  - x * 2\n              -     >= 4\n              + x * 2\n              +     > 4\n"
        ),
        "{cards}"
    );
}

/// A pattern that binds by `ref mut` into a field changes the field, so the copy keeps the field a
/// place, not a value in a switch: its test passes there as under `cargo test`, as url's do.
#[test]
fn a_field_a_pattern_binds_into_stays_a_place() {
    let dir = fixture("places");
    let (_, stdout) = mutastic(&["run", &dir, "--format=jsonl"]);
    let finished = events(&stdout).pop().unwrap();
    assert_eq!(finished["reason"], "run-finished", "{finished}");
}

/// A word a command doesn't take stops it, exit 2, naming the word and the command's usage, rather
/// than being dropped for a run nobody asked for; and where a mutant's id goes, a directory or any
/// other word is told apart from an id. Nothing is built: the words are checked first.
#[test]
fn a_word_a_command_doesnt_take_stops_it() {
    // A copy of its own: its verify would otherwise share the fixture's copy and coverage with
    // the tests that run pricing in place at the same time, and read theirs.
    let copy = copy_of("pricing", "words");
    let dir = copy.display().to_string();
    let stopped = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_mutastic"))
            .args(args)
            .env("NO_COLOR", "1")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        String::from_utf8_lossy(&output.stderr).into_owned()
    };
    assert_eq!(
        stopped(&["run", &dir, "--operators=stronger", "explain", "md3a59003"]),
        "mutastic: run doesn't take explain md3a59003: mutastic run [dir] [options]\n"
    );
    assert_eq!(
        stopped(&["explain", &dir]),
        format!(
            "mutastic: explain needs a mutant's id, m and eight hex digits as a card shows it, \
             and {dir} is a directory: mutastic explain <id> [dir], as in mutastic explain \
             m1a2b3c4d {dir}\n"
        )
    );
    assert_eq!(
        stopped(&["verify", "m1a2b3c4d,total", &dir]),
        "mutastic: verify needs a mutant's id, m and eight hex digits as a card shows it, and \
         m1a2b3c4d,total isn't one: mutastic verify <id>[,<id>…] [<id>…] [dir], as in mutastic verify \
         m1a2b3c4d\n"
    );
    assert_eq!(
        stopped(&["verify", "mb2f8ae57", "m0b3873d0", &dir, "extra"]),
        "mutastic: verify doesn't take extra: mutastic verify <id>[,<id>…] [<id>…] [dir]\n"
    );
    // Ids split by spaces, as the Java edition takes them, then the directory.
    let (code, out) = mutastic(&["verify", "mb2f8ae57", "m0b3873d0", &dir]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("mutastic: mb2f8ae57 is still missed\n"),
        "{out}"
    );
    assert!(out.contains("mutastic: m0b3873d0 is caught by "), "{out}");
    std::fs::remove_dir_all(&copy).ok();
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
    assert_eq!(guarded["caughtWith"], "crash: SIGABRT");
    assert!(
        guarded.get("caughtBy").is_none(),
        "no test caught it: {guarded}"
    );
    let total = fate("total", "-");
    assert!(total["caughtBy"].is_string(), "{total}");
    assert_eq!(
        total["caughtWith"], "panic: attempt to subtract with overflow",
        "{total}"
    );
    assert!(events
        .iter()
        .all(|e| e["location"]["method"] != "halve" && e["location"]["method"] != "helper"));

    // checked() is called, by run::checks, so only its line that aborts is out of reach.
    let (_, full) = mutastic(&["run", &dir, "--format=full"]);
    assert!(
        full.contains("○ src/raw.rs:25 checked() (1 mutant)"),
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
    // A copy of its own: runs of one package side by side share its build in target/mutastic.
    let copy = copy_of("pricing", "colour");
    let dir = copy.display().to_string();
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
        "\x1b[1;31m^^ >= changed to >\x1b[0m\n",
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
        "        \x1b[2mcovered by tests::small_orders_pay_full_price",
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
    // The fixture runs in place, so a run before this one may have left a last run to compare with.
    let score = regex::Regex::new(
        r" \x1b\[2m    Score\x1b\[0m  \x1b\[32m\x1b\[1m90\.9%\x1b\[0m\x1b\[0m(?<delta>\x1b\[2m  = vs last run\x1b\[0m\x1b\[2m \((?<sameness>[a-z ]+)\)\x1b\[0m)?\x1b\[2m  threshold 80\x1b\[0m\n",
    )
    .unwrap();
    assert!(
        score.is_match(&out),
        "a passing score is bold green, its threshold dim: {out:?}"
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
    std::fs::remove_dir_all(&copy).ok();
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

/// At a terminal, the counter is one line redrawn in place as each mutant finishes, as the Java
/// edition's: cleared before a file's line, drawn again after it, and cleared before the result.
#[test]
fn the_counter_is_redrawn_in_place() {
    // A copy of its own: runs of one package side by side share its build in target/mutastic.
    let copy = copy_of("pricing", "counter");
    let dir = copy.display().to_string();
    let (_, err) = run_with(&dir, "--progress=bar", false);
    let clear = "\r\x1b[2K";
    let counter = regex::Regex::new(
        r"\[11/11\] ✓ 10 caught · ✗ 1 missed · ○ 0 not covered · ⏱ 0 timed out  \d+:\d\d",
    )
    .unwrap();
    assert!(counter.is_match(&err), "the last count: {err:?}");
    assert!(err.contains(&format!("{clear}[1/11] ✓ ")), "{err:?}");
    assert!(
        err.contains(&format!("{clear} ❯ [pricing] src/lib.rs")),
        "a file's line clears the counter: {err:?}"
    );
    assert!(err.ends_with(clear), "cleared before the result: {err:?}");
    let (_, plain) = run_with(&dir, "--progress=none", false);
    assert!(
        !plain.contains(clear) && !plain.contains("[1/11]"),
        "{plain:?}"
    );
    std::fs::remove_dir_all(&copy).ok();
}

/// A full run writes `mutation.json`, in the mutation-testing report schema as the Java edition
/// writes it, and `mutation.html`, that report in the viewer, and says where in its summary and
/// its events.
#[test]
fn a_full_run_writes_the_report_files() {
    let copy = copy_of("pricing", "reports");
    let dir = copy.display().to_string();
    let out = std::env::temp_dir().join(format!("mutastic-report-files-{}", std::process::id()));
    let (json, html) = (out.join("m.json"), out.join("m.html"));
    let (code, stdout) = mutastic(&[
        "run",
        &dir,
        "--format=full",
        &format!("--report=json:{}", json.display()),
        &format!("--report=html:{}", html.display()),
    ]);
    assert_eq!(code, 0, "{stdout}");
    assert!(
        stdout.contains(&format!(
            "  Reports  {} · {}\n",
            json.display(),
            html.display()
        )),
        "{stdout}"
    );
    let report: Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    let schema: Value = serde_json::from_str(include_str!(
        "schema/report/mutation-testing-report-schema.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<String> = validator
        .iter_errors(&report)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(report["schemaVersion"], "2");
    assert_eq!(
        report["thresholds"],
        serde_json::json!({ "high": 80, "low": 60 })
    );
    let mutants = report["files"]["src/lib.rs"]["mutants"].as_array().unwrap();
    assert_eq!(mutants.len(), 11);
    let missed = mutants.iter().find(|m| m["id"] == "mb2f8ae57").unwrap();
    assert_eq!(missed["status"], "Survived");
    assert_eq!(missed["mutatorName"], "boundary");
    assert_eq!(missed["replacement"], ">");
    assert_eq!(
        missed["location"],
        serde_json::json!({ "start": { "line": 26, "column": 18 }, "end": { "line": 26, "column": 20 } })
    );
    assert_eq!(
        missed["statusReason"],
        "no test checks discount() at total == 100"
    );
    let page = std::fs::read_to_string(&html).unwrap();
    assert!(page.contains("<mutation-test-report-app titlePostfix=\"mutastic\">"));
    assert!(page.contains(".report = {\"$schema\""));
    std::fs::remove_dir_all(&out).ok();

    let (_, jsonl) = mutastic(&["run", &dir, "--format=jsonl", "--report=json:x.json"]);
    let finished = events(&jsonl).pop().unwrap();
    assert_eq!(finished["reports"], serde_json::json!(["x.json"]));
    let (_, only) = mutastic(&["run", &dir, "--format=jsonl", "--only=mb2f8ae57"]);
    assert_eq!(
        events(&only).pop().unwrap()["reports"],
        serde_json::json!([])
    );
    std::fs::remove_dir_all(&copy).ok();
}

/// A copy of a fixture in a directory of its own, to change.
fn copy_of(name: &str, label: &str) -> std::path::PathBuf {
    let to = std::env::temp_dir().join(format!("mutastic-{label}-{}", std::process::id()));
    std::fs::remove_dir_all(&to).ok();
    let from = Path::new(&fixture(name)).to_path_buf();
    let mut stack = vec![std::path::PathBuf::new()];
    while let Some(relative) = stack.pop() {
        std::fs::create_dir_all(to.join(&relative)).unwrap();
        for entry in std::fs::read_dir(from.join(&relative)).unwrap() {
            let entry = entry.unwrap();
            let path = relative.join(entry.file_name());
            if entry.file_name() == "target" {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                stack.push(path);
            } else {
                std::fs::copy(from.join(&path), to.join(&path)).unwrap();
            }
        }
    }
    to
}

fn mutastic_in(dir: &Path, args: &[&str]) -> (i32, String) {
    // Each run's own name, for the flaky fixture's test, which fails once in each run.
    static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let output = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args(args)
        .current_dir(dir)
        .env(
            "RAFFLE_RUN",
            format!("{}-{run}-{nanos}", std::process::id()),
        )
        .env_remove("MUTASTIC_FORMAT")
        .env_remove("MUTASTIC_AGENT")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned()
            + &String::from_utf8_lossy(&output.stderr),
    )
}

/// `ignore` records a decision, which skips the mutant while its line is as it was, says how to
/// undo each step, and goes stale when the line changes; a proposal waits for a person.
#[test]
fn an_ignore_holds_while_its_line_does() {
    let dir = copy_of("pricing", "ignore");
    let (code, out) = mutastic_in(
        &dir,
        &[
            "ignore",
            "mb2f8ae57",
            "--reason=totals are tens",
            "--as=unreachable",
        ],
    );
    assert_eq!(code, 0, "{out}");
    assert_eq!(
        out,
        "mutastic: mb2f8ae57 is ignored: totals are tens\n  recorded in .mutastic/ignores.yaml: commit it, as a decision to review\n  to undo: mutastic ignore --remove mb2f8ae57 .\n"
    );
    let file = std::fs::read_to_string(dir.join(".mutastic/ignores.yaml")).unwrap();
    assert!(
        file.contains(
            "    code: 'if total >= 100 {'\n    reason: 'totals are tens'\n    kind: unreachable\n"
        ),
        "{file}"
    );
    let (code, jsonl) = mutastic_in(&dir, &["run", "--format=jsonl", "--report=none"]);
    assert_eq!(code, 0, "{jsonl}");
    let first = events(&jsonl);
    let ignored = first.iter().find(|e| e["id"] == "mb2f8ae57").unwrap();
    assert_eq!(ignored["status"], "Ignored");
    assert_eq!(
        ignored["statusReason"],
        "user: unreachable: totals are tens (ignored)"
    );
    assert_eq!(ignored["skipKind"], "user");
    // The summary counts an ignore apart, by the sort it was given, as the Java edition does.
    let (_, out) = mutastic_in(&dir, &["run", "--format=full", "--report=none"]);
    assert!(
        out.contains(" · 1 skipped (1 ignored as unreachable)\n"),
        "{out}"
    );

    // The line changes: the decision no longer holds, and the mutant runs again.
    let lib = dir.join("src/lib.rs");
    let text = std::fs::read_to_string(&lib).unwrap();
    std::fs::write(
        &lib,
        text.replace("if total >= 100 {", "if total >= 100  {"),
    )
    .unwrap();
    let (_, jsonl) = mutastic_in(&dir, &["run", "--format=jsonl", "--report=none"]);
    let again = events(&jsonl);
    assert_eq!(
        again.last().unwrap()["staleIgnores"],
        serde_json::json!(["mb2f8ae57"])
    );
    let (_, out) = mutastic_in(&dir, &["ignore", "--prune"]);
    assert!(
        out.starts_with("mutastic: removed 1 ignore that no longer hold: mb2f8ae57\n"),
        "{out}"
    );

    let (_, out) = mutastic_in(
        &dir,
        &["ignore", "mb2f8ae57", "--propose", "--reason=only tens"],
    );
    assert!(
        out.contains("  to accept: mutastic ignore --accept mb2f8ae57 .\n"),
        "{out}"
    );
    let (_, out) = mutastic_in(&dir, &["ignore", "--accept", "mb2f8ae57"]);
    assert!(
        out.starts_with("mutastic: mb2f8ae57 is ignored, as proposed: only tens\n"),
        "{out}"
    );
    assert!(!dir.join(".mutastic/proposals.yaml").exists());
    let (_, out) = mutastic_in(&dir, &["ignore", "--remove", "mb2f8ae57"]);
    assert!(
        out.contains("  to undo: mutastic ignore mb2f8ae57 --reason=\"only tens\" .\n"),
        "{out}"
    );
    let (code, out) = mutastic_in(&dir, &["ignore", "mb2f8ae57"]);
    assert_eq!(code, 2, "a reason is needed: {out}");
    std::fs::remove_dir_all(&dir).ok();
}

/// `--since` runs only the mutants on the lines changed since a commit, and says how much changed.
#[test]
fn since_runs_the_changed_lines_mutants() {
    let dir = copy_of("pricing", "since");
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(&dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap()
            .status;
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    std::fs::write(dir.join(".gitignore"), "target\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "start"]);
    let lib = dir.join("src/lib.rs");
    let text = std::fs::read_to_string(&lib).unwrap();
    std::fs::write(
        &lib,
        text.replace("if total >= 100 {", "if total >= 100 {  "),
    )
    .unwrap();
    let (_, out) = mutastic_in(
        &dir,
        &["run", "--since=HEAD", "--format=full", "--report=none"],
    );
    assert!(
        out.contains("  Changed  1 line in 1 file since HEAD\n"),
        "{out}"
    );
    let (_, jsonl) = mutastic_in(
        &dir,
        &["run", "--since=HEAD", "--format=jsonl", "--report=none"],
    );
    let scoped = events(&jsonl);
    let finished = scoped.last().unwrap();
    assert_eq!(finished["scope"]["lineCount"], 1);
    assert!(scoped
        .iter()
        .filter(|e| e["reason"] == "mutant-finished")
        .all(|e| e["location"]["line"] == 26));

    // A change that only writes output has its mutant skipped as arid, and says so, as the Java
    // edition does: a run that scores nothing tells why.
    std::fs::write(
        &lib,
        text.replace(
            "        if total < self.free_shipping_from {",
            "        eprintln!(\"shipping on {total}\");\n        if total < self.free_shipping_from {",
        ),
    )
    .unwrap();
    let (_, out) = mutastic_in(
        &dir,
        &["run", "--since=HEAD", "--format=full", "--report=none"],
    );
    assert!(
        out.contains("   Mutants  ✓ 0 caught (0) · 1 skipped (1 arid)\n"),
        "{out}"
    );
    let (_, out) = mutastic_in(
        &dir,
        &["run", "--since=HEAD", "--format=agent", "--report=none"],
    );
    assert!(
        out.contains("0 caught of 0 mutants (1 more skipped)\n"),
        "{out}"
    );
    let (_, jsonl) = mutastic_in(
        &dir,
        &["run", "--since=HEAD", "--format=jsonl", "--report=none"],
    );
    let arid = events(&jsonl);
    assert_eq!(arid[1]["statusReason"], "arid: only writes output");
    assert_eq!(arid[1]["skipKind"], "arid");
    assert_eq!(arid.last().unwrap()["ignored"], 1);
    std::fs::remove_dir_all(&dir).ok();
}

/// `mutastic.yaml` chooses the operators, adds a rules file's, and skips what its rules cover, with
/// their reasons; the rules file's operator says its own words on the card.
#[test]
fn mutastic_yaml_is_read_as_the_java_editions() {
    let copy = copy_of("configured", "configured");
    let dir = copy.display().to_string();
    let (_, jsonl) = mutastic(&["run", &dir, "--format=jsonl", "--report=none"]);
    let found = events(&jsonl);
    let mutants: Vec<&Value> = found
        .iter()
        .filter(|e| e["reason"] == "mutant-finished")
        .collect();
    assert!(
        mutants
            .iter()
            .all(|e| e["operator"] != "mutastic/return-value"),
        "operators: takes return-value away: {jsonl}"
    );
    let trim = mutants
        .iter()
        .find(|e| e["operator"] == "acme/remove-trim")
        .expect("the rules file's operator");
    assert_eq!(trim["status"], "Survived");
    assert!(
        trim["rendered"]
            .as_str()
            .unwrap()
            .contains("no test passes `clean()` text with spaces for `.trim()` to remove"),
        "{trim}"
    );
    let generated: Vec<&&Value> = mutants
        .iter()
        .filter(|e| e["location"]["method"] == "checksum")
        .collect();
    assert!(!generated.is_empty(), "{jsonl}");
    for mutant in generated {
        assert_eq!(mutant["status"], "Ignored");
        assert_eq!(
            mutant["statusReason"],
            "user: written by a code generator (rule `skip: configured::generated::**`)"
        );
    }
    // --operators replaces operators:, and can name the rules file's.
    let (_, only) = mutastic(&["mutants", &dir, "--operators=acme/remove-trim"]);
    assert!(
        only.contains("acme/remove-trim") && only.contains("\n1 mutants\n"),
        "{only}"
    );
    // What mutastic doesn't understand is an error that names the line.
    std::fs::write(copy.join("mutastic.yaml"), "rules:\n  - skip: x::**\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args(["mutants", &dir])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("mutastic.yaml line 2: the rule `skip: x::**` needs a `reason:` under it"));
    std::fs::remove_dir_all(&copy).ok();
}

/// A package uses its nearest `mutastic.yaml`, so it gets the same rules from the workspace's top as
/// from its own directory; once, a run from the top read only the top's file. And a file adds
/// another's when it says `extends:`.
#[test]
fn a_packages_own_mutastic_yaml_applies_from_the_workspaces_top() {
    let copy = copy_of("workspace", "nearest-yaml");
    // One package, so the workspace's top runs it.
    std::fs::write(
        copy.join("Cargo.toml"),
        "[workspace]\nmembers = [\"shop\"]\nresolver = \"2\"\n",
    )
    .unwrap();
    let status_reason = |dir: &Path| {
        let (_, jsonl) = mutastic(&[
            "run",
            &dir.display().to_string(),
            "--format=jsonl",
            "--report=none",
        ]);
        let found = events(&jsonl);
        let total = found
            .iter()
            .find(|e| e["reason"] == "mutant-finished" && e["location"]["method"] == "total")
            .unwrap_or_else(|| panic!("a mutant in total: {jsonl}"))
            .clone();
        total["statusReason"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };
    let own = "user: summed by the till (rule `skip: shop::total`)";
    std::fs::write(
        copy.join("shop/mutastic.yaml"),
        "rules:\n  - skip: shop::total\n    reason: summed by the till\n",
    )
    .unwrap();
    assert_eq!(status_reason(&copy), own);
    assert_eq!(status_reason(&copy.join("shop")), own);
    std::fs::write(copy.join("shop/mutastic.yaml"), "extends: ../shared.yaml\n").unwrap();
    std::fs::write(
        copy.join("shared.yaml"),
        "rules:\n  - skip: shop::**\n    reason: shared by the team\n",
    )
    .unwrap();
    assert_eq!(
        status_reason(&copy),
        "user: shared by the team (rule `skip: shop::**`)"
    );
    std::fs::remove_dir_all(&copy).ok();
}

fn mutastic_with_input(dir: &Path, args: &[&str], input: &str) -> (i32, String) {
    use std::io::Write as _;
    let mut child = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args(args)
        .current_dir(dir)
        .env_remove("MUTASTIC_FORMAT")
        .env_remove("MUTASTIC_AGENT")
        .env("NO_COLOR", "1")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

/// `gate` holds a task to its changed lines' mutants, as the Java edition's does: it sends the
/// agent back with the cards while one is missed, won't pass a task that took a test away or
/// ignored a mutant itself, lets the agent stop when it changed nothing since it was sent back,
/// and answers a Claude Code hook as one reads it.
#[test]
fn the_gate_holds_a_task_to_its_changed_lines() {
    let dir = copy_of("pricing", "gate");
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(&dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}");
    };
    let (code, out) = mutastic_in(&dir, &["gate", "open"]);
    assert_eq!(code, 2, "outside git: {out}");
    assert!(
        out.contains("the gate needs a git repository with a commit"),
        "{out}"
    );
    git(&["init", "-q"]);
    std::fs::write(dir.join(".gitignore"), "target\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "start"]);
    let (code, out) = mutastic_in(&dir, &["gate", "check"]);
    assert_eq!(code, 2, "none open: {out}");
    let (code, out) = mutastic_with_input(
        &dir,
        &["gate", "open", ".", "--hook=claude"],
        r#"{"session_id": "s1"}"#,
    );
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("mutastic gate: this task is gated from "),
        "{out}"
    );
    let (_, again) = mutastic_with_input(
        &dir,
        &["gate", "open", ".", "--hook=claude"],
        r#"{"session_id": "s1"}"#,
    );
    assert_eq!(again, "", "once for each session");

    let (code, out) = mutastic_in(&dir, &["gate", "check"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("mutastic gate: passed: the task changed no lines mutastic mutates\n"),
        "{out}"
    );

    // A change no test checks: the hook sends the agent back.
    let lib = dir.join("src/lib.rs");
    let text = std::fs::read_to_string(&lib).unwrap();
    std::fs::write(
        &lib,
        format!("{text}\npub fn double(x: u32) -> u32 {{\n    x * 2\n}}\n"),
    )
    .unwrap();
    let (_, hooked) = mutastic_with_input(&dir, &["gate", "check", ".", "--hook=claude"], "{}");
    let answer: Value = serde_json::from_str(hooked.trim()).unwrap();
    assert_eq!(answer["decision"], "block", "{hooked}");
    // Sent back, the agent changed nothing: it may stop, and the person is told what's left.
    let (_, hooked) = mutastic_with_input(&dir, &["gate", "check", ".", "--hook=claude"], "{}");
    let answer: Value = serde_json::from_str(hooked.trim()).unwrap();
    assert!(
        answer["systemMessage"]
            .as_str()
            .unwrap()
            .starts_with("mutastic gate: nothing changed since the agent was sent back"),
        "{hooked}"
    );
    // Run by a person, it says what's left, and fails.
    let (code, out) = mutastic_in(&dir, &["gate", "check"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("on the lines the task changed"), "{out}");
    assert!(out.contains("so it isn't done"), "{out}");
    // What holds it is named by id, the mutants no test reaches too.
    assert!(
        out.contains("so it isn't done (no test reaches: m"),
        "{out}"
    );
    assert!(
        out.contains("not covered (no test reaches these lines):\n- src/lib.rs:"),
        "{out}"
    );
    // Each mutant no test reaches can be proposed from what's shown: its id is there.
    assert!(
        out.contains("  or propose: mutastic ignore ") || out.contains("  or propose (line "),
        "{out}"
    );

    // A test ignored holds the gate.
    let tests = dir.join("tests/pricing.rs");
    let kept = std::fs::read_to_string(&tests).unwrap();
    std::fs::write(&tests, kept.replacen("#[test]", "#[test]\n#[ignore]", 1)).unwrap();
    let (code, out) = mutastic_in(&dir, &["gate", "check"]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("mutastic gate: fewer tests run than when the task started"),
        "{out}"
    );
    std::fs::write(&tests, kept).unwrap();

    // An ignore the task made itself is a person's call.
    let (_, ids) = mutastic_in(&dir, &["mutants"]);
    let id = ids
        .lines()
        .find(|line| line.contains(" in double"))
        .and_then(|line| line.split_whitespace().next())
        .unwrap()
        .to_string();
    mutastic_in(&dir, &["ignore", &id, "--reason=doubling is obvious"]);
    let (code, out) = mutastic_in(&dir, &["gate", "check"]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("itself, and whether a mutant is worth a test is a person's call"),
        "{out}"
    );
    mutastic_in(&dir, &["ignore", "--remove", &id]);

    // The test written: done.
    std::fs::write(
        dir.join("tests/double.rs"),
        "#[test]\nfn doubles() {\n    assert_eq!(pricing::double(3), 6);\n}\n",
    )
    .unwrap();
    let (code, out) = mutastic_in(&dir, &["gate", "check"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("mutastic gate: passed: the tests catch "),
        "{out}"
    );
    let (_, out) = mutastic_in(&dir, &["gate", "close"]);
    assert!(out.contains("mutastic gate: closed\n"), "{out}");
    std::fs::remove_dir_all(&dir).ok();
}

/// The mutants of one line, from `mutastic mutants`: id, then where it is, as `src/lib.rs:6:5`.
fn mutants_where(dir: &Path) -> Vec<(String, String)> {
    let (code, out) = mutastic_in(dir, &["mutants"]);
    assert_eq!(code, 0, "{out}");
    out.lines()
        .filter(|line| line.starts_with('m') && line.contains(" in four_steps"))
        .map(|line| {
            let mut words = line.split_whitespace();
            let id = words.next().unwrap().to_string();
            (id, words.next().unwrap().to_string())
        })
        .collect()
}

/// Four identical `i = i + 1;` lines: each has ids of its own, its own card in every format, and
/// `verify` asks only of its own line. Ids come from a site's place in its function, counted from
/// the top, so lines added above move none, while deleting one passes its ids to the lines below.
#[test]
fn identical_lines_have_ids_of_their_own() {
    let dir = copy_of("repeats", "repeats");
    let steps = [6, 8, 10, 12];
    let found = mutants_where(&dir);
    let ids: std::collections::BTreeSet<&String> = found.iter().map(|(id, _)| id).collect();
    assert_eq!(ids.len(), found.len(), "every id is distinct: {found:?}");
    let on = |line: u32| -> Vec<String> {
        let at = [
            format!("src/lib.rs:{line}:5"),
            format!("src/lib.rs:{line}:11"),
        ];
        let here: Vec<String> = found
            .iter()
            .filter(|(_, place)| at.contains(place))
            .map(|(id, _)| id.clone())
            .collect();
        assert_eq!(
            here.len(),
            2,
            "removed, and + becomes -, on line {line}: {found:?}"
        );
        here
    };
    let by_step: Vec<Vec<String>> = steps.iter().map(|&line| on(line)).collect();

    // One card per line, in each format, naming that line's ids and no other's.
    let (_, full) = mutastic_in(&dir, &["run", "--format=full", "--report=none"]);
    let (_, agent) = mutastic_in(&dir, &["run", "--format=agent", "--report=none"]);
    for (line, here) in steps.iter().zip(&by_step) {
        let done = format!("verify {}", here.join(","));
        assert_eq!(
            full.matches(&format!("─ src/lib.rs:{line}\n")).count(),
            1,
            "{full}"
        );
        assert_eq!(full.matches(&done).count(), 1, "{full}");
        assert_eq!(
            agent.matches(&format!("   at src/lib.rs:{line}\n")).count(),
            1,
            "{agent}"
        );
        assert_eq!(agent.matches(&done).count(), 1, "{agent}");
    }
    let (_, jsonl) = mutastic_in(&dir, &["run", "--format=jsonl", "--report=none"]);
    let finished = events(&jsonl);
    for (line, here) in steps.iter().zip(&by_step) {
        let mut missed: Vec<&str> = finished
            .iter()
            .filter(|e| e["reason"] == "mutant-finished" && e["location"]["line"] == *line)
            .inspect(|e| assert_eq!(e["status"], "Survived", "{e}"))
            .map(|e| e["id"].as_str().unwrap())
            .collect();
        missed.sort_unstable();
        let mut expected: Vec<&str> = here.iter().map(String::as_str).collect();
        expected.sort_unstable();
        assert_eq!(missed, expected, "line {line}: {jsonl}");
    }

    // A test that tells only the third step's change verifies its ids, and no other line's.
    let lib = dir.join("src/lib.rs");
    let text = std::fs::read_to_string(&lib).unwrap();
    let third_step_test = "    #[test]\n    fn the_third_step_is_one() {\n        let mut seen = Vec::new();\n        four_steps(0, &mut seen);\n        assert_eq!(seen[2] - seen[1], 1);\n    }\n}\n";
    let tested = format!(
        "{}{third_step_test}",
        text.trim_end().strip_suffix('}').unwrap()
    );
    std::fs::write(&lib, &tested).unwrap();
    let (code, out) = mutastic_in(&dir, &["verify", &by_step[2].join(",")]);
    assert_eq!(code, 0, "{out}");
    for (step, here) in by_step.iter().enumerate().filter(|(step, _)| *step != 2) {
        let (code, out) = mutastic_in(&dir, &["verify", &here.join(",")]);
        assert_eq!(code, 1, "step {}: {out}", step + 1);
        assert!(
            out.contains(&format!("{} is still missed", here[0])),
            "{out}"
        );
    }
    std::fs::write(&lib, &text).unwrap();

    // Lines added above move no id.
    std::fs::write(
        &lib,
        text.replacen(
            "    let mut i = start;\n",
            "    // a comment\n\n    let mut i = start;\n",
            1,
        )
        .replacen("pub fn four_steps", "/// More words.\npub fn four_steps", 1),
    )
    .unwrap();
    let moved: Vec<(String, String)> = mutants_where(&dir);
    let unmoved: Vec<&String> = moved.iter().map(|(id, _)| id).collect();
    assert_eq!(unmoved, found.iter().map(|(id, _)| id).collect::<Vec<_>>());
    assert!(
        moved.iter().any(|(_, place)| place == "src/lib.rs:9:5"),
        "{moved:?}"
    );

    // Deleting the second step: the steps below take the earlier ids, and the last ids go.
    let deleted = text.replacen(
        "    seen.push(i);\n    i = i + 1;\n",
        "    seen.push(i);\n",
        1,
    );
    assert_eq!(deleted.matches("i = i + 1;").count(), 3, "{deleted}");
    std::fs::write(&lib, &deleted).unwrap();
    let shifted = mutants_where(&dir);
    let ids_on = |line: u32| -> Vec<&String> {
        shifted
            .iter()
            .filter(|(_, place)| place.starts_with(&format!("src/lib.rs:{line}:")))
            .map(|(id, _)| id)
            .collect()
    };
    assert_eq!(ids_on(6), by_step[0].iter().collect::<Vec<_>>());
    assert_eq!(
        ids_on(9),
        by_step[1].iter().collect::<Vec<_>>(),
        "the third step takes the second's ids"
    );
    assert_eq!(
        ids_on(11),
        by_step[2].iter().collect::<Vec<_>>(),
        "the fourth takes the third's"
    );
    assert!(
        shifted.iter().all(|(id, _)| !by_step[3].contains(id)),
        "the last step's ids are gone: {shifted:?}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// A test that fails, then passes when run again on its own, is flaky: the run goes on without it
/// and says so, in each format, in the Java edition's words, and what only it reached is not
/// covered. A test that fails every time stops the run, and nothing says to turn it off.
#[test]
fn a_flaky_test_is_left_out_and_one_that_always_fails_stops_the_run() {
    let dir = copy_of("flaky", "flaky");
    let test = "raffle::a_draw_is_a_place_in_the_raffle";
    let (code, out) = mutastic_in(&dir, &["run", "--format=full", "--report=none"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(" (1 flaky, left out) · "), "{out}");
    assert!(
        out.contains(&format!(
            " ! flaky: {test} failed, then passed when run again; left out of this run; it reaches SystemTime::now\n"
        )),
        "{out}"
    );
    assert!(out.contains("  Flaky  1 test left out\n"), "{out}");
    assert!(
        out.contains("(no test reaches these lines, or only a flaky one does)"),
        "{out}"
    );

    let (_, agent) = mutastic_in(&dir, &["run", "--format=agent", "--report=none"]);
    assert!(
        agent.contains(&format!(
            "flaky: {test} failed, then passed when run again; left out of this run, so its catches don't count. The code it runs reaches SystemTime::now, which answers differently every run: making that the same every time (a seed, a fixed clock, through the code's own seam) is likely safe, so long as the test still checks what it did. Never remove, disable or loosen it to get a run through\n"
        )),
        "{agent}"
    );
    assert!(!agent.contains("ignore]"), "{agent}");

    let (_, jsonl) = mutastic_in(&dir, &["run", "--format=jsonl", "--report=none"]);
    let found = events(&jsonl);
    let validator = schema();
    for event in &found {
        assert!(validator.is_valid(event), "{event}");
    }
    let finished = found.last().unwrap();
    assert_eq!(finished["flaky"], serde_json::json!([test]));
    assert_eq!(
        finished["flakyEvidence"],
        serde_json::json!([{
            "test": test,
            "failure": [],
            "reaches": ["SystemTime::now"],
            "reachesInTest": [],
            "outside": false,
        }])
    );
    // Its catches don't count: what only it reaches is not covered.
    let in_draw: Vec<&Value> = found
        .iter()
        .filter(|e| e["location"]["method"] == "draw")
        .collect();
    assert!(!in_draw.is_empty());
    assert!(
        in_draw.iter().all(|e| e["status"] == "NoCoverage"),
        "{jsonl}"
    );

    let tests = dir.join("tests/raffle.rs");
    let text = std::fs::read_to_string(&tests).unwrap();
    std::fs::write(
        &tests,
        text.replace(
            "    first_try_fails();\n}",
            "    panic!(\"fails every time\");\n}",
        ),
    )
    .unwrap();
    let (code, out) = mutastic_in(&dir, &["run", "--report=none"]);
    assert_eq!(code, 2, "{out}");
    assert!(
        out.contains(&format!(
            "mutastic: the tests fail without any mutant: {test}; their output is in target/mutastic/tests.log\n"
        )),
        "{out}"
    );
    assert!(!out.contains("ignore"), "{out}");
    let log = std::fs::read_to_string(dir.join("target/mutastic/tests.log")).unwrap();
    assert!(log.contains("fails every time"), "{log}");
    std::fs::remove_dir_all(&dir).ok();
}

/// Each full run is compared with the last under the same operators, as in the Java edition: its
/// score, whether the code and mutastic are the same, and which mutants changed result. A run of
/// one mutant, or of a change, says nothing about the whole package, so it neither compares nor is
/// kept.
#[test]
fn a_full_run_compares_with_the_last_and_is_kept_for_the_next() {
    let copy = copy_of("pricing", "last-run");
    let dir = copy.to_str().unwrap();
    // A repository, for the run of a change.
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(&copy)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap()
            .status;
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    std::fs::write(copy.join(".gitignore"), "target\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "start"]);
    let finished = |args: &[&str]| -> Value {
        let (_, out) = mutastic_in(&copy, args);
        let events = events(
            out.lines()
                .filter(|l| l.starts_with('{'))
                .collect::<Vec<_>>()
                .join("\n")
                .as_str(),
        );
        let finished = events
            .last()
            .unwrap_or_else(|| panic!("no events: {out}"))
            .clone();
        let errors: Vec<String> = schema()
            .iter_errors(&finished)
            .map(|e| e.to_string())
            .collect();
        assert!(errors.is_empty(), "{errors:?}");
        finished
    };

    let first = finished(&["run", dir, "--format=jsonl"]);
    assert!(
        first.get("previousScore").is_none(),
        "nothing to compare with: {first}"
    );
    let again = finished(&["run", dir, "--format=jsonl"]);
    assert_eq!(again["previousScore"], first["score"]);
    assert_eq!(again["sameCodeAsLastRun"], true);
    assert_eq!(again["sameMutasticAsLastRun"], true);
    assert_eq!(again["changedSinceLastRun"], serde_json::json!([]));
    assert_eq!(again["likelyFlaky"], serde_json::json!([]));

    let kept = std::fs::read_to_string(copy.join("target/mutastic/last-run.tsv")).unwrap();
    let only = finished(&["run", dir, "--format=jsonl", "--only=mb2f8ae57"]);
    assert!(only.get("previousScore").is_none(), "{only}");
    let since = finished(&["run", dir, "--format=jsonl", "--since=HEAD"]);
    assert!(since.get("previousScore").is_none(), "{since}");
    assert_eq!(
        std::fs::read_to_string(copy.join("target/mutastic/last-run.tsv")).unwrap(),
        kept,
        "a run of one mutant, or of a change, isn't kept"
    );

    let lib = copy.join("src/lib.rs");
    let source = std::fs::read_to_string(&lib).unwrap();
    std::fs::write(&lib, source + "\n// changed\n").unwrap();
    let changed = finished(&["run", dir, "--format=jsonl"]);
    assert_eq!(changed["sameCodeAsLastRun"], false);
    let (_, agent) = mutastic_in(&copy, &["run", dir, "--format=agent"]);
    assert!(
        agent.contains("vs last run: 90.9% → 90.9% (same code)\n"),
        "{agent}"
    );
    std::fs::remove_dir_all(&copy).ok();
}

/// `--report=sarif`: SARIF 2.1.0 the schema accepts, a result for the missed mutant on its line
/// with its id, and paths from the repository's root, a folder above the package here.
#[test]
fn the_sarif_report_is_a_result_for_each_miss_keyed_by_its_id() {
    let repository = copy_of("pricing", "sarif");
    let package = repository.join("shop");
    std::fs::create_dir_all(&package).unwrap();
    for name in ["Cargo.toml", "src", "tests"] {
        if repository.join(name).exists() {
            std::fs::rename(repository.join(name), package.join(name)).unwrap();
        }
    }
    std::fs::create_dir_all(repository.join(".git")).unwrap();
    let (code, out) = mutastic_in(
        &repository,
        &["run", "shop", "--report=sarif", "--format=concise"],
    );
    assert_eq!(code, 0, "{out}");
    let sarif: Value = serde_json::from_str(
        &std::fs::read_to_string(package.join("target/mutastic/mutation.sarif")).unwrap(),
    )
    .unwrap();
    let schema: Value = serde_json::from_str(include_str!("schema/sarif-2.1.0.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<String> = validator
        .iter_errors(&sarif)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    assert!(
        !validator.is_valid(&serde_json::json!({ "version": "2.0.0", "runs": [{}] })),
        "the schema refuses what isn't SARIF 2.1.0"
    );

    let run = &sarif["runs"][0];
    let results = run["results"].as_array().unwrap();
    assert_eq!(results.len(), 1, "{results:?}");
    let missed = &results[0];
    assert_eq!(missed["level"], "warning");
    assert_eq!(missed["ruleId"], "mutastic/boundary");
    assert_eq!(
        run["tool"]["driver"]["rules"][missed["ruleIndex"].as_u64().unwrap() as usize]["id"],
        "mutastic/boundary"
    );
    assert_eq!(missed["partialFingerprints"]["mutasticId/v1"], "mb2f8ae57");
    assert!(missed.get("fixes").is_none());
    assert_eq!(
        missed["message"]["text"],
        "no test checks discount() at total == 100"
    );
    let location = &missed["locations"][0]["physicalLocation"];
    assert_eq!(location["artifactLocation"]["uri"], "shop/src/lib.rs");
    assert_eq!(
        location["region"],
        serde_json::json!({ "startLine": 26, "startColumn": 18, "endColumn": 20 })
    );
    std::fs::remove_dir_all(&repository).ok();
}

/// `--vocabulary`, `--color` and `--hyperlinks`, as the Java edition takes them: the classic words
/// in what a person or an agent reads and never in the events, colour and links forced on or off,
/// and a value they don't take refused.
#[test]
fn the_words_colour_and_links_are_chosen_as_the_java_edition_chooses_them() {
    let dir = fixture("pricing");
    let (_, concise) = mutastic(&["run", &dir, "--format=concise", "--vocabulary=classic"]);
    for words in [
        "Survivors",
        "10 killed",
        "1 survived",
        "with survivors",
        "1 survived, above",
    ] {
        assert!(concise.contains(words), "{words}: {concise}");
    }
    let (_, agent) = mutastic(&["run", &dir, "--format=agent", "--vocabulary=classic"]);
    assert!(
        agent.contains("1 survived, 0 no coverage, 10 killed of 11 mutants"),
        "{agent}"
    );
    assert!(agent.contains("\n1. survived mb2f8ae57:"), "{agent}");
    let (_, jsonl) = mutastic(&["run", &dir, "--format=jsonl", "--vocabulary=classic"]);
    assert!(jsonl.contains("\"status\":\"Survived\""), "{jsonl}");
    assert!(
        jsonl.contains("missed mb2f8ae57:"),
        "the events keep the standard words"
    );

    let (_, coloured) = mutastic(&["run", &dir, "--format=concise", "--color=always"]);
    assert!(coloured.contains("\x1b["), "{coloured}");
    let (_, linked) = mutastic(&["run", &dir, "--format=concise", "--hyperlinks=always"]);
    assert!(linked.contains("\x1b]8;;file://"), "{linked}");
    assert!(linked.contains("src/lib.rs:26\x1b]8;;\x1b\\"), "{linked}");
    let (_, plain) = mutastic(&["run", &dir, "--format=concise", "--hyperlinks=never"]);
    assert!(!plain.contains("\x1b]8;;"), "{plain}");
    // The full card's change mark opens its operator's section in the docs, and the event says
    // where that is.
    let boundary = "https://docs.mutastic.io/rust/operators/conditions/#boundary";
    let (_, full) = mutastic(&["run", &dir, "--format=full", "--hyperlinks=always"]);
    assert!(
        full.contains(&format!("\x1b]8;;{boundary}\x1b\\^")),
        "{full}"
    );
    assert!(
        jsonl.contains(&format!(
            "\"operator\":\"mutastic/boundary\",\"docs\":\"{boundary}\""
        )),
        "{jsonl}"
    );

    for (option, value) in [
        ("--vocabulary", "loud"),
        ("--color", "sometimes"),
        ("--hyperlinks", "yes"),
    ] {
        let refused = Command::new(env!("CARGO_BIN_EXE_mutastic"))
            .args(["run", &dir, &format!("{option}={value}")])
            .output()
            .unwrap();
        let said = String::from_utf8_lossy(&refused.stderr);
        assert_eq!(refused.status.code(), Some(2), "{said}");
        assert!(
            said.contains(&format!("{option}: {value} isn't one of")),
            "{said}"
        );
    }
}

/// A function only the crate calls gets its inputs from its callers: a suggested test that calls it
/// alone on a value no caller passed may test what can't happen, as `fee(-1)` would here, where
/// `repay` only passes a `u8`. Its card offers no such test: it says no caller passed the value,
/// and to test a caller that can, or propose the mutant unreachable. `pub(crate)`, so a test can
/// call it, as the crate's own tests could.
#[test]
fn a_private_function_gets_no_test_on_an_input_its_callers_never_gave() {
    let copy = copy_of("suggestions", "beyond-callers");
    let lib = copy.join("src/lib.rs");
    let source = std::fs::read_to_string(&lib).unwrap();
    std::fs::write(
        &lib,
        source
            + r#"
pub mod repayments {
    /// What a repayment costs: the balance and a fee for the days it took, which only `repay`
    /// asks for, with whole days.
    pub fn repay(balance: i64, days: u8) -> i64 {
        balance + fee(days as i64)
    }

    pub(crate) fn fee(days: i64) -> i64 {
        if days < 0 {
            return 100;
        }
        days * 3
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_week_costs_more_than_three_days() {
            assert_eq!(repay(100, 7), 121);
            assert_eq!(repay(100, 3), 109);
        }
    }
}
"#,
    )
    .unwrap();
    let (_, agent) = mutastic_in(&copy, &["run", "--format=agent", "--operators=stronger"]);
    let entry = agent
        .split("\n\n")
        .find(|entry| entry.contains("missed m") && entry.contains("`fee()`"))
        .unwrap_or_else(|| panic!("{agent}"));
    assert!(!entry.contains("test to add"), "{entry}");
    assert!(
        entry.contains(
            "no caller of `fee()` passed `days = 0`, which a test of it alone would need"
        ),
        "{entry}"
    );
    assert!(
        entry.contains(
            "fix: if a caller can pass `days = 0`, test that caller with it; if none can, the \
             mutant can't happen: propose ignoring it as unreachable"
        ),
        "{entry}"
    );
    // A function anyone can call keeps its suggested test.
    assert!(
        agent.contains("test to add in mod rates::tests:"),
        "{agent}"
    );
    std::fs::remove_dir_all(&copy).ok();
}

/// A closed stderr (`mutastic run … 2>&1 | head`) loses the progress, never a mutant's result: a
/// run whose progress can't be written keeps every mutant it had, as one that can.
#[test]
fn a_closed_stderr_loses_no_results() {
    let copy = copy_of("pricing", "closed-stderr");
    let kept = copy.join("target/mutastic/last-run.tsv");
    let run = |stderr: Stdio| {
        std::fs::remove_file(&kept).ok();
        let mut child = Command::new(env!("CARGO_BIN_EXE_mutastic"))
            .args(["run", copy.to_str().unwrap(), "--progress=counter"])
            .env("NO_COLOR", "1")
            .env_remove("MUTASTIC_FORMAT")
            .env_remove("MUTASTIC_AGENT")
            .stdout(Stdio::null())
            .stderr(stderr)
            .spawn()
            .unwrap();
        // Closed before the run writes anything to it.
        drop(child.stderr.take());
        let status = child.wait().unwrap().code();
        let record = std::fs::read_to_string(&kept).unwrap_or_default();
        (status, record.lines().count())
    };
    let open = run(Stdio::null());
    let closed = run(Stdio::piped());
    assert!(open.1 > 1, "the open run kept a record: {open:?}");
    assert_eq!(
        closed, open,
        "(exit, record lines) with stderr closed and open"
    );
    std::fs::remove_dir_all(&copy).ok();
}

/// `next`, worked to the end on the ranking fixture: one item at a time in the run's order, exit 1
/// while something is queued; a rerun of only what's still uncaught while the code under test is
/// the same, a new test in its `mod tests` included; every mutant again once that code changes; a
/// mutant proposed for ignoring waits on a person, not in the queue; and exit 0, with nothing run,
/// once nothing missed is left.
#[test]
fn next_works_through_the_misses_one_at_a_time() {
    let copy = copy_of("ranking", "next");
    let next = |args: &[&str]| mutastic_in(&copy, &[&["next"], args].concat());
    let add_test = |test: &str| {
        let lib = copy.join("src/lib.rs");
        let source = std::fs::read_to_string(&lib).unwrap();
        let end = source.rfind('}').unwrap();
        std::fs::write(&lib, format!("{}{test}}}\n", &source[..end])).unwrap();
    };
    // How many mutants the run ran, from its coverage line; none when nothing ran.
    let ran = |out: &str| {
        out.lines()
            .find_map(|line| line.strip_prefix("coverage  "))
            .and_then(|line| line.rsplit(" · ").next())
            .map(str::to_string)
    };
    let jsonl = |out: &str| -> Vec<Value> {
        let lines: Vec<&str> = out.lines().filter(|l| l.starts_with('{')).collect();
        let events = events(&lines.join("\n"));
        for event in &events {
            let errors: Vec<String> = schema().iter_errors(event).map(|e| e.to_string()).collect();
            assert!(errors.is_empty(), "{event}\n{}", errors.join("\n"));
        }
        events
    };
    let ids_of = |event: &Value| -> Vec<String> {
        event["ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap().to_string())
            .collect()
    };

    // The head of the queue: is_small's condition, which three tests run.
    let (code, out) = next(&["--format=jsonl"]);
    assert_eq!(code, 1, "{out}");
    let events = jsonl(&out);
    assert_eq!(events.len(), 2, "{out}");
    let head = &events[0];
    assert_eq!(head["reason"], "next-item");
    assert_eq!(head["location"]["line"], 15);
    let ids = ids_of(head);
    assert_eq!(ids.len(), 3, "{head}");
    assert_eq!(head["ids"][0], head["id"], "the lead first");
    assert_eq!(head["rank"]["position"], 1);
    assert_eq!(head["rank"]["order"], "run-1");
    let values: Vec<&Value> = head["rank"]["factors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|factor| &factor["value"])
        .collect();
    assert_eq!(
        values,
        [
            &serde_json::json!("condition-or-removal"),
            &serde_json::json!(3)
        ]
    );
    assert_eq!(
        events[1],
        serde_json::json!({"schema": "mutastic.events/v1", "edition": "rust", "reason": "next-finished", "queued": 3, "mutants": 7, "shown": 1, "legacy": 0, "waiting": 0})
    );
    // That was a run of every mutant, so explain has a last run to say.
    let (_, explained) = mutastic_in(&copy, &["explain", &ids[0]]);
    assert!(
        explained.contains("last run  missed: every covering test still passed"),
        "{explained}"
    );

    // Again, with the code as it was: only the seven left uncaught run. --top shows two, as two is
    // the most from one file.
    let (code, out) = next(&["--top=3", "--format=concise", "--progress=counter"]);
    assert_eq!(code, 1, "{out}");
    assert_eq!(ran(&out).as_deref(), Some("7 mutants"), "{out}");
    assert!(
        out.contains("Next: the first 2 of 3 lines with missed mutants in ranking\n"),
        "{out}"
    );
    let places: Vec<&str> = out
        .lines()
        .filter_map(|line| line.trim().strip_prefix("┌─ "))
        .collect();
    assert_eq!(places, ["src/lib.rs:15", "src/lib.rs:11"], "{out}");
    assert!(
        out.contains(
            "\n Ranked first because\n   a condition or a removal: it changes what the code \
             decides or does\n   3 tests run it, and none checks it\n"
        ),
        "{out}"
    );
    assert!(out.contains("\n Ranked 2nd because\n"), "{out}");
    assert!(
        out.contains("\nThen: mutastic next ., for the next one\n"),
        "{out}"
    );

    // For an agent, one task, whole, and every command it offers parses.
    let (code, agent) = next(&["--format=agent"]);
    assert_eq!(code, 1, "{agent}");
    assert!(
        agent.starts_with(
            "mutastic next: 1 of 3 lines with missed mutants in ranking; close this one, then ask \
             for the next\nwhy first: a condition or a removal: it changes what the code decides \
             or does; 3 tests run it, and none checks it\n"
        ),
        "{agent}"
    );
    let done_when = format!("mutastic verify {} .", ids.join(","));
    assert!(
        agent.contains(&format!(
            "\nrules: {}, and never delete, disable or loosen a test; it's done when \
             `{done_when}` passes\ndecline: {} (`mutastic ignore <id> --propose",
            mutastic::report::RULE,
            mutastic::report::DECLINE
        )),
        "{agent}"
    );
    let tasks = agent
        .lines()
        .filter(|line| line.starts_with("missed "))
        .count();
    assert_eq!(tasks, 1, "one task: {agent}");
    assert!(
        agent.ends_with(
            "\nthen: run `mutastic next .` again for the next one; when it says nothing is left, \
             you're done\n"
        ),
        "{agent}"
    );
    for command in [done_when.as_str(), "mutastic next ."] {
        let words: Vec<String> = command.split(' ').skip(1).map(str::to_string).collect();
        assert_eq!(mutastic::cli::check(&words), Ok(()), "{command}");
    }

    // A test for is_small, in mod tests: the code under test is the same, so again only the seven
    // run, and its line drops out.
    add_test(
        "    #[test]\n    fn ten_isnt_small() {\n        assert!(!is_small(10));\n        \
         assert!(is_small(9));\n    }\n",
    );
    let (code, out) = next(&["--top=25", "--format=jsonl"]);
    assert_eq!(code, 1, "{out}");
    let events = jsonl(&out);
    let lines: Vec<&Value> = events
        .iter()
        .filter(|e| e["reason"] == "next-item")
        .map(|e| &e["location"]["line"])
        .collect();
    assert_eq!(
        lines,
        [&serde_json::json!(11), &serde_json::json!(6)],
        "{out}"
    );
    let doubled = ids_of(&events[1]);

    // doubled's mutants, proposed for ignoring, wait on a person, out of the queue.
    for id in &doubled {
        let (code, out) = mutastic_in(&copy, &["ignore", id, "--propose", "--reason=only doubles"]);
        assert_eq!(code, 0, "{out}");
    }
    let waiting = if doubled.len() == 1 {
        "1 mutant proposed for ignoring, waiting on a person to accept it \
         (.mutastic/proposals.yaml)"
            .to_string()
    } else {
        format!(
            "{} mutants proposed for ignoring, waiting on a person to accept them \
             (.mutastic/proposals.yaml)",
            doubled.len()
        )
    };
    let (code, out) = next(&["--format=concise", "--progress=counter"]);
    assert_eq!(code, 1, "{out}");
    let rerun = format!("{} mutants", 3 + doubled.len());
    assert_eq!(ran(&out), Some(rerun), "{out}");
    assert!(
        out.contains(&format!(
            "Next: 1 of 1 line with missed mutants in ranking; {waiting}\n"
        )),
        "{out}"
    );

    // A test for is_big leaves only the proposals: nothing is queued, so next exits 0.
    add_test(
        "    #[test]\n    fn a_hundred_is_big() {\n        assert!(is_big(100));\n        \
         assert!(!is_big(99));\n    }\n",
    );
    let nothing_but_waiting = format!("Next: nothing missed is left in ranking but {waiting}\n");
    let (code, out) = next(&["--format=concise"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.starts_with(&nothing_but_waiting), "{out}");
    let (code, out) = next(&["--format=jsonl"]);
    assert_eq!(code, 0, "{out}");
    let events = jsonl(&out);
    assert_eq!(events.len(), 1, "{out}");
    assert_eq!(events[0]["queued"], 0);
    assert_eq!(events[0]["waiting"], doubled.len());

    // The code under test changes: every mutant runs again.
    let lib = copy.join("src/lib.rs");
    let source = std::fs::read_to_string(&lib).unwrap();
    let changed = source.replacen(
        "#[cfg(test)]",
        "pub const UNUSED: u32 = 7;\n\n#[cfg(test)]",
        1,
    );
    std::fs::write(&lib, changed).unwrap();
    let (code, out) = next(&["--format=concise", "--progress=counter"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(ran(&out).as_deref(), Some("12 mutants"), "{out}");
    assert!(out.starts_with(&nothing_but_waiting), "{out}");

    // Withdrawn, the proposals' mutants are queued again; a test for them, and nothing is left.
    for id in &doubled {
        let (code, out) = mutastic_in(&copy, &["ignore", "--remove", id]);
        assert_eq!(code, 0, "{out}");
    }
    let (code, out) = next(&["--format=concise"]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("Next: 1 of 1 line with missed mutants in ranking\n"),
        "{out}"
    );
    add_test(
        "    #[test]\n    fn three_doubled_is_six() {\n        assert_eq!(doubled(3), 6);\n    }\n",
    );
    let (code, out) = next(&["--format=concise"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.starts_with("Next: nothing missed is left in ranking\n"),
        "{out}"
    );
    // Nothing was left uncaught, and the code is the same: nothing runs.
    let (code, out) = next(&["--format=concise", "--progress=counter"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(ran(&out), None, "{out}");
    assert!(
        out.starts_with("Next: nothing missed is left in ranking\n"),
        "{out}"
    );
    let (code, agent) = next(&["--format=agent"]);
    assert_eq!(code, 0, "{agent}");
    assert_eq!(agent, "mutastic next: nothing missed is left in ranking\n");
    std::fs::remove_dir_all(&copy).ok();
}

/// Two mutants of one line with a test each, as the test-first guide's loyalty credit has: one fix
/// can't describe both tests, so each comes after its own mutant's fix and id, in the card and the
/// agent format. Once, the second test came with nothing saying why.
#[test]
fn each_suggested_test_comes_after_its_own_mutants_fix() {
    let dir = copy_of("pricing", "each-fix");
    let lib = dir.join("src/lib.rs");
    let text = std::fs::read_to_string(&lib).unwrap();
    let credit = "    /// 5 off orders of 30 or more, for loyal customers.
    pub fn loyalty_credit(&self, total: u32) -> u32 {
        if total >= 30 {
            return 5;
        }
        0
    }

    /// Free from the threshold up.";
    let test = "#[cfg(test)]
mod loyalty {
    use super::*;

    #[test]
    fn loyal_customers_get_five_off() {
        assert_eq!(Pricing::new().loyalty_credit(50), 5);
    }
}

#[cfg(test)]";
    let text = text
        .replacen("    /// Free from the threshold up.", credit, 1)
        .replacen("#[cfg(test)]", test, 1);
    std::fs::write(&lib, text).unwrap();

    let (_, card) = mutastic_in(&dir, &["run", "--format=full"]);
    let (_, agent) = mutastic_in(&dir, &["run", "--format=agent"]);
    std::fs::remove_dir_all(&dir).ok();

    let card = card[card.find("loyalty_credit").expect(&card)..].to_string();
    assert!(card.contains("help: one test for each mutant"), "{card}");
    let first = card.find("with total = 30").expect(&card);
    let second = card.find("is false").expect(&card);
    let tests: Vec<usize> = card
        .match_indices("loyalty_credit(")
        .map(|(at, _)| at)
        .collect();
    assert!(
        tests.iter().any(|&at| at > first && at < second) && tests.iter().any(|&at| at > second),
        "{card}"
    );
    assert!(
        agent.contains("   fix: one test for each mutant"),
        "{agent}"
    );
    assert!(agent.contains("   fix for m"), "{agent}");
}

/// Two runs on one package at once each get the whole run's result: the second waits for the
/// first, which holds the package's work directory, rather than rewriting its copy and build in the
/// middle of it. Once, the samples' own tests ran on the pricing fixture side by side and got each
/// other's mutants.
#[test]
fn two_runs_at_once_each_get_the_whole_result() {
    let dir = copy_of("pricing", "at-once");
    let start = || {
        Command::new(env!("CARGO_BIN_EXE_mutastic"))
            .args(["run", "--format=jsonl"])
            .current_dir(&dir)
            .env_remove("MUTASTIC_FORMAT")
            .env_remove("MUTASTIC_AGENT")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    };
    let (first, second) = (start(), start());
    let outputs = [
        first.wait_with_output().unwrap(),
        second.wait_with_output().unwrap(),
    ];
    std::fs::remove_dir_all(&dir).ok();
    for output in &outputs {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let finished: Vec<Value> = events(&stdout)
            .into_iter()
            .filter(|e| e["reason"] == "mutant-finished")
            .collect();
        let missed = finished
            .iter()
            .filter(|e| e["status"] == "Survived")
            .count();
        assert_eq!(
            (finished.len(), missed),
            (11, 1),
            "{stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
