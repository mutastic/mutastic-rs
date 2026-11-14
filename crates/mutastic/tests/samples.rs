//! `mutastic run` on the samples, through the binary as a person runs it.

#![allow(
    clippy::unwrap_used,
    reason = "a helper here asserts as the tests it serves do: a value that isn't there fails the test"
)]

use std::io::BufRead as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

mod common;
use common::schema;

/// A fixture to run as it's checked in, in a copy this test process has to itself.
fn fixture(name: &str) -> String {
    common::own_copy(name, &format!("samples-{name}"))
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
    let started = events
        .iter()
        .find(|e| e["reason"] == "run-started")
        .unwrap();
    assert_eq!(started["rule"], mutastic::report::RULE);
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
    let (_, listed) = mutastic(&["mutants", &dir]);
    // Not the two that make `in_while` count to `i32::MAX` before it overflows, ten seconds each:
    // neither is missed, so neither has a card, and every mutant of mutastic this test reaches
    // would wait for them.
    let endless = ["m417a9a25", "m7539bd4b"];
    let mutants: Vec<&str> = listed
        .lines()
        .filter_map(|line| line.split(' ').next())
        .filter(|word| word.len() == 9 && word.starts_with('m'))
        .collect();
    assert!(
        endless.iter().all(|id| mutants.contains(id)),
        "both are still the fixture's: {listed}"
    );
    let ids: Vec<&str> = mutants
        .into_iter()
        .filter(|id| !endless.contains(id))
        .collect();
    let only = format!("--only={}", ids.join(","));
    let (_, cards) = mutastic(&["run", &dir, "--format=full", &only]);
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
    let started = events
        .iter()
        .find(|e| e["reason"] == "run-started")
        .unwrap();
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
        .env_remove("FORCE_COLOR")
        // Whatever terminal runs the tests, no progress in its tab: that's tested on its own.
        .env("MUTASTIC_TERM_PROGRESS", "false");
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
    // The score is the last line, with its verdict: PASS on green, and why in green. The fixture
    // runs in place, so a run before this one may have left a last run to compare with.
    let score = regex::Regex::new(
        r" \x1b\[2m    Score\x1b\[0m  \x1b\[32m\x1b\[1m90\.9%\x1b\[0m\x1b\[0m  \x1b\[42;30;1m PASS \x1b\[0m \x1b\[32m1 missed, above the threshold of 80\.\x1b\[0m(?<delta>\x1b\[2m  = vs last run\x1b\[0m\x1b\[2m \((?<sameness>[a-z ]+)\)\x1b\[0m)?\n$",
    )
    .unwrap();
    assert!(
        score.is_match(&out),
        "a passing score is bold green, its verdict PASS on green, last: {out:?}"
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

/// At a terminal, the phase's line is a live block redrawn in place, with wrapping off while it's
/// drawn: each permanent line clears it first and draws it again beneath, and it's cleared before
/// the result.
#[test]
fn the_live_line_is_redrawn_in_place() {
    // A copy of its own: runs of one package side by side share its build in target/mutastic.
    let copy = copy_of("pricing", "counter");
    let dir = copy.display().to_string();
    let (_, err) = run_with(&dir, "--progress=bar", false);
    // Each frame is one synchronized write; inside them, what the terminal is told.
    let (opened, closed) = (
        err.matches("\x1b[?2026h").count(),
        err.matches("\x1b[?2026l").count(),
    );
    assert!(opened > 0 && opened == closed, "{err:?}");
    let err = err.replace("\x1b[?2026h", "").replace("\x1b[?2026l", "");
    let clear = "\r\x1b[2K";
    // Piped, it can't ask the terminal its width, so it takes 100 columns: the bar, the
    // weighted percent, then the fates in as many words as fit.
    let live = regex::Regex::new(
        r"\x1b\[\?7l[⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏] mutants   [━─]+  \d+% · \d+/11 · ✓ \d+ caught · ✗ \d missed · ",
    )
    .unwrap();
    assert!(live.is_match(&err), "the mutants' live line: {err:?}");
    // A block of any height: the line it's on, then each line above.
    let cleared_then = |permanent: &str| {
        regex::Regex::new(&format!(
            r"\r\x1b\[2K(\x1b\[1A\x1b\[2K)*{}",
            regex::escape(permanent)
        ))
        .unwrap()
        .is_match(&err)
    };
    for permanent in [
        " ❯ [pricing] src/lib.rs",
        "built     11 mutants in ",
        "ran       11 mutants in ",
    ] {
        assert!(
            cleared_then(permanent),
            "{permanent:?} clears the live block: {err:?}"
        );
    }
    let (drawn, cleared) = (err.rfind("\x1b[?7h").unwrap(), err.rfind(clear).unwrap());
    assert!(
        drawn < cleared && err.ends_with('\n'),
        "cleared before the result: {err:?}"
    );
    let (_, plain) = run_with(&dir, "--progress=none", false);
    assert!(plain.is_empty(), "{plain:?}");
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
}

/// A copy of a fixture this test process has to itself, to change.
fn copy_of(name: &str, label: &str) -> std::path::PathBuf {
    common::own_copy(name, &format!("samples-{label}"))
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
        // What these runs say is what they find: their progress is tested on its own.
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
}

/// A command that refuses says so on stderr, never stdout, which holds only results: a script
/// that reads stdout reads nothing it has to tell from them. A refusal that does nothing exits 1;
/// gate check with no task open exits 2, as declared.
#[test]
fn a_refusal_is_on_stderr() {
    let dir = copy_of("pricing", "refusals");
    for (words, code) in [
        (&["ignore", "--remove", "mb2f8ae57"][..], 1),
        (&["ignore", "--accept", "mb2f8ae57"], 1),
        (&["baseline", "--prune"], 1),
        (&["gate", "check"], 2),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_mutastic"))
            .args(words)
            .current_dir(&dir)
            .env("MUTASTIC_PROGRESS", "none")
            .env("NO_COLOR", "1")
            .output()
            .unwrap();
        let err = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(code), "{words:?}: {err}");
        assert!(
            output.stdout.is_empty(),
            "{words:?}: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(err.starts_with("mutastic"), "{words:?}: {err}");
    }
}

/// The header `mutastic baseline` writes, the Java edition's.
const BASELINE_HEADER: &str =
    "# The mutants this project missed when its baseline was taken, written by `mutastic baseline`.
# Commit it. A miss that isn't here is new: the gate holds only new misses. As tests
# catch these, `mutastic baseline --prune` drops them; it never takes in new misses.
baseline:
";

/// The `run-finished` event of a `--format=jsonl` run, each event checked against the schema.
fn run_finished(jsonl: &str) -> Value {
    let events = events(jsonl);
    for event in &events {
        let errors: Vec<String> = schema().iter_errors(event).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{event}\n{}", errors.join("\n"));
    }
    events.last().unwrap().clone()
}

/// `baseline` records every mutant no test catches, once; `run --fail-on-new` passes while each is
/// in it, whatever the score, and fails naming one that isn't, in every format, as the Java
/// edition says it; with no baseline it's a usage error, before the build.
#[test]
fn fail_on_new_judges_a_run_by_its_baseline() {
    let dir = copy_of("pricing", "fail-on-new");
    let file = dir.join(".mutastic/baseline.yaml");
    let judged = ["run", "--fail-on-new", "--threshold=95", "--report=none"];
    let with = |format: &str| {
        let mut args = judged.to_vec();
        args.push(format);
        mutastic_in(&dir, &args)
    };
    for format in ["--format=full", "--format=agent", "--format=jsonl"] {
        let (code, out) = with(format);
        assert_eq!(code, 2, "{out}");
        // In the agent and jsonl formats, the error is first on stdout as a `run-failed` event.
        let said = if format == "--format=full" {
            out.as_str()
        } else {
            let (event, said) = out.split_once('\n').unwrap_or_default();
            let event: Value = serde_json::from_str(event).unwrap_or_else(|_| panic!("{out}"));
            assert_eq!(event["reason"], "run-failed", "{out}");
            assert_eq!(event["code"], "no-baseline", "{out}");
            assert_eq!(event["rendered"], said, "{out}");
            said
        };
        let shown = std::fs::canonicalize(&dir).unwrap();
        assert_eq!(
            said,
            format!(
                "mutastic: --fail-on-new needs a baseline to tell new misses from, and {} has no \
                 .mutastic/baseline.yaml: `mutastic baseline` writes one, to commit\n",
                shown.display()
            ),
            "before anything is read or built"
        );
    }
    let (code, out) = mutastic_in(&dir, &["run", "--fail-on-new", "--only=mb2f8ae57"]);
    assert_eq!(code, 2, "{out}");
    assert!(
        out.starts_with("mutastic: --fail-on-new judges a run of the package on its new misses"),
        "{out}"
    );

    let (code, out) = mutastic_in(&dir, &["baseline", "--progress=none"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.starts_with(
            "mutastic: 1 mutants no test catches are in .mutastic/baseline.yaml: commit it. A miss \
             that isn't in it is new: the gate holds only new misses, and next offers them first\n"
        ),
        "{out}"
    );
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        format!(
            "{BASELINE_HEADER}  - id: mb2f8ae57\n    at: 'Pricing::discount, line 26'\n    \
             change: '`>=` becomes `>`'\n"
        )
    );
    let (code, out) = mutastic_in(&dir, &["baseline"]);
    assert_eq!(code, 1, "{out}");
    assert_eq!(
        out,
        "mutastic: there's a baseline already, in .mutastic/baseline.yaml. `mutastic baseline \
         --prune` drops its mutants the tests catch now; it's never rewritten to take in new \
         misses, which would hide them\n"
    );

    // 90.9% is below 95, and the run passes: its one miss is in the baseline.
    let (code, out) = with("--format=full");
    assert_eq!(code, 0, "{out}");
    assert!(
        out.lines()
            .last()
            .is_some_and(|last| last
                .starts_with("     Score  90.9%  [PASS] No new misses: 1 in the baseline.")),
        "{out}"
    );
    let (code, out) = with("--format=agent");
    assert_eq!(code, 0, "{out}");
    assert!(
        out.starts_with(
            "mutastic: PASS · no new misses, 1 in the baseline · score 90.9% (threshold not \
             applied) · 1 missed, 0 not covered, 10 caught of 11 mutants\n"
        ),
        "{out}"
    );
    let (code, out) = with("--format=jsonl");
    assert_eq!(code, 0, "{out}");
    let finished = run_finished(&out);
    assert_eq!(finished["passed"], true);
    assert_eq!(finished["failOnNew"], true);
    assert_eq!(finished["newMisses"], serde_json::json!([]));
    assert_eq!(finished["inBaseline"], 1);
    // Without it, the threshold decides.
    let (code, out) = mutastic_in(
        &dir,
        &["run", "--threshold=95", "--report=none", "--format=jsonl"],
    );
    assert_eq!(code, 1, "{out}");
    assert!(run_finished(&out).get("failOnNew").is_none(), "{out}");
    // A threshold of 0 passes any score: there's nothing to be above.
    let (code, out) = mutastic_in(&dir, &["run", "--threshold=0", "--report=none"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.lines()
            .last()
            .is_some_and(|last| last
                .starts_with("     Score  90.9%  [PASS] 1 missed, with no threshold set.")),
        "{out}"
    );

    // Taken out of the baseline, the miss is new.
    std::fs::write(&file, BASELINE_HEADER).unwrap();
    let (code, out) = with("--format=full");
    assert_eq!(code, 1, "{out}");
    assert!(
        out.lines().last().is_some_and(|last| last.starts_with(
            "     Score  90.9%  [FAIL] 1 new miss, not in the baseline (0 in it): mb2f8ae57."
        )),
        "{out}"
    );
    let (code, out) = with("--format=agent");
    assert_eq!(code, 1, "{out}");
    assert!(
        out.starts_with(
            "mutastic: FAIL · 1 new miss, not in the baseline (0 in it): mb2f8ae57 · score 90.9% \
             (threshold not applied) · "
        ),
        "{out}"
    );
    let (code, out) = with("--format=jsonl");
    assert_eq!(code, 1, "{out}");
    let finished = run_finished(&out);
    assert_eq!(finished["passed"], false);
    assert_eq!(finished["newMisses"], serde_json::json!(["mb2f8ae57"]));
    assert_eq!(finished["inBaseline"], 0);
}

/// A function no test checks, for a miss the baseline doesn't have: its return is a condition or
/// a removal, as the baseline's miss is, but one test reaches it where two reach that one, so the
/// run's own order puts it second.
const UNCHECKED: &str = "\npub fn tax(total: u32) -> u32 {\n    total / 5\n}\n";

/// `next` offers a miss the baseline doesn't have before one it does, and says which is which;
/// `baseline --prune` drops a mutant the tests catch now, and never takes in a new miss.
#[test]
fn next_puts_new_misses_first_and_prune_drops_the_caught() {
    let dir = copy_of("pricing", "baseline-next");
    let file = dir.join(".mutastic/baseline.yaml");
    let (code, out) = mutastic_in(&dir, &["baseline", "--prune"]);
    assert_eq!(code, 1, "{out}");
    assert_eq!(
        out,
        "mutastic: there's no baseline to prune: `mutastic baseline` writes one\n"
    );
    let (code, out) = mutastic_in(&dir, &["baseline"]);
    assert_eq!(code, 0, "{out}");

    let lib = dir.join("src/lib.rs");
    let text = std::fs::read_to_string(&lib).unwrap();
    std::fs::write(&lib, format!("{text}{UNCHECKED}")).unwrap();
    let tests = dir.join("tests/pricing.rs");
    let kept = std::fs::read_to_string(&tests).unwrap();
    std::fs::write(
        &tests,
        format!("{kept}\n#[test]\nfn tax_runs() {{\n    let _ = pricing::tax(10);\n}}\n"),
    )
    .unwrap();
    let (code, jsonl) = mutastic_in(&dir, &["next", "--format=jsonl", "--top=2"]);
    assert_eq!(code, 1, "{jsonl}");
    let events = events(&jsonl);
    for event in &events {
        let errors: Vec<String> = schema().iter_errors(event).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{event}\n{}", errors.join("\n"));
    }
    let items: Vec<&Value> = events
        .iter()
        .filter(|e| e["reason"] == "next-item")
        .collect();
    assert_eq!(items.len(), 2, "{jsonl}");
    assert_eq!(
        items[0]["location"]["method"], "tax",
        "the new miss first: {jsonl}"
    );
    assert_eq!(items[0]["rank"]["factors"][0]["signal"], "baseline");
    assert_eq!(items[0]["rank"]["factors"][0]["value"], "new");
    assert_eq!(items[1]["id"], "mb2f8ae57", "{jsonl}");
    assert_eq!(items[1]["rank"]["factors"][0]["value"], "legacy");
    assert_eq!(events.last().unwrap()["legacy"], 1, "{jsonl}");
    let (_, out) = mutastic_in(&dir, &["next", "--progress=none"]);
    assert!(
        out.contains(
            "Next: 1 of 2 lines with missed mutants in pricing (1 new, 1 in the baseline)\n"
        ),
        "{out}"
    );
    assert!(
        out.contains(" Ranked first because\n   not in the baseline: a new miss\n"),
        "{out}"
    );

    // A test for the baseline's miss: a full run says it's caught now, and prune drops it, but
    // not the new misses.
    std::fs::write(
        &tests,
        format!(
            "{kept}\n#[test]\nfn at_the_edge() {{\n    assert_eq!(Pricing::new().discount(100), 10);\n}}\n"
        ),
    )
    .unwrap();
    let (_, out) = mutastic_in(&dir, &["run", "--report=none", "--progress=none"]);
    assert!(
        out.contains(
            "note: 1 of the baseline's mutants are caught now: `mutastic baseline --prune` drops them\n"
        ),
        "{out}"
    );
    let (code, out) = mutastic_in(&dir, &["baseline", "--prune", "--progress=none"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.starts_with(
            "mutastic: 1 of the baseline's mutants are caught now, and are dropped from \
             .mutastic/baseline.yaml\n"
        ),
        "{out}"
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), BASELINE_HEADER);
}

/// The gate holds only new misses: one the baseline had before the task is the project's
/// backlog, even on a line the task changed; and the task can't add to the baseline itself.
#[test]
fn the_gate_holds_only_new_misses() {
    let dir = copy_of("pricing", "gate-baseline");
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
    let (code, out) = mutastic_in(&dir, &["baseline", "--progress=none"]);
    assert_eq!(code, 0, "{out}");
    git(&["init", "-q"]);
    std::fs::write(dir.join(".gitignore"), "target\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "start"]);
    let (code, out) = mutastic_in(&dir, &["gate", "open"]);
    assert_eq!(code, 0, "{out}");

    // The baseline's miss is on the line the task changes: it doesn't hold the task.
    let lib = dir.join("src/lib.rs");
    let text = std::fs::read_to_string(&lib).unwrap();
    std::fs::write(
        &lib,
        text.replace("if total >= 100 {", "if total >= 100  {"),
    )
    .unwrap();
    let (code, out) = mutastic_in(&dir, &["gate", "check"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(
            "; 1, missed before the task and in .mutastic/baseline.yaml, is the project's backlog, \
             not the task's\n"
        ),
        "{out}"
    );

    // A miss the task added to the baseline itself is a person's call.
    let file = dir.join(".mutastic/baseline.yaml");
    let baseline = std::fs::read_to_string(&file).unwrap();
    std::fs::write(
        &file,
        format!("{baseline}  - id: m00000001\n    at: 'x'\n    change: 'y'\n"),
    )
    .unwrap();
    let (code, out) = mutastic_in(&dir, &["gate", "check"]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains(
            "mutastic gate: the task added m00000001 to .mutastic/baseline.yaml itself, and \
             whether a miss is the project's backlog is a person's call"
        ),
        "{out}"
    );
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
    let skipped = arid
        .iter()
        .find(|e| e["reason"] == "mutant-finished")
        .unwrap();
    assert_eq!(skipped["statusReason"], "arid: only writes output");
    assert_eq!(skipped["skipKind"], "arid");
    assert_eq!(arid.last().unwrap()["ignored"], 1);
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
}

/// An ignore the person accepts between turns, after the gate let the agent stop and before their
/// next prompt, is the task's to start from: the next check passes. One the agent accepts in its
/// turn, or writes into `.mutastic/ignores.yaml` itself, stays the agent's, after the person's next
/// prompt too: re-reading the ignores at every prompt would pass whatever the agent ignored.
#[test]
fn the_gate_takes_an_ignore_the_person_accepts_between_turns() {
    for who in ["person", "agent", "file"] {
        let dir = copy_of("pricing", &format!("gate-accept-{who}"));
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
        git(&["init", "-q"]);
        std::fs::write(dir.join(".gitignore"), "target\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "start"]);
        let hook = |action: &str| -> Value {
            let (code, out) = mutastic_with_input(
                &dir,
                &["gate", action, ".", "--hook=claude"],
                r#"{"session_id": "s1"}"#,
            );
            assert_eq!(code, 0, "{out}");
            if action == "open" {
                return Value::Null;
            }
            serde_json::from_str(out.trim()).unwrap()
        };
        hook("open");

        // The agent's turn: a change no test checks, its mutants proposed, and the gate lets it stop.
        let lib = dir.join("src/lib.rs");
        let text = std::fs::read_to_string(&lib).unwrap();
        std::fs::write(
            &lib,
            format!("{text}\npub fn double(x: u32) -> u32 {{\n    x * 2\n}}\n"),
        )
        .unwrap();
        let (_, ids) = mutastic_in(&dir, &["mutants"]);
        let ids: Vec<String> = ids
            .lines()
            .filter(|line| line.contains(" in double"))
            .filter_map(|line| line.split_whitespace().next().map(str::to_string))
            .collect();
        assert!(!ids.is_empty(), "double has mutants");
        assert_eq!(hook("check")["decision"], "block");
        for id in &ids {
            let (code, out) = mutastic_in(
                &dir,
                &[
                    "ignore",
                    id,
                    "--propose",
                    "--as=accepted-risk",
                    "--reason=only doubles",
                ],
            );
            assert_eq!(code, 0, "{out}");
        }
        let accept_all = || {
            for id in &ids {
                let (code, out) = mutastic_in(&dir, &["ignore", "--accept", id]);
                assert_eq!(code, 0, "{out}");
            }
        };
        let proposals = dir.join(".mutastic/proposals.yaml");
        match who {
            "agent" => accept_all(),
            // The agent moves the proposals into the ignores by hand, rather than accept them.
            "file" => {
                std::fs::rename(&proposals, dir.join(".mutastic/ignores.yaml")).unwrap();
            }
            _ => {}
        }
        let stop = hook("check");
        if who == "person" {
            assert!(
                stop["systemMessage"]
                    .as_str()
                    .unwrap()
                    .starts_with("mutastic gate: passed"),
                "{stop}"
            );
            // Between turns: the person accepts what the agent proposed, and prompts again.
            accept_all();
        } else {
            // Held; sent back, the agent changes nothing, so it's over to the person, with the
            // ignores still the agent's.
            assert_eq!(stop["decision"], "block", "{who}: {stop}");
            assert!(
                stop["reason"]
                    .as_str()
                    .unwrap()
                    .contains("itself, and whether a mutant is worth a test"),
                "{who}: {stop}"
            );
            let handed = hook("check");
            assert!(
                handed["systemMessage"]
                    .as_str()
                    .unwrap()
                    .contains("so it's over to you"),
                "{who}: {handed}"
            );
        }
        hook("open");
        let next = hook("check");
        if who == "person" {
            assert!(
                next["systemMessage"]
                    .as_str()
                    .unwrap()
                    .starts_with("mutastic gate: passed"),
                "{next}"
            );
        } else {
            assert_eq!(next["decision"], "block", "{who}: {next}");
            assert!(
                next["reason"]
                    .as_str()
                    .unwrap()
                    .contains(&format!("the task ignored {}", ids[0])),
                "{who}: {next}"
            );
        }
        std::fs::remove_dir_all(&dir).ok();
    }
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
}

/// A test that fails, then passes when run again on its own, is flaky: the run goes on without it
/// and says so, in each format, in the Java edition's words, and what only it reached is not
/// covered. A test that fails every time stops the run, and nothing says to turn it off.
#[test]
fn a_flaky_test_is_left_out_and_one_that_always_fails_stops_the_run() {
    let dir = copy_of("flaky", "flaky");
    let test = "raffle::a_draw_is_a_place_in_the_raffle";
    let (code, out) = mutastic_in(
        &dir,
        &[
            "run",
            "--format=full",
            "--report=none",
            "--progress=counter",
        ],
    );
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
            "mutastic: {test} fails without any mutant, so mutastic can't judge one.\n\
             It panicked: fails every time\n"
        )),
        "{out}"
    );
    let rerun = "cargo test -p raffle --test raffle -- --exact a_draw_is_a_place_in_the_raffle";
    assert!(
        out.contains(&format!(
            "Its output is in target/mutastic/tests.log. To run it alone:\n  {rerun}\n"
        )),
        "{out}"
    );
    assert!(!out.contains("ignore"), "{out}");
    let log = std::fs::read_to_string(dir.join("target/mutastic/tests.log")).unwrap();
    assert!(log.contains("fails every time"), "{log}");
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
}

/// A closed stdout (`mutastic run | head`) ends the run as an open one does, never in a panic and
/// exit 101: what can't be written is dropped, in every format.
#[test]
fn a_closed_stdout_ends_the_run_as_an_open_one() {
    let copy = copy_of("pricing", "closed-stdout");
    let run = |format: &str, stdout: Stdio| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_mutastic"))
            .args(["run", copy.to_str().unwrap(), "--report=none"])
            .arg(format!("--format={format}"))
            .env("NO_COLOR", "1")
            .env_remove("MUTASTIC_FORMAT")
            .env_remove("MUTASTIC_AGENT")
            .stdout(stdout)
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        drop(child.stdout.take());
        let output = child.wait_with_output().unwrap();
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).contains("panicked"),
        )
    };
    for format in ["full", "agent", "jsonl"] {
        let open = run(format, Stdio::null());
        assert_eq!(run(format, Stdio::piped()), open, "--format={format}");
        assert_eq!(open, (Some(0), false), "--format={format}");
    }
    for words in [&["--help"][..], &["run", "--help"], &["mutants", "."]] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_mutastic"))
            .args(words)
            .current_dir(&copy)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        drop(child.stdout.take());
        assert_eq!(child.wait().unwrap().code(), Some(0), "{words:?}");
    }
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

/// `explain --format=jsonl`: the events a run writes for the one mutant, then `mutant-explained`
/// with what `About this mutant` says, each field in the text's words, as the Java edition writes
/// it; another format is refused.
#[test]
fn explain_as_events_says_what_the_text_says() {
    let dir = copy_of("pricing", "explain-jsonl");
    mutastic_in(&dir, &["run", "--format=concise"]);
    let (code, stdout) = mutastic_in(&dir, &["explain", "mb2f8ae57", "--format=jsonl"]);
    let (_, text) = mutastic_in(&dir, &["explain", "mb2f8ae57"]);
    let (refused, said) = mutastic_in(&dir, &["explain", "mb2f8ae57", "--format=agent"]);
    assert_eq!(code, 0, "{stdout}");
    // Its progress is on stderr, which comes back with it here: the events are the JSON lines.
    let lines: String = stdout
        .lines()
        .filter(|line| line.starts_with('{'))
        .map(|line| format!("{line}\n"))
        .collect();
    let all = events(&lines);
    let reasons: Vec<&str> = all.iter().map(|e| e["reason"].as_str().unwrap()).collect();
    assert_eq!(
        reasons,
        [
            "run-started",
            "mutant-finished",
            "run-finished",
            "mutant-explained"
        ],
        "{stdout}"
    );
    for event in &all {
        let errors: Vec<String> = schema().iter_errors(event).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{event}\n{}", errors.join("\n"));
    }
    let explained = &all[3];
    assert_eq!(explained["fingerprint"], "b2f8ae57ca6c48cb");
    assert_eq!(explained["applied"]["level"], "source");
    assert_eq!(
        explained["applied"]["text"], all[1]["mutation"]["applied"]["text"],
        "the same text as the mutant-finished event's applied"
    );
    for words in [
        &explained["operator"]["description"],
        &explained["idDescription"],
        &explained["idNote"],
        &explained["site"]["description"],
        &explained["lastRun"]["description"],
        &explained["applied"]["description"],
    ] {
        assert!(
            text.contains(words.as_str().unwrap()),
            "{words} isn't in:\n{text}"
        );
    }
    assert_eq!(refused, 2, "{said}");
    assert!(
        said.contains("explain takes --format=full or --format=jsonl"),
        "{said}"
    );
}

/// Where a command runs is checked before Cargo runs. A directory that isn't there, a word that's
/// an option, a workspace's root, no project at all and no Cargo are each said in the person's
/// words, ending in the command to run instead, and that command runs. A package's name typed
/// alone runs that package, and says so; below a package, the package runs, with its own
/// `mutastic.yaml`, ignores and `--since` read from its directory, as from the package itself.
#[test]
fn a_command_run_in_the_wrong_place_ends_in_the_one_to_run() {
    let root = copy_of("workspace", "located");
    std::fs::create_dir_all(root.join("crates")).unwrap();
    std::fs::rename(root.join("shop"), root.join("crates/shop")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"billing\", \"crates/shop\"]\nresolver = \"2\"\n",
    )
    .unwrap();
    let real = std::fs::canonicalize(&root).unwrap();
    let ran = |dir: &Path, args: &[&str], env: &[(&str, &Path)]| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mutastic"));
        command.args(args).current_dir(dir).env("NO_COLOR", "1");
        for (name, value) in env {
            command.env(name, value);
        }
        command.output().unwrap()
    };
    let stopped = |dir: &Path, args: &[&str]| {
        let output = ran(dir, args, &[]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        String::from_utf8_lossy(&output.stderr).into_owned()
    };
    // Each command a message ends in runs where it was shown.
    let runs = |dir: &Path, message: &str| {
        for line in message
            .lines()
            .filter_map(|l| l.strip_prefix("  mutastic "))
        {
            let (code, out) = mutastic_in(dir, &line.split(' ').collect::<Vec<_>>());
            assert_eq!(code, 0, "mutastic {line}: {out}");
        }
    };

    // The workspace's root: each package, as the command to test it.
    let listed = stopped(&root, &["mutants", "--operators=defaults"]);
    assert_eq!(
        listed,
        "mutastic: this is a workspace's root, and mutastic tests one package at a time.\n\
         It has 2 packages. To test one:\n  \
         mutastic mutants billing --operators=defaults\n  \
         mutastic mutants crates/shop --operators=defaults\n"
    );
    runs(&root, &listed);
    // A directory in the workspace that's in none of its packages: the same, from where it is.
    let crates = stopped(&root.join("crates"), &["mutants"]);
    assert!(
        crates.starts_with(
            "mutastic: this directory is in a workspace, but in none of its packages, and \
             mutastic tests one package at a time.\nIt has 2 packages. To test one:\n  \
             mutastic mutants ../billing\n  mutastic mutants shop\n"
        ),
        "{crates}"
    );
    runs(&root.join("crates"), &crates);

    // A name close to a package's, where there's no such directory: offered, never run.
    let near = stopped(&root, &["mutants", "shpo"]);
    assert_eq!(
        near,
        format!(
            "mutastic: there's no directory shpo in {}.\n\
             There's a package called shop, in crates/shop. To test it:\n  \
             mutastic mutants crates/shop\n",
            real.display()
        )
    );
    runs(&root, &near);
    // Close to nothing: the workspace's packages.
    let far = stopped(&root, &["mutants", "nowhere"]);
    assert!(
        far.ends_with(
            ".\nThis workspace has 2 packages. To test one:\n  \
             mutastic mutants billing\n  mutastic mutants crates/shop\n"
        ),
        "{far}"
    );
    // A package's name exactly: that package runs, and the note says where.
    // shop has no tests, so its run fails: on its score, not with mutastic's 2.
    let (code, out) = mutastic_in(&root, &["run", "shop", "--report=none"]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("mutastic: shop is the package in crates/shop, so mutastic runs there\n"),
        "{out}"
    );
    // A package with no tests says so, in place of a score below the threshold.
    assert!(
        out.contains("Found no tests, so no mutant can be caught: "),
        "{out}"
    );

    // A word with one dash is an option mutastic doesn't have, never a directory.
    let short = stopped(&root, &["run", "-v"]);
    assert_eq!(
        short,
        "mutastic: run has no option -v.\n\
         mutastic's options are long ones, such as --format=full. To see them all:\n  \
         mutastic run --help\n"
    );
    let help = ran(&root, &["--help"], &[]);
    assert_eq!(help.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&help.stdout).contains("\nOptions:\n  --only=<id>"));

    // Below a package, the package runs: its ignores, its mutastic.yaml and --since are read
    // from its directory.
    let src = root.join("crates/shop/src");
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(&root)
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
    std::fs::write(root.join(".gitignore"), "target\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "start"]);
    let finished = |args: &[&str]| {
        let (code, jsonl) = mutastic_in(&src, args);
        assert_ne!(code, 2, "{jsonl}");
        events(&jsonl)
            .into_iter()
            .filter(|e| e["reason"] == "mutant-finished")
            .collect::<Vec<_>>()
    };
    let since = ["run", "--since=HEAD", "--format=jsonl", "--report=none"];
    assert_eq!(finished(&since).len(), 0);
    let lib = root.join("crates/shop/src/lib.rs");
    let text = std::fs::read_to_string(&lib).unwrap();
    std::fs::write(
        &lib,
        text.replace("prices.iter().sum()", "prices.iter().sum::<u32>()"),
    )
    .unwrap();
    let changed = finished(&since);
    assert!(
        !changed.is_empty() && changed.iter().all(|e| e["location"]["method"] == "total"),
        "{changed:?}"
    );
    std::fs::write(
        root.join("crates/shop/mutastic.yaml"),
        "rules:\n  - skip: shop::total\n    reason: summed by the till\n",
    )
    .unwrap();
    let skipped = finished(&["run", "--format=jsonl", "--report=none"]);
    assert!(
        !skipped.is_empty()
            && skipped
                .iter()
                .all(|e| e["statusReason"] == "user: summed by the till (rule `skip: shop::total`)"),
        "{skipped:?}"
    );
    let (_, listing) = mutastic_in(&src, &["mutants"]);
    let id = listing.split_whitespace().next().unwrap().to_string();
    let (code, out) = mutastic_in(&src, &["ignore", &id, "--reason=kept"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.ends_with(&format!("to undo: mutastic ignore --remove {id} ..\n")),
        "{out}"
    );
    assert!(root.join("crates/shop/.mutastic/ignores.yaml").is_file());
    assert!(!src.join(".mutastic").exists());

    // No project at all, in either edition's terms.
    let empty = std::env::temp_dir().join(format!("mutastic-no-project-{}", std::process::id()));
    std::fs::create_dir_all(&empty).unwrap();
    let nothing = stopped(&empty, &["run"]);
    assert_eq!(
        nothing,
        format!(
            "mutastic: there's no project here for mutastic to test.\n\
             It looked in {} and every directory above it, for Cargo.toml, pom.xml, \
             build.gradle(.kts) or settings.gradle(.kts).\n\
             Go to your project, or name its directory:\n  mutastic run <project directory>\n",
            std::fs::canonicalize(&empty).unwrap().display()
        )
    );

    // No Cargo where the CARGO variable points.
    let no_cargo = root.join("no-cargo");
    let output = ran(
        &root.join("billing"),
        &["mutants"],
        &[("CARGO", no_cargo.as_path())],
    );
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!(
            "mutastic: CARGO is {}, and there's no cargo there.\n\
             mutastic runs the cargo CARGO names. Point it at one, or unset it to use the cargo on \
             the PATH:\n  unset CARGO\n",
            no_cargo.display()
        )
    );
    std::fs::remove_dir_all(&empty).ok();
    std::fs::remove_dir_all(&root).ok();
}

/// No Cargo on the PATH, and no CARGO variable to say where one is.
#[cfg(unix)]
#[test]
fn no_cargo_on_the_path_is_said_with_how_to_get_one() {
    let output = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args(["mutants", &fixture("pricing")])
        .env_remove("CARGO")
        .env("PATH", "/nonexistent")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "mutastic: there's no cargo on the PATH, and mutastic reads a Rust package with it.\n\
         Install Rust from https://rustup.rs, or add the directory that holds cargo to the PATH. \
         To check it's found:\n  cargo --version\n"
    );
}

/// A run is never silent. With a heartbeat of a second, a piped stderr gets a line at least every
/// two seconds, through a build, a coverage pass whose test takes three seconds, and mutants
/// that take as long; and every phase says something.
#[test]
fn a_piped_run_is_never_quiet_for_longer_than_its_heartbeat() {
    let dir = fixture("sleepy");
    let mut child = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args(["run", &dir, "--format=concise", "--report=none"])
        .env("MUTASTIC_HEARTBEAT", "1")
        .env("NO_COLOR", "1")
        .env_remove("MUTASTIC_PROGRESS")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let lines: Vec<(Duration, String)> = std::io::BufReader::new(child.stderr.take().unwrap())
        .lines()
        .map(|line| (started.elapsed(), line.unwrap()))
        .collect();
    child.wait().unwrap();
    let mut last = Duration::ZERO;
    for (at, line) in &lines {
        assert!(
            *at - last <= Duration::from_secs(2),
            "{:?} without a line before {line:?}:\n{lines:#?}",
            *at - last
        );
        last = *at;
    }
    for phase in [
        "reading",
        "building ",
        "built ",
        "coverage ",
        "mutants ",
        "ran ",
    ] {
        assert!(
            lines.iter().any(|(_, line)| line.contains(phase)),
            "no {phase:?} line:\n{lines:#?}"
        );
    }
}

/// `kill -USR1` asks a run what's running, piped as on a terminal: here, during the coverage
/// pass's three-second test, which test it is and how long it's run.
#[cfg(unix)]
#[test]
fn sigusr1_says_whats_running() {
    let dir = copy_of("sleepy", "sigusr1");
    let mut child = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args(["run", &dir.display().to_string(), "--report=none"])
        .env("MUTASTIC_PROGRESS", "counter")
        .env("MUTASTIC_HEARTBEAT", "0.5")
        .env("NO_COLOR", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = std::io::BufReader::new(child.stderr.take().unwrap());
    let (mut lines, mut asked) = (Vec::new(), false);
    for line in stderr.lines() {
        let line = line.unwrap();
        // Once a heartbeat says the test is running, however long the machine took to get there.
        if !asked && line.starts_with("coverage  [0/1 tests] · ") {
            asked = true;
            let asked = Command::new("kill")
                .args(["-USR1", &child.id().to_string()])
                .status()
                .unwrap();
            assert!(asked.success());
        }
        lines.push(line);
    }
    child.wait().unwrap();
    let said = lines.join("\n");
    let running = regex::Regex::new(
        r"running   1 test at \ds · 0 of 1 done\n   \d tests::doubles_slowly  \[00:00:0\d\]",
    )
    .unwrap();
    assert!(running.is_match(&said), "{said}");
}

/// `--format=jsonl` streams the run's phases and progress as they happen: `run-started` once, as
/// the build finishes, with only phase events before it; `progress` on the heartbeat's cadence;
/// and every event as the schema has it.
#[test]
fn jsonl_streams_phases_and_progress_as_they_happen() {
    let dir = copy_of("sleepy", "live-events");
    let started = std::time::Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args([
            "run",
            &dir.display().to_string(),
            "--format=jsonl",
            "--report=none",
        ])
        .env("MUTASTIC_HEARTBEAT", "0.5")
        .env("MUTASTIC_PROGRESS", "none")
        .output()
        .unwrap();
    assert!(
        started.elapsed() > Duration::from_secs(3),
        "the fixture's test sleeps"
    );
    let events = events(&String::from_utf8_lossy(&output.stdout));
    let schema = schema();
    for event in &events {
        let errors: Vec<String> = schema.iter_errors(event).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{event}\n{}", errors.join("\n"));
    }
    let reasons: Vec<&str> = events
        .iter()
        .map(|e| e["reason"].as_str().unwrap())
        .collect();
    let run_started = reasons.iter().position(|&r| r == "run-started").unwrap();
    assert_eq!(reasons.iter().filter(|&&r| r == "run-started").count(), 1);
    assert!(
        reasons[..run_started]
            .iter()
            .all(|r| r.starts_with("phase-")),
        "{reasons:?}"
    );
    let phases: Vec<&str> = events
        .iter()
        .filter(|e| e["reason"] == "phase-started")
        .map(|e| e["phase"].as_str().unwrap())
        .collect();
    assert_eq!(phases, ["reading", "building", "coverage", "mutants"]);
    let progress: Vec<&Value> = events
        .iter()
        .filter(|e| e["reason"] == "progress")
        .collect();
    assert!(
        progress
            .iter()
            .any(|e| e["phase"] == "coverage" && e["total"] == 1),
        "{progress:#?}"
    );
    assert!(
        progress
            .iter()
            .any(|e| e["phase"] == "mutants" && e["running"].as_u64() > Some(0)),
        "{progress:#?}"
    );
    assert_eq!(reasons.last(), Some(&"run-finished"));
}
