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
    let output = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args(args)
        .current_dir(dir)
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
    assert!(
        out.contains("not covered (no test reaches these lines):\n- src/lib.rs:"),
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
