//! The docs are kept true. Every output block in `docs/` and the README is what mutastic prints on
//! one of the fixtures: it's captured in `docs/output/`, in colour as a terminal shows it, and this
//! fails while a fresh run prints anything else. A page shows its copy without colour, as GitHub
//! can't show it; the docs site shows the captured colour.
//! Every Rust block is either an excerpt of a fixture these runs build, word for word, the test
//! the getting-started page adds, or the test-first pages' cycles and the coverage page's tests,
//! which this writes and runs.
//! Every link between pages resolves, every `mutastic` command a page shows parses, and every id it names is a mutant of its fixture. And
//! every missed mutant's card on the operators fixture says something true: written into the
//! source as the card claims it, the fixture's tests still pass.
//!
//! To write the docs' copy again, after a change to what mutastic prints:
//! `MUTASTIC_DOCS_WRITE=1 cargo test --test docs`, then read the diff.

#![allow(
    clippy::unwrap_used,
    reason = "a helper here asserts as the tests it serves do: a value that isn't there fails the test"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use mutastic::{engine, workspace};
use serde_json::Value;

mod common;

/// The operator families, each a page under `docs/operators/` with a file of its own in
/// `fixtures/operators/src/`, and the operators its run uses.
const FAMILIES: &[(&str, &str)] = &[
    ("conditions", "mutastic/boundary,mutastic/comparison,mutastic/other-comparison,mutastic/remove-conditional,mutastic/logical-connector"),
    ("arithmetic", "mutastic/math,mutastic/arithmetic-replacement,mutastic/other-arithmetic,mutastic/keep-operand,mutastic/remove-negation,mutastic/constant-value,mutastic/bitwise,library/unchecked-arithmetic"),
    ("returns", "mutastic/return-value,mutastic/return-value(extended),mutastic/replace-body,mutastic/unwrap-try,mutastic/remove-throw"),
    ("calls", "mutastic/remove-void-call,mutastic/remove-discarded-call,mutastic/remove-assignment,mutastic/remove-field-write,mutastic/remove-struct-field"),
    ("arguments", "mutastic/omit-call,mutastic/swap-arguments,mutastic/drop-vararg,mutastic/null-parameter,mutastic/default-parameter"),
    ("library", "library/swap-string,library/swap-math,library/swap-index-of,library/reverse-ordering,mutastic/string-constant"),
    ("iterators", "stream/swap-match,stream/remove-stage,stream/remove-other-stage"),
    ("options", "optional/swap-present,optional/empty,optional/always-default,optional/no-default,optional/remove-filter"),
    ("control", "mutastic/remove-switch-arm,mutastic/swap-break"),
];

/// The card each operator's section shows: its family, the function, and the operator.
const CARDS: &[(&str, &str, &str)] = &[
    ("conditions", "free_delivery", "mutastic/boundary"),
    ("conditions", "needs_restock", "mutastic/comparison"),
    ("conditions", "is_weekend", "mutastic/other-comparison"),
    ("conditions", "greeting", "mutastic/remove-conditional"),
    ("conditions", "can_ship", "mutastic/logical-connector"),
    ("arithmetic", "line_total", "mutastic/math"),
    ("arithmetic", "with_fee", "mutastic/arithmetic-replacement"),
    ("arithmetic", "per_person", "mutastic/other-arithmetic"),
    ("arithmetic", "after_refund", "mutastic/keep-operand"),
    ("arithmetic", "reversed", "mutastic/remove-negation"),
    ("arithmetic", "postage", "mutastic/constant-value"),
    ("arithmetic", "has_flag", "mutastic/bitwise"),
    ("arithmetic", "add_points", "library/unchecked-arithmetic"),
    ("returns", "loyalty_points", "mutastic/return-value"),
    ("returns", "is_valid_code", "mutastic/replace-body"),
    ("returns", "parse_quantity", "mutastic/unwrap-try"),
    ("returns", "quantity", "mutastic/remove-throw"),
    ("calls", "add", "mutastic/remove-void-call"),
    ("calls", "first_visit", "mutastic/remove-discarded-call"),
    ("calls", "with_tip", "mutastic/remove-assignment"),
    ("calls", "pay", "mutastic/remove-field-write"),
    ("calls", "muted", "mutastic/remove-struct-field"),
    ("arguments", "username", "mutastic/omit-call"),
    ("arguments", "is_percent", "mutastic/swap-arguments"),
    ("arguments", "best_of_three", "mutastic/drop-vararg"),
    ("arguments", "price_with", "mutastic/null-parameter"),
    ("arguments", "greet", "mutastic/default-parameter"),
    ("library", "is_comment", "library/swap-string"),
    ("library", "capped_discount", "library/swap-math"),
    ("library", "extension", "library/swap-index-of"),
    ("library", "sorted", "library/reverse-ordering"),
    ("library", "header", "mutastic/string-constant"),
    ("iterators", "any_overdue", "stream/swap-match"),
    ("iterators", "paid_total", "stream/remove-stage"),
    ("iterators", "logged_total", "stream/remove-other-stage"),
    ("options", "display_name", "optional/swap-present"),
    ("options", "page_size", "optional/empty"),
    ("options", "port", "optional/always-default"),
    ("options", "largest", "optional/no-default"),
    ("options", "plausible_age", "optional/remove-filter"),
    ("control", "delivery_days", "mutastic/remove-switch-arm"),
    ("control", "first_negative", "mutastic/swap-break"),
];

/// Operators whose examples need a crate the fixtures don't depend on: their pages show the
/// mutants `mutastic mutants` finds on the truth fixture's stand-ins.
const LISTED: &[&str] = &["mutastic/regex-constant", "bigdecimal/remove-step"];

/// Fixtures the docs may quote: each is built by a run here, so what's quoted compiles.
const BUILT: &[&str] = &[
    "pricing",
    "checkout",
    "operators",
    "unsafety",
    "loops",
    "truth",
    "platforms",
    "configured",
    "suggestions",
    "ranking",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn docs() -> PathBuf {
    root().join("docs")
}

fn writing() -> bool {
    std::env::var_os("MUTASTIC_DOCS_WRITE").is_some()
}

/// A directory outside the repository, so Cargo doesn't take a copied fixture for a member of
/// mutastic's own workspace, kept from run to run so builds there stay warm, and this test
/// process's alone while it runs.
fn scratch(name: &str) -> PathBuf {
    common::slot(&std::env::temp_dir().join("mutastic-rs-docs"), name).0
}

/// Where a section's fixtures are copied to run, as `fixtures/<name>`, so commands print what a
/// person types at the repository's root, and the repository's own fixtures are left alone. Each
/// section has its own copy, of only the fixtures it runs: the sections run at once, and a mutant
/// runs only the sections that reach its code, not every section for any of them.
fn work(section: &str, fixtures: &[&str]) -> PathBuf {
    let work = scratch(&format!("docs-{section}"));
    for name in fixtures {
        let fixture = work.join("fixtures").join(name);
        // The runs a page shows start with no last run to compare with, whatever this copy ran
        // before.
        common::fresh(&root().join("fixtures").join(name), &fixture);
    }
    work
}

/// Takes away a fixture's kept runs, so its next run is a first run, as a fresh copy's is, with its
/// build still warm.
fn forget_last_runs(fixture: &Path) {
    let kept = fixture.join("target/mutastic");
    std::fs::remove_file(kept.join("last-run.tsv")).ok();
    std::fs::remove_dir_all(kept.join("last-runs")).ok();
    std::fs::remove_file(kept.join(mutastic::nextrun::FILE)).ok();
}

struct Ran {
    code: i32,
    stdout: String,
    stderr: String,
}

/// `mutastic` in `work`, as a person at a terminal runs it there: cards, unless `--format` says
/// otherwise, in colour.
fn mutastic(work: &Path, args: &[&str]) -> Ran {
    let output = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args(args)
        .current_dir(work)
        .env("MUTASTIC_FORMAT", "full")
        .env_remove("MUTASTIC_AGENT")
        .env_remove("NO_COLOR")
        .env("FORCE_COLOR", "1")
        .output()
        .unwrap();
    Ran {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// `mutastic` where it must run to the end: it passed, or it found mutants it missed.
fn ran(work: &Path, args: &[&str]) -> Ran {
    let ran = mutastic(work, args);
    assert!(
        ran.code == 0 || ran.code == 1,
        "mutastic {}: {}{}",
        args.join(" "),
        ran.stdout,
        ran.stderr
    );
    ran
}

/// An escape that sets a colour, or a run of them.
const ESCAPES: &str = r"(?:\x1b\[[0-9;]*m)*";

/// Output as mutastic prints it without colour: a label, black on a block of colour, is `[MISSED]`,
/// and every other colour is gone.
fn plain(text: &str) -> String {
    // Compiled once: a section reads every line of a run's output through here, once per card.
    static LABEL: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"\x1b\[4[0-9];30;1m (?<label>[^\x1b]*) \x1b\[0m").unwrap()
    });
    static ESCAPE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(ESCAPES).unwrap());
    let labelled = LABEL.replace_all(text, "[$label]");
    ESCAPE.replace_all(&labelled, "").into_owned()
}

/// What changes from run to run or machine to machine: timings, when the run started, the
/// compiler's version, the fence token in an agent's text, the number of workers, which follows the cores, and how many tests ran,
/// which follows timing too: a mutant tries the binary whose tests were fastest first, and stops at
/// the first that fails.
fn steady(text: &str) -> String {
    let rules = [
        (r#""testsRun": ?\d+"#, r#""testsRun":…"#),
        (r"Test runs(?<e>(?:\x1b\[[0-9;]*m)*)  \d+", "Test runs$e  …"),
        (
            r"Start at(?<e>(?:\x1b\[[0-9;]*m)*)  \d\d:\d\d:\d\d",
            "Start at$e  …",
        ),
        // A number starts at a word's edge, or straight after a colour, whose `m` would hide the edge.
        (r"(?<c>\x1b\[[0-9;]*m|\b)\d+m\d\ds\b", "${c}…"),
        (r"(?<c>\x1b\[[0-9;]*m|\b)\d+(\.\d+)?(ms|s)\b", "${c}…"),
        (r#"Ms": ?\d+"#, r#"Ms":…"#),
        (
            r#""name": ?"rustc", ?"version": ?"[^"]*""#,
            r#""name":"rustc","version":…"#,
        ),
        (r"rustc \S+", "rustc …"),
        // The commit a gate's task starts from, which changes with each repository a run makes.
        (r"(?<w>from |--since=)[0-9a-f]{7,40}\b", "$w…"),
        (r"(?<c>\x1b\[[0-9;]*m|\b)\d+ workers?\b", "${c}… workers"),
        // The fence around the project's text in an agent's: each run draws its own token.
        (r"mutastic-data [0-9a-f]{8}>>", "mutastic-data 3f9a0c1e>>"),
        (r#""workers(?<s>Started)?": ?\d+"#, r#""workers$s":…"#),
    ];
    rules
        .iter()
        .fold(text.to_string(), |text, (pattern, with)| {
            regex::Regex::new(pattern)
                .unwrap()
                .replace_all(&text, *with)
                .into_owned()
        })
}

/// The card on `output` whose mutants include one of `ids`, from its headline to its last command,
/// in its colour.
fn card(output: &str, ids: &BTreeSet<String>) -> Option<String> {
    let coloured: Vec<&str> = output.lines().collect();
    let lines: Vec<String> = coloured.iter().map(|line| plain(line)).collect();
    let mut start = None;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("[✗ MISSED]") {
            start = Some(index);
        }
        if let (Some(first), Some(rest)) = (start, trimmed.strip_prefix("done when")) {
            let named = rest.split_whitespace().nth(2).unwrap_or_default();
            if named.split(',').any(|id| ids.contains(id)) {
                let end = (index..lines.len())
                    .find(|&i| lines[i].trim_start().starts_with("reproduce"))
                    .unwrap();
                return Some(coloured[first..=end].join("\n") + "\n");
            }
        }
    }
    None
}

/// The summary at the end of a run, from its `Mutants` line.
fn summary(output: &str) -> String {
    let mut at = 0;
    for line in output.split_inclusive('\n') {
        if plain(line).starts_with("   Mutants") {
            return output[at..].to_string();
        }
        at += line.len();
    }
    panic!("no summary: {output}")
}

fn events(stdout: &str) -> Vec<Value> {
    stdout
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn pretty(event: &Value) -> String {
    serde_json::to_string_pretty(event).unwrap() + "\n"
}

/// The ids of a fixture's mutants in `function` by `operator`.
fn ids_of(work: &Path, fixture: &str, function: &str, operator: &str) -> BTreeSet<String> {
    ids_in(&found_in(work, fixture), function, operator)
}

/// Every mutant of a fixture, found once for all the cards that need it: finding them is the
/// engine's whole scan, and a section asked for it once per card.
fn found_in(work: &Path, fixture: &str) -> mutastic::sites::Crate {
    let project =
        workspace::read(&work.join("fixtures").join(fixture).display().to_string()).unwrap();
    engine::find_all(&project)
}

/// The ids of `found`'s mutants in `function` by `operator`.
fn ids_in(found: &mutastic::sites::Crate, function: &str, operator: &str) -> BTreeSet<String> {
    found
        .mutants
        .iter()
        .filter(|m| m.operator == operator && found.functions[m.function].name == function)
        .map(|m| m.id.clone())
        .collect()
}

/// The blocks a page marks for checking, by the comment before them: `<!-- output: name -->`,
/// `<!-- from: path -->` or `<!-- added to: path -->`, and every Rust block.
/// A block's marker: its kind (`output`, `from`, `added to`) and what it names.
type Marker = Option<(String, String)>;

#[derive(Debug)]
struct Block {
    marker: Marker,
    language: String,
    /// Byte range of the block's text, between its fences.
    text: std::ops::Range<usize>,
}

fn pages() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "md") {
                out.push(path);
            }
        }
    }
    // The repository's own pages (the README, CONTRIBUTING and the rest), then the docs.
    let mut out: Vec<PathBuf> = std::fs::read_dir(root())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "md"))
        .collect();
    walk(&docs(), &mut out);
    out.sort();
    out
}

fn blocks(text: &str) -> Vec<Block> {
    let mut found = Vec::new();
    let mut marker = None;
    let mut offset = 0;
    let mut open: Option<(String, usize, Marker)> = None;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        match &open {
            None if trimmed.starts_with("```") => {
                let language = trimmed.trim_start_matches('`').to_string();
                open = Some((language, offset + line.len(), marker.take()));
            }
            None => {
                if let Some(comment) = trimmed
                    .strip_prefix("<!-- ")
                    .and_then(|rest| rest.strip_suffix(" -->"))
                {
                    marker = comment
                        .split_once(": ")
                        .map(|(kind, value)| (kind.to_string(), value.to_string()));
                } else if !trimmed.is_empty() {
                    marker = None;
                }
            }
            Some((language, start, block_marker)) if trimmed == "```" => {
                found.push(Block {
                    marker: block_marker.clone(),
                    language: language.clone(),
                    text: *start..offset,
                });
                open = None;
            }
            Some(_) => {}
        }
        offset += line.len();
    }
    found
}

/// The getting-started tour, in order: a run that misses one mutant, the test the page adds,
/// `verify` passing, and the run after it.
fn getting_started(work: &Path, shown: &mut BTreeMap<String, String>) {
    let page = docs().join("getting-started.md");
    let text = std::fs::read_to_string(&page).unwrap();
    let added = blocks(&text)
        .into_iter()
        .find(|b| {
            b.marker
                .as_ref()
                .is_some_and(|(kind, _)| kind == "added to")
        })
        .map(|b| text[b.text].to_string())
        .expect("getting-started.md adds a test");
    let run = ran(work, &["run", "fixtures/pricing"]);
    assert_eq!(run.code, 0, "{}", run.stdout);
    shown.insert("getting-started/run".into(), run.stdout.clone());
    shown.insert("commands/run".into(), run.stdout.clone());
    shown.insert("concepts/run-progress".into(), run.stderr);
    // The line as mutastic's copy holds it, every mutant of it behind the switch.
    let copy =
        std::fs::read_to_string(work.join("fixtures/pricing/target/mutastic/copy/src/lib.rs"))
            .unwrap();
    let line = copy.lines().nth(25).unwrap();
    assert!(line.contains("total >= 100"), "{line}");
    shown.insert("concepts/schema-line".into(), format!("{}\n", line.trim()));
    let id = plain(&run.stdout)
        .split_whitespace()
        .skip_while(|word| *word != "verify")
        .nth(1)
        .unwrap()
        .to_string();
    shown.insert("readme/card".into(), readme_card(&run.stdout, &id));

    let lib = work.join("fixtures/pricing/src/lib.rs");
    let source = std::fs::read_to_string(&lib).unwrap();
    let close = source.trim_end().rfind('}').unwrap();
    let indented: String = added
        .lines()
        .map(|line| match line {
            "" => "\n".to_string(),
            _ => format!("    {line}\n"),
        })
        .collect();
    std::fs::write(
        &lib,
        format!("{}\n\n{indented}}}\n", source[..close].trim_end()),
    )
    .unwrap();
    let verify = ran(work, &["verify", &id, "fixtures/pricing"]);
    assert_eq!(verify.code, 0, "{}", verify.stdout);
    shown.insert("getting-started/verify".into(), verify.stdout.clone());
    shown.insert("commands/verify".into(), verify.stdout);
    let again = ran(work, &["run", "fixtures/pricing"]);
    shown.insert("getting-started/run-again".into(), again.stdout);
    std::fs::write(&lib, source).unwrap();
    let unknown = mutastic(work, &["verify", "m00000000", "fixtures/pricing"]);
    assert_eq!(unknown.code, 2, "{}", unknown.stderr);
    shown.insert("concepts/unknown-id".into(), unknown.stderr);
}

/// What the README shows of the getting-started run: the card for `id`, then the score and the
/// verdict, as the Java edition's README shows its own.
fn readme_card(output: &str, id: &str) -> String {
    let card = card(output, &BTreeSet::from([id.to_string()])).expect("the run has a card");
    let verdict: String = output
        .lines()
        .skip_while(|line| !plain(line).trim_start().starts_with("Score "))
        .take_while(|line| !plain(line).trim_start().starts_with("Next:"))
        .map(|line| format!("{line}\n"))
        .collect();
    assert!(plain(&verdict).contains("[PASS]"), "{output}");
    format!("{card}\n{verdict}")
}

/// The pricing fixture in each format but `full`, the `mutants` listing, `--version`, and
/// `explain`.
/// The pricing run's `mutation.json`, as the Report files page shows it: its one missed mutant,
/// with the file's text, its absolute root and its timings left out, as they're long or change from
/// machine to machine.
fn mutation_json(work: &Path) -> String {
    let file = work.join("fixtures/pricing/target/mutastic/mutation.json");
    let report: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    let file = &report["files"]["src/lib.rs"];
    let mut missed = file["mutants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|mutant| mutant["status"] == "Survived")
        .unwrap()
        .clone();
    missed.as_object_mut().unwrap().remove("duration");
    pretty(&serde_json::json!({
        "$schema": report["$schema"],
        "schemaVersion": report["schemaVersion"],
        "thresholds": report["thresholds"],
        "files": { "src/lib.rs": {
            "language": file["language"],
            "source": "…",
            "mutants": [missed],
        }},
        "framework": report["framework"],
    }))
}

fn reference(work: &Path, shown: &mut BTreeMap<String, String>) {
    for format in ["brief", "concise", "grouped", "agent", "jsonl"] {
        // Each a first run, as the Java edition shows each format from a fresh copy.
        forget_last_runs(&work.join("fixtures/pricing"));
        let run = ran(
            work,
            &["run", "fixtures/pricing", &format!("--format={format}")],
        );
        if format == "jsonl" {
            for event in events(&run.stdout) {
                let errors: Vec<String> = common::schema()
                    .iter_errors(&event)
                    .map(|e| e.to_string())
                    .collect();
                assert!(errors.is_empty(), "{event}\n{}", errors.join("\n"));
            }
        }
        shown.insert(format!("concepts/format-{format}"), run.stdout);
    }
    shown.insert("reference/mutation-json".into(), mutation_json(work));
    shown.insert(
        "commands/mutants".into(),
        ran(work, &["mutants", "fixtures/pricing"]).stdout,
    );
    // Before the Java edition is fetched: a home with nothing kept in it.
    let home = scratch("no-java-edition");
    std::fs::create_dir_all(&home).unwrap();
    let version = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .arg("--version")
        .env("MUTASTIC_HOME", &home)
        .env_remove("MUTASTIC_JAVA_ENGINE")
        .env_remove("MUTASTIC_VERSION")
        .output()
        .unwrap();
    shown.insert(
        "reference/version".into(),
        String::from_utf8_lossy(&version.stdout).into_owned(),
    );
    // mutastic.yaml's rules file adds an operator, whose card says its own words.
    let listed = plain(&ran(work, &["mutants", "fixtures/configured"]).stdout);
    let ids: BTreeSet<String> = listed
        .lines()
        .filter(|line| line.contains(" acme/remove-trim "))
        .filter_map(|line| line.split_whitespace().next().map(str::to_string))
        .collect();
    assert_eq!(ids.len(), 1, "{listed}");
    let configured = ran(work, &["run", "fixtures/configured", "--format=full"]);
    let found = card(&configured.stdout, &ids).expect("the rules file's card");
    assert!(
        plain(&found).contains("no test passes clean() text with spaces for .trim() to remove"),
        "{found}"
    );
    shown.insert("reference/configured-card".into(), found);
    // After the full runs above, so the last run is known.
    for (name, id) in [("missed", "mb2f8ae57"), ("caught", "m95f96847")] {
        let explained = ran(work, &["explain", id, "fixtures/pricing"]);
        assert_eq!(explained.code, 0, "{}", explained.stdout);
        let last = if name == "missed" {
            "last run  missed: every covering test still passed"
        } else {
            "last run  caught by pricing::big_orders_get_ten_percent_off"
        };
        assert!(
            plain(&explained.stdout).contains(last),
            "{}",
            explained.stdout
        );
        if name == "missed" {
            shown.insert("commands/explain".into(), explained.stdout.clone());
        }
        shown.insert(format!("concepts/explain-{name}"), explained.stdout);
    }
}

/// The unsafety fixture: what's left alone, what's marked, and what a crash means.
fn unsafety(work: &Path, shown: &mut BTreeMap<String, String>) {
    let run = ran(work, &["run", "fixtures/unsafety"]);
    shown.insert("concepts/unsafe-summary".into(), summary(&run.stdout));
    shown.insert("concepts/unsafe-run".into(), run.stdout);
    shown.insert(
        "concepts/unsafe-mutants".into(),
        ran(work, &["mutants", "fixtures/unsafety"]).stdout,
    );
    let jsonl = ran(work, &["run", "fixtures/unsafety", "--format=jsonl"]);
    let events = events(&jsonl.stdout);
    let started = events
        .iter()
        .find(|e| e["reason"] == "run-started")
        .unwrap();
    shown.insert("concepts/run-started".into(), pretty(started));
    let find = |method: &str, replacement: &str| -> Value {
        events
            .iter()
            .find(|e| {
                e["location"]["method"] == method && e["mutation"]["replacement"] == replacement
            })
            .unwrap()
            .clone()
    };
    shown.insert(
        "concepts/runtime-error-event".into(),
        pretty(&find("checked", "true")),
    );
    shown.insert(
        "concepts/precondition-event".into(),
        pretty(&find("positive", "false")),
    );
    let total = find("total", "-");
    let verify = ran(
        work,
        &["verify", total["id"].as_str().unwrap(), "fixtures/unsafety"],
    );
    shown.insert("concepts/verify-panic".into(), verify.stdout);
    let guarded = find("guarded", "true");
    shown.insert("concepts/crash-event".into(), pretty(&guarded));
    let verify = ran(
        work,
        &[
            "verify",
            guarded["id"].as_str().unwrap(),
            "fixtures/unsafety",
        ],
    );
    shown.insert("concepts/verify-crash".into(), verify.stdout);
}

/// Each operator family's run on the operators fixture, and the card each section shows.
fn operators(work: &Path, shown: &mut BTreeMap<String, String>) {
    let mut runs = BTreeMap::new();
    for (family, operators) in FAMILIES {
        let run = ran(
            work,
            &[
                "run",
                "fixtures/operators",
                "--format=full",
                &format!("--operators={operators}"),
            ],
        );
        runs.insert(*family, run.stdout);
    }
    let found = found_in(work, "operators");
    for (family, function, operator) in CARDS {
        let ids = ids_in(&found, function, operator);
        assert!(!ids.is_empty(), "{operator} has no mutant in {function}()");
        let found = card(&runs[family], &ids)
            .unwrap_or_else(|| panic!("no card for {operator} in {function}(): {}", runs[family]));
        let name = operator.rsplit('/').next().unwrap();
        shown.insert(format!("operators/{family}/{name}"), found);
    }
    // A `?` on a formatter's write is skipped as equivalent, and only that one.
    let jsonl = ran(
        work,
        &[
            "run",
            "fixtures/operators",
            "--format=jsonl",
            "--operators=mutastic/unwrap-try",
        ],
    );
    let skipped: Vec<Value> = events(&jsonl.stdout)
        .into_iter()
        .filter(|e| e["status"] == "Ignored")
        .collect();
    assert_eq!(skipped.len(), 1, "{}", jsonl.stdout);
    assert_eq!(skipped[0]["location"]["method"], "fmt");
    assert_eq!(skipped[0]["skipKind"], "equivalent");
    shown.insert(
        "operators/returns/unwrap-try-skipped".into(),
        pretty(&skipped[0]),
    );
    // A zero check on a `NonZeroU32` made never true is skipped as equivalent, and only that one.
    let jsonl = ran(
        work,
        &[
            "run",
            "fixtures/operators",
            "--format=jsonl",
            "--operators=mutastic/remove-conditional",
        ],
    );
    let share: Vec<Value> = events(&jsonl.stdout)
        .into_iter()
        .filter(|e| e["location"]["method"] == "share")
        .collect();
    let skipped: Vec<&Value> = share.iter().filter(|e| e["status"] == "Ignored").collect();
    assert_eq!((share.len(), skipped.len()), (2, 1), "{}", jsonl.stdout);
    assert_eq!(skipped[0]["mutation"]["replacement"], "false");
    assert_eq!(skipped[0]["skipKind"], "equivalent");
    shown.insert(
        "operators/conditions/remove-conditional-skipped".into(),
        pretty(skipped[0]),
    );
    // A boundary inside a guard that its sides differ is skipped as equivalent, and only that one.
    let jsonl = ran(
        work,
        &[
            "run",
            "fixtures/operators",
            "--format=jsonl",
            "--operators=mutastic/boundary",
        ],
    );
    let is_newer: Vec<Value> = events(&jsonl.stdout)
        .into_iter()
        .filter(|e| e["location"]["method"] == "is_newer")
        .collect();
    let skipped: Vec<&Value> = is_newer
        .iter()
        .filter(|e| e["status"] == "Ignored")
        .collect();
    assert_eq!((is_newer.len(), skipped.len()), (2, 1), "{}", jsonl.stdout);
    assert_eq!(skipped[0]["mutation"]["replacement"], ">=");
    assert_eq!(skipped[0]["skipKind"], "equivalent");
    shown.insert(
        "operators/conditions/boundary-skipped".into(),
        pretty(skipped[0]),
    );
    for operator in LISTED {
        let name = operator.rsplit('/').next().unwrap();
        shown.insert(
            format!("operators/library/{name}"),
            ran(
                work,
                &[
                    "mutants",
                    "fixtures/truth",
                    &format!("--operators={operator}"),
                ],
            )
            .stdout,
        );
    }
}

/// What the troubleshooting page shows: a mutant withdrawn because it doesn't compile, loop
/// counters that never end, code this build doesn't have, and a workspace's root.
fn troubleshooting(work: &Path, shown: &mut BTreeMap<String, String>) {
    let jsonl = ran(
        work,
        &[
            "run",
            "fixtures/operators",
            "--format=jsonl",
            "--operators=mutastic/omit-call",
        ],
    );
    let unviable = events(&jsonl.stdout)
        .into_iter()
        .find(|e| e["status"] == "Ignored")
        .expect("omit-call's to_lowercase() is unviable");
    shown.insert("troubleshooting/unviable-event".into(), pretty(&unviable));
    // Each hanging mutant waits for its time limit, at least 10s: one worker for each of the five,
    // and they wait together, once.
    let loops = ran(
        work,
        &[
            "run",
            "fixtures/loops",
            "--operators=stronger",
            "--workers=5",
        ],
    );
    assert!(
        plain(&loops.stdout).contains("timed out"),
        "{}",
        loops.stdout
    );
    shown.insert("troubleshooting/loops".into(), summary(&loops.stdout));
    // Code for another platform, and behind a feature that's off, is counted, not mutated.
    let platforms = ran(work, &["run", "fixtures/platforms"]);
    let other = if cfg!(windows) { "unix" } else { "windows" };
    assert!(
        plain(&platforms.stdout).contains(&format!(
            "not built here: `cfg(feature = \"simd\")` (2), `cfg({other})`"
        )),
        "{}",
        platforms.stdout
    );
    shown.insert(
        "troubleshooting/platforms".into(),
        summary(&platforms.stdout),
    );
    let root = mutastic(work, &["run", "fixtures/workspace"]);
    assert_eq!(root.code, 2, "{}", root.stderr);
    shown.insert("troubleshooting/workspace".into(), root.stderr);
    // A directory that isn't there, named close to a package: offered, as the command to run. The
    // page shows the directory mutastic looked in as a reader's, not this copy's.
    let missing = mutastic(work, &["run", "fixtures/workspace/shpo"]);
    assert_eq!(missing.code, 2, "{}", missing.stderr);
    let looked = std::fs::canonicalize(work).unwrap().display().to_string();
    shown.insert(
        "troubleshooting/no-directory".into(),
        missing.stderr.replace(&looked, "/home/you/project"),
    );
    // A mistyped option: the close one offered, in the line as typed, before anything runs.
    let mistyped = mutastic(work, &["run", "fixtures/pricing", "--formt=agent"]);
    assert_eq!(mistyped.code, 2, "{}", mistyped.stderr);
    shown.insert("troubleshooting/mistyped".into(), mistyped.stderr);
    red_and_broken(work, shown);
}

/// The error a run stopped with, without the progress before it.
fn stopped_with(stderr: &str) -> String {
    let at = stderr.find("mutastic: ").unwrap_or(0);
    stderr[at..].to_string()
}

/// A command an error offers, run as a person pastes it in `work`, the directory mutastic ran in.
fn pasted(work: &Path, line: &str) -> std::process::Output {
    let words: Vec<&str> = line.trim().split(' ').collect();
    Command::new(words[0])
        .args(&words[1..])
        .current_dir(work)
        .env_remove("CARGO_TARGET_DIR")
        .output()
        .unwrap()
}

/// A test that fails with no mutant in, and a package that doesn't build: each error says where, and
/// the command it ends in does what it says.
fn red_and_broken(work: &Path, shown: &mut BTreeMap<String, String>) {
    let lib = work.join("fixtures/pricing/src/lib.rs");
    let source = std::fs::read_to_string(&lib).unwrap();

    std::fs::write(
        &lib,
        source.replace(
            "assert_eq!(pricing.shipping(39), 5);",
            "assert_eq!(pricing.shipping(39), 4);",
        ),
    )
    .unwrap();
    let red = mutastic(work, &["run", "fixtures/pricing"]);
    assert_eq!(red.code, 2, "{}", red.stderr);
    let said = stopped_with(&red.stderr);
    assert!(said.contains("left: 5, right: 4"), "{said}");
    let rerun = said.lines().last().unwrap();
    let alone = pasted(work, rerun);
    let out = String::from_utf8_lossy(&alone.stdout);
    assert!(
        !alone.status.success()
            && out.contains("test tests::shipping_is_free_from_the_threshold ... FAILED")
            && out.contains("1 failed"),
        "{rerun}: {out}"
    );
    shown.insert("troubleshooting/red".into(), said);

    std::fs::write(
        &lib,
        source.replace("return total / 10;", "return total / \"10\";"),
    )
    .unwrap();
    let broken = mutastic(work, &["run", "fixtures/pricing"]);
    assert_eq!(broken.code, 2, "{}", broken.stderr);
    let said = stopped_with(&broken.stderr);
    // The line is the person's: line 27 of their file, whatever mutastic's copy made of it.
    assert!(said.contains("src/lib.rs, line 27: "), "{said}");
    let rerun = said.lines().last().unwrap();
    let built = pasted(work, rerun);
    // Cargo writes the path as the platform does: `src\lib.rs` on Windows.
    let err = String::from_utf8_lossy(&built.stderr).replace('\\', "/");
    assert!(
        !built.status.success() && err.contains("src/lib.rs:27:"),
        "{rerun}: {err}"
    );
    shown.insert("troubleshooting/broken".into(), said);
    std::fs::write(&lib, source).unwrap();
}

/// The suggested-tests page's card: a test written out for `late_fee`'s boundary.
fn suggested(work: &Path, shown: &mut BTreeMap<String, String>) {
    let run = ran(work, &["run", "fixtures/suggestions", "--report=none"]);
    let ids = ids_of(work, "suggestions", "late_fee", "mutastic/boundary");
    let card = card(&run.stdout, &ids).expect("late_fee's boundary is missed");
    assert!(
        plain(&card).contains("assert_eq!(late_fee(7), 0);"),
        "{card}"
    );
    shown.insert("concepts/suggested-test".into(), card);
}

/// `next` on the ranking fixture, from a first pass: the head of the queue for a person, the same
/// task for an agent, and the events with `--top`, which spreads them over the code.
fn queue(work: &Path, shown: &mut BTreeMap<String, String>) {
    let fixture = work.join("fixtures/ranking");
    let first = |args: &[&str]| {
        forget_last_runs(&fixture);
        let next = mutastic(work, &[&["next", "fixtures/ranking"], args].concat());
        assert_eq!(next.code, 1, "{}{}", next.stdout, next.stderr);
        next.stdout
    };
    let head = first(&[]);
    shown.insert("concepts/next".into(), head.clone());
    shown.insert("commands/next".into(), head);
    shown.insert("concepts/next-agent".into(), first(&["--format=agent"]));
    let top = first(&["--top=3", "--format=jsonl"]);
    for event in events(&top) {
        let errors: Vec<String> = common::schema()
            .iter_errors(&event)
            .map(|e| e.to_string())
            .collect();
        assert!(errors.is_empty(), "{event}\n{}", errors.join("\n"));
    }
    shown.insert("concepts/next-jsonl".into(), top);
}

/// The pages whose code is a test-first cycle on the pricing fixture, in the order they're worked:
/// the second starts where the first ends. The agent page adds no code of its own: it names the
/// first page's mutants, as an agent working that cycle meets them.
const CYCLES: &[&str] = &[
    "test-first.md",
    "test-first-refactor.md",
    "test-first-agent.md",
];

/// A test-first page's code, in order: each block marked `added to` or `changed in`, with what it
/// marks.
fn cycle_steps(page: &str) -> Vec<(String, String)> {
    let text = std::fs::read_to_string(docs().join(page)).unwrap();
    blocks(&text)
        .into_iter()
        .filter_map(|b| match b.marker {
            Some((kind, path)) if kind == "added to" || kind == "changed in" => {
                assert_eq!(path, "fixtures/pricing/src/lib.rs", "{page} changes {path}");
                Some((kind, text[b.text].to_string()))
            }
            _ => None,
        })
        .collect()
}

/// `code`, indented one level, as a line of a module or an `impl` is.
fn indented(code: &str) -> String {
    code.lines()
        .map(|line| match line {
            "" => "\n".to_string(),
            _ => format!("    {line}\n"),
        })
        .collect()
}

/// One of the test-first page's steps, made to the pricing fixture's `src/lib.rs` as the page says:
/// a test is added at the end of the `tests` module, other code at the end of `impl Pricing`, and
/// a change replaces the function of the same name.
fn test_first_step(lib: &Path, (kind, code): &(String, String)) {
    let source = std::fs::read_to_string(lib).unwrap();
    let changed = if kind == "changed in" {
        let signature = code.lines().find(|l| l.starts_with("pub fn ")).unwrap();
        let at = source.find(signature).unwrap();
        let start = source[..at].rfind("\n\n").unwrap() + 2;
        let end = at + source[at..].find("\n    }\n").unwrap() + "\n    }\n".len();
        format!("{}{}{}", &source[..start], indented(code), &source[end..])
    } else if code.contains("#[test]") {
        let close = source.trim_end().rfind('}').unwrap();
        format!("{}\n\n{}}}\n", source[..close].trim_end(), indented(code))
    } else {
        let close = source.find("\n}\n\n#[cfg(test)]").unwrap();
        format!(
            "{}\n\n{}{}",
            &source[..close],
            indented(code).trim_end(),
            &source[close..]
        )
    };
    std::fs::write(lib, changed).unwrap();
}

/// The ids of the pricing fixture's mutants at each step of the test-first pages' cycles: the
/// pages' `verify` commands name some of them, on code the fixture doesn't have until a page adds
/// it, and a later step may take away. Found once in a process: two tests ask, at once, and each
/// copying the fixture into the one directory took the other's copy away as it read it.
fn test_first_ids() -> BTreeSet<String> {
    static IDS: std::sync::OnceLock<BTreeSet<String>> = std::sync::OnceLock::new();
    IDS.get_or_init(|| {
        let dir = scratch("test-first-ids").join("pricing");
        common::fresh(&root().join("fixtures/pricing"), &dir);
        let mut ids = BTreeSet::new();
        for step in CYCLES.iter().flat_map(|page| cycle_steps(page)) {
            test_first_step(&dir.join("src/lib.rs"), &step);
            let project = workspace::read(&dir.display().to_string()).unwrap();
            ids.extend(engine::find_all(&project).mutants.into_iter().map(|m| m.id));
        }
        ids
    })
    .clone()
}

/// `git` in `work`, which must succeed, as a person with a name set runs it.
fn git(work: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(work)
        .env("GIT_AUTHOR_NAME", "docs")
        .env("GIT_AUTHOR_EMAIL", "docs@example.com")
        .env("GIT_COMMITTER_NAME", "docs")
        .env("GIT_COMMITTER_EMAIL", "docs@example.com")
        .output()
        .unwrap()
        .status;
    assert!(status.success(), "git {args:?}");
}

/// `work` made a repository of its own, as a page's clone is, with what's in it committed, so
/// `--since=HEAD` is the next cycle's lines.
fn repository(work: &Path) {
    std::fs::remove_dir_all(work.join(".git")).ok();
    std::fs::write(work.join(".gitignore"), "target\n").unwrap();
    git(work, &["init", "-q"]);
    commit(work);
}

/// The end of a cycle: everything committed, so the commit is the next cycle's `HEAD`.
fn commit(work: &Path) {
    git(work, &["add", "."]);
    git(work, &["commit", "-q", "-m", "cycle"]);
}

/// A run stopped by a failing test, as a cycle's red step makes it: mutastic's message, which
/// names `test`.
fn red(work: &Path, test: &str) -> String {
    let red = mutastic(work, &["run", "fixtures/pricing"]);
    assert_eq!(red.code, 2, "{}{}", red.stdout, red.stderr);
    let message = stopped_with(&red.stderr);
    assert!(plain(&message).contains(test), "{}", red.stderr);
    message
}

/// A cycle's run that misses what its code ran ahead of: the run, and the ids its card's `verify`
/// names, which `page` must show that command for.
fn missed(work: &Path, page: &str) -> (Ran, String) {
    let run = ran(work, &["run", "fixtures/pricing", "--since=HEAD"]);
    assert_eq!(run.code, 1, "{}", run.stdout);
    let done = plain(&run.stdout)
        .lines()
        .find_map(|line| {
            line.trim_start()
                .strip_prefix("done when")
                .map(str::to_string)
        })
        .expect("the cycle's run has a card");
    let ids = done.split_whitespace().nth(2).unwrap().to_string();
    let text = std::fs::read_to_string(docs().join(page)).unwrap();
    assert!(
        text.contains(&format!("mutastic verify {ids} fixtures/pricing\n")),
        "{page}: verify the card's mutants, {ids}"
    );
    (run, ids)
}

/// A cycle's run that catches every mutant on its lines.
fn passed(work: &Path) -> String {
    let run = ran(work, &["run", "fixtures/pricing", "--since=HEAD"]);
    assert_eq!(run.code, 0, "{}", run.stdout);
    run.stdout
}

/// The test-first page's cycle, on the pricing fixture in a repository of its own, from the page's
/// own code: its failing test and the function's stub stop a run, the code that passes it leaves
/// the cycle's edge untested, the card's tests make `verify` pass, and the cycle's run passes after.
fn test_first(work: &Path, shown: &mut BTreeMap<String, String>) {
    let steps = cycle_steps("test-first.md");
    let kinds: Vec<&str> = steps.iter().map(|(kind, _)| kind.as_str()).collect();
    assert_eq!(
        kinds,
        ["added to", "added to", "changed in", "added to"],
        "test-first.md: a test, its stub, the code that passes it, then the card's tests"
    );
    repository(work);
    let lib = work.join("fixtures/pricing/src/lib.rs");
    // Before the cycle, nothing has changed: nothing is built and no test runs.
    let unchanged = ran(work, &["run", "fixtures/pricing", "--since=HEAD"]);
    assert_eq!(unchanged.code, 0, "{}", unchanged.stdout);
    shown.insert("concepts/nothing-to-run".into(), unchanged.stdout);

    test_first_step(&lib, &steps[0]);
    test_first_step(&lib, &steps[1]);
    let message = red(work, "tests::loyal_customers_get_five_off");
    shown.insert("test-first/red".into(), message);

    test_first_step(&lib, &steps[2]);
    let (run, ids) = missed(work, "test-first.md");
    shown.insert("test-first/run".into(), run.stdout);

    test_first_step(&lib, &steps[3]);
    let verify = ran(work, &["verify", &ids, "fixtures/pricing"]);
    assert_eq!(verify.code, 0, "{}", verify.stdout);
    shown.insert("test-first/verify".into(), verify.stdout);
    shown.insert("test-first/again".into(), passed(work));
}

/// One tier's cycle on the refactor page, up to its commit: its failing test, which names the tier,
/// the `if` that passes it, the run that misses its edge, and the card's test with `verify`.
fn tier(work: &Path, tier: &str, steps: &[(String, String)], shown: &mut BTreeMap<String, String>) {
    let lib = work.join("fixtures/pricing/src/lib.rs");
    test_first_step(&lib, &steps[0]);
    let test = cycle_test(&steps[0].1);
    assert!(test.ends_with(&format!("_from_{tier}")), "{test}");
    shown.insert(
        format!("test-first-refactor/red-{tier}"),
        red(work, &format!("tests::{test}")),
    );
    test_first_step(&lib, &steps[1]);
    let (run, ids) = missed(work, "test-first-refactor.md");
    shown.insert(format!("test-first-refactor/run-{tier}"), run.stdout);
    test_first_step(&lib, &steps[2]);
    let verify = ran(work, &["verify", &ids, "fixtures/pricing"]);
    assert_eq!(verify.code, 0, "{}", verify.stdout);
    shown.insert(format!("test-first-refactor/verify-{tier}"), verify.stdout);
}

/// The name of the test a step adds.
fn cycle_test(code: &str) -> &str {
    code.lines()
        .find_map(|line| line.strip_prefix("fn "))
        .and_then(|rest| rest.split('(').next())
        .unwrap()
}

/// The refactor page's cycles, from where the test-first page's ends, committed: for each tier its
/// failing test, the `if` that passes it, the run that misses its edge, and the card's test with
/// `verify`; then, before the last commit, the run that starts the refactor, the refactor, and the
/// run after it, which catches every mutant of the new shape.
fn refactor(work: &Path, shown: &mut BTreeMap<String, String>) {
    let page = "test-first-refactor.md";
    let steps = cycle_steps(page);
    let kinds: Vec<&str> = steps.iter().map(|(kind, _)| kind.as_str()).collect();
    assert_eq!(
        kinds,
        [
            "added to",
            "changed in",
            "added to",
            "added to",
            "changed in",
            "added to",
            "changed in"
        ],
        "{page}: for two tiers a test, the code that passes it and the card's test, then the \
         refactor"
    );
    let lib = work.join("fixtures/pricing/src/lib.rs");
    for step in cycle_steps("test-first.md") {
        test_first_step(&lib, &step);
    }
    repository(work);

    tier(work, "100", &steps[0..3], shown);
    passed(work);
    commit(work);
    tier(work, "250", &steps[3..6], shown);

    shown.insert("test-first-refactor/before".into(), passed(work));
    test_first_step(&lib, &steps[6]);
    shown.insert("test-first-refactor/after".into(), passed(work));
}

/// `mutastic` in `work` as an agent's hook runs it: the hook's input on stdin, and no colour, as
/// the agent reads it.
fn hooked(work: &Path, args: &[&str]) -> Ran {
    use std::io::Write as _;
    let mut child = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args(args)
        .current_dir(work)
        .env_remove("MUTASTIC_AGENT")
        .env_remove("FORCE_COLOR")
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
        .write_all(br#"{"session_id":"docs"}"#)
        .unwrap();
    let output = child.wait_with_output().unwrap();
    Ran {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// The agent page's cycle, as the agent and its gate see it: the test-first page's code on the
/// pricing fixture in a repository of its own, with the gate opened as the plugin's hook opens it,
/// the agent format's run on the cycle's lines once the code passes, and the gate's check once the
/// card's tests are in. The page's `verify` is the test-first page's.
fn test_first_agent(work: &Path, shown: &mut BTreeMap<String, String>) {
    let steps = cycle_steps("test-first.md");
    repository(work);
    let lib = work.join("fixtures/pricing/src/lib.rs");
    let opened = hooked(work, &["gate", "open", "fixtures/pricing", "--hook=claude"]);
    assert_eq!(opened.code, 0, "{}{}", opened.stdout, opened.stderr);
    assert!(opened.stdout.contains("gated from"), "{}", opened.stdout);
    shown.insert("test-first-agent/open".into(), opened.stdout);

    for step in &steps[..3] {
        test_first_step(&lib, step);
    }
    let run = ran(
        work,
        &["run", "fixtures/pricing", "--since=HEAD", "--format=agent"],
    );
    assert_eq!(run.code, 1, "{}", run.stdout);
    shown.insert("test-first-agent/run".into(), run.stdout);

    test_first_step(&lib, &steps[3]);
    let check = mutastic(work, &["gate", "check", "fixtures/pricing"]);
    assert_eq!(check.code, 0, "{}{}", check.stdout, check.stderr);
    assert!(
        plain(&check.stdout).starts_with("mutastic gate: passed"),
        "{}",
        check.stdout
    );
    shown.insert("test-first-agent/check".into(), check.stdout);
}

/// The coverage page, under `docs`, and the tests its steps change: the checkout fixture's, which
/// check too little until the page changes them.
const COVERAGE_PAGE: &str = "coverage-and-mutants.md";
const CHECKOUT_TESTS: &str = "fixtures/checkout/tests/checkout.rs";

/// The coverage page's steps, in order: each block marked `changed in` or `added to`, with what it
/// marks.
fn coverage_steps() -> Vec<(String, String)> {
    let text = std::fs::read_to_string(docs().join(COVERAGE_PAGE)).unwrap();
    blocks(&text)
        .into_iter()
        .filter_map(|b| match b.marker {
            Some((kind, path)) if kind == "added to" || kind == "changed in" => {
                assert_eq!(path, CHECKOUT_TESTS, "{COVERAGE_PAGE} changes {path}");
                Some((kind, text[b.text].to_string()))
            }
            _ => None,
        })
        .collect()
}

/// One of the coverage page's steps, made to the checkout fixture's tests as the page says: tests
/// added go at the end of the file, and each test a change shows replaces the one of its name.
fn coverage_step(tests: &Path, (kind, code): &(String, String)) {
    let mut source = std::fs::read_to_string(tests).unwrap();
    if kind == "added to" {
        source = format!("{}\n\n{code}", source.trim_end());
    } else {
        for test in code.trim_end().split("\n\n") {
            let name = cycle_test(test);
            let at = source.find(&format!("fn {name}(")).unwrap();
            let start = source[..at].rfind("#[test]").unwrap();
            let end = at + source[at..].find("\n}\n").unwrap() + "\n}".len();
            source.replace_range(start..end, test);
        }
    }
    std::fs::write(tests, source).unwrap();
}

/// `cargo llvm-cov`'s summary of the checkout fixture's tests, as the coverage page runs it, from a
/// fresh build: lcov's, which counts lines, functions and branches, and not regions, whose count
/// changes from one compiler to the next. Where it isn't installed, nothing: CI installs it, and sets
/// `MUTASTIC_DOCS_COVERAGE`, which makes it a failure to be without it.
fn llvm_cov(work: &Path) -> Option<String> {
    std::fs::remove_dir_all(work.join("fixtures/checkout/target/llvm-cov-target")).ok();
    let output = Command::new("cargo")
        .args([
            "llvm-cov",
            "--manifest-path",
            "fixtures/checkout/Cargo.toml",
            "--lcov",
            "--summary-only",
            "--remap-path-prefix",
        ])
        .current_dir(work)
        .env_remove("CARGO_TARGET_DIR")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        assert!(
            std::env::var_os("MUTASTIC_DOCS_COVERAGE").is_none(),
            "cargo llvm-cov: {stderr}"
        );
        eprintln!(
            "cargo llvm-cov didn't run, so the coverage page's coverage isn't checked: {stderr}"
        );
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The coverage page's example. The checkout fixture's tests check too little, and the page's
/// check the results: `cargo llvm-cov` measures the same coverage for both, and mutastic misses
/// mutants of the first that the second catches, all but the boundary. The edge's tests catch that,
/// and `stronger` finds what every test buying one item hides, which the page's last test catches.
fn coverage(work: &Path, shown: &mut BTreeMap<String, String>) {
    let steps = coverage_steps();
    let kinds: Vec<&str> = steps.iter().map(|(kind, _)| kind.as_str()).collect();
    assert_eq!(
        kinds,
        ["changed in", "added to", "added to"],
        "{COVERAGE_PAGE}: the tests that check the results, the edge's, then the quantity's"
    );
    let tests = work.join(CHECKOUT_TESTS);
    let weak = llvm_cov(work);
    let run = ran(work, &["run", "fixtures/checkout", "--format=concise"]);
    assert_eq!(run.code, 1, "{}", run.stdout);
    shown.insert("coverage-and-mutants/weak".into(), run.stdout);
    forget_last_runs(&work.join("fixtures/checkout"));
    let full = ran(work, &["run", "fixtures/checkout"]);
    let found = found_in(work, "checkout");
    for (name, operator) in [
        ("arithmetic", "mutastic/math"),
        ("returns", "mutastic/return-value"),
        ("calls", "mutastic/remove-void-call"),
    ] {
        let ids = ids_in(&found, "pay", operator);
        let missed = card(&full.stdout, &ids)
            .unwrap_or_else(|| panic!("no card for {operator} in pay(): {}", full.stdout));
        shown.insert(format!("coverage-and-mutants/{name}"), missed);
    }

    coverage_step(&tests, &steps[0]);
    let strong = llvm_cov(work);
    if let (Some(weak), Some(strong)) = (weak, strong) {
        assert_eq!(weak, strong, "the two sets of tests' coverage");
        // Every line and function: as many hit as found.
        for (found, hit) in [("LF", "LH"), ("FNF", "FNH")] {
            let count = |key: &str| {
                weak.lines()
                    .find_map(|line| line.strip_prefix(&format!("{key}:")))
                    .unwrap()
                    .to_string()
            };
            assert_eq!(count(found), count(hit), "{weak}");
        }
        shown.insert("coverage-and-mutants/llvm-cov-weak".into(), weak);
        shown.insert("coverage-and-mutants/llvm-cov-strong".into(), strong);
    }
    forget_last_runs(&work.join("fixtures/checkout"));
    // Above the threshold, so it passes, with the one card.
    let run = ran(work, &["run", "fixtures/checkout"]);
    assert_eq!(run.code, 0, "{}", run.stdout);
    let boundary = ids_in(&found, "discount", "mutastic/boundary");
    assert!(card(&run.stdout, &boundary).is_some(), "{}", run.stdout);
    assert_eq!(
        plain(&run.stdout).matches("[✗ MISSED]").count(),
        1,
        "{}",
        run.stdout
    );
    shown.insert("coverage-and-mutants/strong".into(), run.stdout);

    coverage_step(&tests, &steps[1]);
    forget_last_runs(&work.join("fixtures/checkout"));
    let run = ran(work, &["run", "fixtures/checkout"]);
    assert_eq!(run.code, 0, "{}", run.stdout);
    shown.insert("coverage-and-mutants/edges".into(), run.stdout);

    shown.insert(
        "coverage-and-mutants/stronger-mutants".into(),
        ran(
            work,
            &["mutants", "fixtures/checkout", "--operators=stronger"],
        )
        .stdout,
    );
    let run = ran(work, &["run", "fixtures/checkout", "--operators=stronger"]);
    assert_eq!(run.code, 0, "{}", run.stdout);
    let kept = ids_in(&found, "pay", "mutastic/keep-operand");
    let done = plain(&run.stdout)
        .lines()
        .find_map(|line| {
            line.trim_start()
                .strip_prefix("done when")
                .map(str::to_string)
        })
        .expect("the stronger run has a card");
    let ids = done.split_whitespace().nth(2).unwrap().to_string();
    assert!(kept.contains(&ids), "{ids} is keep-operand's, in pay()");
    let text = std::fs::read_to_string(docs().join(COVERAGE_PAGE)).unwrap();
    assert!(
        text.contains(&format!("mutastic verify {ids} fixtures/checkout\n")),
        "{COVERAGE_PAGE}: verify the stronger card's mutant, {ids}"
    );
    shown.insert("coverage-and-mutants/stronger".into(), run.stdout);

    coverage_step(&tests, &steps[2]);
    let verify = ran(work, &["verify", &ids, "fixtures/checkout"]);
    assert_eq!(verify.code, 0, "{}", verify.stdout);
    shown.insert("coverage-and-mutants/stronger-verify".into(), verify.stdout);
}

/// Runs the pages show as a Unix build makes them: code and tests under `cfg(unix)` are in it, and
/// a Windows build leaves them out, so there they're run but the pages' copies are kept.
const UNIX_BUILDS: &[&str] = &[
    "concepts/unsafe-run",
    "concepts/unsafe-summary",
    "troubleshooting/platforms",
];

/// What each section of the docs shows of mutastic's output is what it prints now. Each section is
/// a test of its own, since what one section runs reaches code another doesn't.
fn check(shown: &BTreeMap<String, String>) {
    let output = docs().join("output");
    let mut stale = Vec::new();
    for (name, text) in shown {
        if cfg!(windows) && UNIX_BUILDS.contains(&name.as_str()) {
            continue;
        }
        let file = output.join(format!("{name}.ansi"));
        let kept = std::fs::read_to_string(&file).unwrap_or_default();
        if steady(&kept) != steady(text) {
            if writing() {
                std::fs::create_dir_all(file.parent().unwrap()).unwrap();
                std::fs::write(&file, text).unwrap();
            } else {
                let (kept, now) = (steady(&kept), steady(text));
                let differs = kept
                    .lines()
                    .zip(now.lines())
                    .find(|(a, b)| a != b)
                    .map_or("(one is longer)".to_string(), |(a, b)| {
                        format!("kept: {a}\n  now:  {b}")
                    });
                stale.push(format!("docs/output/{name}.ansi\n  {differs}"));
            }
        }
    }
    assert!(
        stale.is_empty(),
        "mutastic prints something else now: MUTASTIC_DOCS_WRITE=1 cargo test --test docs\n{}",
        stale.join("\n")
    );
}

/// A section's captures, run in its own copy of the fixtures it names, then checked.
fn section(name: &str, fixtures: &[&str], run: fn(&Path, &mut BTreeMap<String, String>)) {
    let work = work(name, fixtures);
    let mut shown = BTreeMap::new();
    run(&work, &mut shown);
    check(&shown);
}

#[test]
fn the_docs_show_what_getting_started_prints() {
    section("getting-started", &["pricing"], getting_started);
}

/// The errors page, under `docs`.
const ERRORS_PAGE: &str = "reference/errors.md";

/// An error's code, the words that cause it, and the environment they need.
type Error = (
    &'static str,
    &'static [&'static str],
    &'static [(&'static str, &'static str)],
);

/// Each error the errors page shows: what triggers it, from `work`; the environment it needs; and
/// its code, which its `run-failed` event must carry.
const ERRORS: &[Error] = &[
    ("no-directory", &["run", "fixtures/pricng"], &[]),
    ("no-project", &["run", "empty"], &[]),
    ("workspace-root", &["run", "fixtures/workspace"], &[]),
    (
        "no-cargo",
        &["run", "fixtures/pricing"],
        &[("CARGO", "/nowhere/cargo")],
    ),
    ("unknown-command", &["rnu", "fixtures/pricing"], &[]),
    (
        "unknown-option",
        &["run", "fixtures/pricing", "--formt=agent"],
        &[],
    ),
    (
        "option-value",
        &["run", "fixtures/pricing", "--vocabulary=clasic"],
        &[],
    ),
    (
        "option-not-taken",
        &["verify", "mb2f8ae57", "fixtures/pricing", "--threshold=70"],
        &[],
    ),
    (
        "extra-words",
        &["mutants", "fixtures/pricing", "fixtures/workspace"],
        &[],
    ),
    ("needs-id", &["verify", "fixtures/pricing"], &[]),
    (
        "unknown-id",
        &["verify", "mb2f8ae58", "fixtures/pricing"],
        &[],
    ),
    ("config", &["run", "configured"], &[]),
    (
        "environment",
        &["run", "fixtures/pricing"],
        &[("MUTASTIC_VOCABULARY", "xyz")],
    ),
    (
        "no-baseline",
        &["run", "fixtures/pricing", "--fail-on-new"],
        &[],
    ),
    ("git", &["run", "fixtures/pricing", "--since=main"], &[]),
    (
        "shards",
        &[
            "run",
            "fixtures/pricing",
            "--from-shards=fixtures/workspace",
        ],
        &[],
    ),
    ("both-editions", &["run", "both"], &[]),
];

/// `said` as a Unix machine prints it, run in `/home/you/project` where it ran in `looked`: the
/// page's copies are of a Unix run, and on Windows a path mutastic joins has `\` in it.
fn as_unix(said: &str, looked: &str) -> String {
    let said = said.replace(looked, "/home/you/project");
    if cfg!(windows) {
        said.replace('\\', "/")
    } else {
        said
    }
}

/// The errors page: each error as stderr has it, and, in the jsonl format, as the `run-failed`
/// event with its code, valid in the schema. A command an error gives as `exact` is run, and must
/// get past that error: a printed command that doesn't work is worse than none.
fn errors(work: &Path, shown: &mut BTreeMap<String, String>) {
    std::fs::create_dir_all(work.join("empty")).unwrap();
    std::fs::create_dir_all(work.join("configured")).unwrap();
    common::fresh(&root().join("fixtures/pricing"), &work.join("configured"));
    std::fs::write(
        work.join("configured/mutastic.yaml"),
        "rulez:\n  - skip: pricing::**\n    reason: generated\n",
    )
    .unwrap();
    std::fs::create_dir_all(work.join("both")).unwrap();
    std::fs::write(work.join("both/Cargo.toml"), "[package]\nname = \"both\"\n").unwrap();
    std::fs::write(work.join("both/pom.xml"), "<project/>\n").unwrap();
    let validator = common::schema();
    let looked = std::fs::canonicalize(work).unwrap().display().to_string();
    let mut codes = BTreeSet::new();
    for (code, args, env) in ERRORS {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mutastic"));
        command
            .args(*args)
            .current_dir(work)
            .env("MUTASTIC_FORMAT", "jsonl")
            .env("NO_COLOR", "1")
            .env_remove("FORCE_COLOR")
            .env_remove("MUTASTIC_AGENT");
        for (key, value) in *env {
            command.env(key, value);
        }
        let output = command.output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(output.status.code(), Some(2), "{code}: {stderr}");
        let event: Value = serde_json::from_str(stdout.lines().last().unwrap_or_default())
            .unwrap_or_else(|_| panic!("{code}: no event last: {stdout}"));
        assert!(validator.is_valid(&event), "{code}: {event}");
        assert_eq!(event["reason"], "run-failed", "{code}");
        assert_eq!(event["code"], *code, "{event}");
        let said = stopped_with(&stderr);
        assert_eq!(event["rendered"], said, "{code}");
        for help in event["help"].as_array().unwrap() {
            let line = help["command"].as_str().unwrap();
            if help["applicability"] != "exact" || !line.starts_with("mutastic ") {
                continue;
            }
            let words: Vec<&str> = line.split(' ').skip(1).collect();
            let mut again = Command::new(env!("CARGO_BIN_EXE_mutastic"));
            again
                .args(&words)
                .current_dir(work)
                .env("MUTASTIC_FORMAT", "jsonl")
                .env("MUTASTIC_PROGRESS", "none");
            for (key, value) in *env {
                again.env(key, value);
            }
            let next = again.output().unwrap();
            let last = String::from_utf8_lossy(&next.stdout);
            let after: Value =
                serde_json::from_str(last.lines().last().unwrap_or("{}")).unwrap_or(Value::Null);
            assert!(
                after["code"] != *code,
                "{code}: `{line}` meets the same error: {}",
                String::from_utf8_lossy(&next.stderr)
            );
        }
        codes.insert(code.to_string());
        if *code == "unknown-option" {
            // The events page's example of the event itself.
            // Its path is slashed, without the `\\?\` Windows puts before it.
            let line = stdout.lines().last().unwrap_or_default();
            let path = looked.replace('\\', "/");
            let path = path.strip_prefix("//?/").unwrap_or(&path);
            shown.insert(
                "errors/event".into(),
                format!("{}\n", line.replace(path, "/home/you/project")),
            );
        }
        shown.insert(format!("errors/{code}"), as_unix(&said, &looked));
    }
    // The page has an entry for every code mutastic gives, and only those.
    let page = std::fs::read_to_string(docs().join(ERRORS_PAGE)).unwrap();
    let entries: BTreeSet<String> = page
        .lines()
        .filter_map(|line| line.strip_prefix("## `"))
        .filter_map(|line| line.strip_suffix('`'))
        .map(str::to_string)
        .collect();
    let given: BTreeSet<String> = mutastic::failure::Code::ALL
        .iter()
        .map(|code| code.name().to_string())
        .collect();
    assert_eq!(entries, given, "docs/reference/errors.md's entries");
    // Each entry's command is the one run above: the page's commands fail on purpose, so the
    // checks of every other page's commands pass over them, and rest on this.
    for (code, args, env) in ERRORS {
        let heading = format!("## `{code}`\n");
        let at = page
            .find(&heading)
            .unwrap_or_else(|| panic!("no entry for {code}"));
        let rest = &page[at..];
        let block = blocks(rest)
            .into_iter()
            .find(|block| block.language == "sh")
            .unwrap_or_else(|| panic!("{code}: no command"));
        let typed: String = env
            .iter()
            .map(|(key, value)| format!("{key}={value} "))
            .chain([format!("mutastic {}", args.join(" "))])
            .collect();
        assert_eq!(rest[block.text].trim(), typed, "{code}'s command");
    }
    assert!(codes.is_subset(&given));
}

#[test]
fn the_docs_show_what_the_errors_print() {
    section("errors", &["pricing", "workspace"], errors);
}

#[test]
fn the_docs_show_what_the_reference_prints() {
    section("reference", &["pricing", "configured"], reference);
}

#[test]
fn the_docs_show_what_unsafe_code_prints() {
    section("unsafety", &["unsafety"], unsafety);
}

#[test]
fn the_docs_show_what_the_operators_print() {
    section("operators", &["operators", "truth"], operators);
}

#[test]
fn the_docs_show_what_troubleshooting_prints() {
    section(
        "troubleshooting",
        &["operators", "loops", "platforms", "workspace", "pricing"],
        troubleshooting,
    );
}

#[test]
fn the_docs_show_what_suggestions_print() {
    section("suggested", &["suggestions"], suggested);
}

#[test]
fn the_docs_show_what_next_prints() {
    section("queue", &["ranking"], queue);
}

#[test]
fn the_docs_show_what_a_test_first_cycle_prints() {
    section("test-first", &["pricing"], test_first);
}

#[test]
fn the_docs_show_what_a_refactor_prints() {
    section("test-first-refactor", &["pricing"], refactor);
}

#[test]
fn the_docs_show_what_an_agent_sees_test_first() {
    section("test-first-agent", &["pricing"], test_first_agent);
}

#[test]
fn the_docs_show_what_coverage_misses() {
    section("coverage", &["checkout"], coverage);
}

/// The lines of `text` without the indent they share.
fn dedent(lines: &[&str]) -> Vec<String> {
    let indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|line| line.get(indent..).unwrap_or("").to_string())
        .collect()
}

#[test]
fn every_rust_example_is_compiled() {
    for page in pages() {
        let text = std::fs::read_to_string(&page).unwrap();
        for block in blocks(&text) {
            let body = &text[block.text.clone()];
            let place = format!("{} at byte {}", page.display(), block.text.start);
            match &block.marker {
                Some((kind, path)) if kind == "from" => {
                    let fixture = path
                        .strip_prefix("fixtures/")
                        .and_then(|rest| rest.split('/').next())
                        .unwrap_or_default();
                    assert!(
                        BUILT.contains(&fixture),
                        "{place}: quotes {path}, which no docs run builds"
                    );
                    let source = std::fs::read_to_string(root().join(path))
                        .unwrap_or_else(|_| panic!("{place}: no {path}"));
                    let file: Vec<&str> = source.lines().collect();
                    let quoted: Vec<&str> = body.lines().collect();
                    let want = dedent(&quoted);
                    let found = (0..file.len().saturating_sub(quoted.len()) + 1)
                        .any(|at| dedent(&file[at..at + quoted.len()]) == want);
                    assert!(found, "{place}: isn't in {path} word for word:\n{body}");
                }
                Some((kind, path)) if kind == "added to" || kind == "changed in" => {
                    let catching = kind == "added to"
                        && page.starts_with(docs().join("operators"))
                        && path.starts_with("fixtures/operators/src/");
                    let cycle = CYCLES.iter().any(|name| page == docs().join(name));
                    let started = kind == "added to" && page == docs().join("getting-started.md");
                    let covered = page == docs().join(COVERAGE_PAGE) && path == CHECKOUT_TESTS;
                    assert!(
                        path == "fixtures/pricing/src/lib.rs" && (started || cycle)
                            || catching
                            || covered,
                        "{place}: only the getting-started test, the test-first cycles' code, the \
                         coverage page's tests and the operators' tests that catch them are added \
                         and run"
                    );
                }
                _ => assert!(
                    block.language != "rust",
                    "{place}: a Rust block needs <!-- from: … -->, <!-- added to: … --> or \
                     <!-- changed in: … -->"
                ),
            }
        }
    }
}

/// Each operator's section ends with the test that catches it. Every one is added to the tests of
/// its file in a copy of the operators fixture, and one `verify` runs the mutants of every card the
/// sections show: each must be caught, so what a page says catches a mutant does.
#[test]
fn every_test_that_catches_a_card_does() {
    let work = scratch("catching");
    let fixture = work.join("fixtures/operators");
    common::fresh(&root().join("fixtures/operators"), &fixture);
    let mut added: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for page in pages() {
        let text = std::fs::read_to_string(&page).unwrap();
        for block in blocks(&text) {
            if let Some((kind, path)) = &block.marker {
                if kind == "added to" && path.starts_with("fixtures/operators/") {
                    added
                        .entry(path.clone())
                        .or_default()
                        .push(text[block.text].to_string());
                }
            }
        }
    }
    let count: usize = added.values().map(Vec::len).sum();
    assert!(
        count >= CARDS.len(),
        "{count} tests that catch, for {} cards",
        CARDS.len()
    );
    for (path, tests) in &added {
        let source = std::fs::read_to_string(root().join(path)).unwrap();
        let close = source.trim_end().rfind('}').unwrap();
        let mut with = source[..close].trim_end().to_string();
        for test in tests {
            with.push('\n');
            for line in test.lines() {
                with.push('\n');
                if !line.is_empty() {
                    with.push_str("    ");
                    with.push_str(line);
                }
            }
        }
        workspace::write_if_changed(&work.join(path), &format!("{with}\n}}\n")).unwrap();
    }
    let mut ids = BTreeSet::new();
    for (family, _, operator) in CARDS {
        let name = operator.rsplit('/').next().unwrap();
        let card = plain(
            &std::fs::read_to_string(docs().join(format!("output/operators/{family}/{name}.ansi")))
                .unwrap(),
        );
        let named = card
            .lines()
            .find_map(|line| line.trim_start().strip_prefix("done when"))
            .and_then(|rest| rest.split_whitespace().nth(2))
            .unwrap_or_else(|| panic!("{family}/{name} has no card"));
        ids.extend(named.split(',').map(str::to_string));
    }
    let ids: Vec<String> = ids.into_iter().collect();
    let verify = mutastic(&work, &["verify", &ids.join(","), "fixtures/operators"]);
    assert_eq!(verify.code, 0, "{}{}", verify.stdout, verify.stderr);
}

/// GitHub's anchor for a heading: lowercase, spaces to `-`, other punctuation gone.
fn anchor(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

fn anchors(text: &str) -> BTreeSet<String> {
    let mut fenced = false;
    let mut found = BTreeSet::new();
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        } else if !fenced && line.starts_with('#') {
            found.insert(anchor(line.trim_start_matches('#')));
        }
    }
    found
}

/// Every link `explain`, a card and an event give to an operator's docs lands on its section: the
/// heading on its family's page, the family the operators fixture puts it in.
#[test]
fn every_operator_links_to_its_section_in_the_docs() {
    let page = format!("{}operators/", mutastic::commands::DOCS);
    let mut broken = Vec::new();
    for &operator in mutastic::sites::ALL {
        if operator == mutastic::sites::RETURN_VALUE_EXTENDED {
            continue;
        }
        let Some(url) = mutastic::operator_docs::url(operator) else {
            broken.push(format!("{operator}: no link"));
            continue;
        };
        let (family, fragment) = url
            .strip_prefix(&page)
            .and_then(|rest| rest.split_once("/#"))
            .unwrap();
        let text = std::fs::read_to_string(docs().join(format!("operators/{family}.md"))).unwrap();
        if !anchors(&text).contains(fragment) {
            broken.push(format!("{operator}: {url} (no such heading)"));
        }
        let listed = FAMILIES
            .iter()
            .find(|(_, operators)| operators.split(',').any(|listed| listed == operator));
        if listed.is_some_and(|(listed, _)| *listed != family) {
            broken.push(format!("{operator}: {url} (its fixture is in {listed:?})"));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

/// The docs site's Rust section, which isn't published yet: until it is, a page links to its copy
/// here, which GitHub shows, and the site maps that link to its own page.
const UNPUBLISHED: &str = "https://docs.mutastic.io/rust/";

#[test]
fn every_link_between_pages_resolves() {
    let mut broken = Vec::new();
    for page in pages() {
        let text = std::fs::read_to_string(&page).unwrap();
        let mut rest = text.as_str();
        while let Some(at) = rest.find("](") {
            rest = &rest[at + 2..];
            let target = &rest[..rest.find(')').unwrap()];
            if target.starts_with(UNPUBLISHED) {
                broken.push(format!(
                    "{}: {target} (not published yet: link the page here)",
                    page.display()
                ));
                continue;
            }
            if target.starts_with("http") || target.starts_with("mailto:") {
                continue;
            }
            let (path, fragment) = target.split_once('#').unwrap_or((target, ""));
            let file = if path.is_empty() {
                page.clone()
            } else {
                page.parent().unwrap().join(path)
            };
            if !file.exists() {
                broken.push(format!("{}: {target}", page.display()));
                continue;
            }
            if !fragment.is_empty() && file.extension().is_some_and(|e| e == "md") {
                let linked = std::fs::read_to_string(&file).unwrap();
                if !anchors(&linked).contains(fragment) {
                    broken.push(format!("{}: {target} (no such heading)", page.display()));
                }
            }
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

/// A command line's words as the shell gives them: split at spaces, except inside double quotes,
/// which are taken away. Enough for the commands the docs show.
fn shell_words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    for character in line.chars() {
        match character {
            '"' => quoted = !quoted,
            ' ' if !quoted => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            _ => word.push(character),
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

#[test]
fn every_command_shown_parses_and_names_real_mutants() {
    let mutant_id = regex::Regex::new(r"^m[0-9a-f]{8}$").unwrap();
    let mut wrong = Vec::new();
    let mut found: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    // The test-first pages name mutants of the code their cycles write, which the fixture hasn't
    // got.
    let cycle = test_first_ids();
    for page in CYCLES {
        found.insert(
            format!("{}:fixtures/pricing", docs().join(page).display()),
            cycle.clone(),
        );
    }
    for page in pages() {
        // The errors page's commands fail on purpose; the errors test runs each one.
        if page == docs().join(ERRORS_PAGE) {
            continue;
        }
        let text = std::fs::read_to_string(&page).unwrap();
        for block in blocks(&text) {
            if block.language != "sh" {
                continue;
            }
            for line in text[block.text.clone()].lines() {
                let line = line.split(" #").next().unwrap().trim();
                let words = shell_words(line);
                let args = match words.first().map(String::as_str) {
                    Some("mutastic") => &words[1..],
                    Some("cargo") if words.get(1).is_some_and(|w| w == "mutastic") => &words[2..],
                    _ => continue,
                };
                if args.iter().any(|arg| arg.contains('<')) {
                    continue;
                }
                if let Err(error) = mutastic::cli::check(args) {
                    wrong.push(format!("{}: {line}: {error}", page.display()));
                    continue;
                }
                let positional: Vec<&String> =
                    args[1..].iter().filter(|a| !a.starts_with("--")).collect();
                let (named, dir) = match args[0].as_str() {
                    // The first word's ids, split by commas, then each word that's an id too, then
                    // the directory.
                    "verify" => {
                        let more = positional
                            .iter()
                            .skip(1)
                            .take_while(|word| mutant_id.is_match(word))
                            .count();
                        let ids: Vec<&str> = positional[..positional.len().min(1 + more)]
                            .iter()
                            .map(|s| s.as_str())
                            .collect();
                        (
                            (!ids.is_empty()).then(|| ids.join(",")),
                            positional.get(1 + more).map(|s| s.to_string()),
                        )
                    }
                    "run" => (
                        args.iter()
                            .find_map(|a| a.strip_prefix("--only=").map(str::to_string)),
                        positional.first().map(|s| s.to_string()),
                    ),
                    _ => (None, None),
                };
                let (Some(named), Some(dir)) = (named, dir) else {
                    continue;
                };
                let mine = format!("{}:{dir}", page.display());
                let key = if found.contains_key(&mine) {
                    mine
                } else {
                    dir.clone()
                };
                let ids = found.entry(key).or_insert_with(|| {
                    let project = workspace::read(&root().join(&dir).display().to_string())
                        .unwrap_or_else(|e| panic!("{}: {line}: {e}", page.display()));
                    engine::find_all(&project)
                        .mutants
                        .into_iter()
                        .map(|m| m.id)
                        .collect()
                });
                for id in named.split(',') {
                    if !ids.contains(id) {
                        wrong.push(format!(
                            "{}: {line}: {id} isn't a mutant of {dir}",
                            page.display()
                        ));
                    }
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Every mutant id a page names in its words, outside the output it quotes, is a mutant of one
/// of the fixtures: `m00000000` alone is named as one that isn't.
#[test]
fn every_id_in_the_words_is_a_mutant() {
    let mut ids: BTreeSet<String> = BTreeSet::from(["m00000000".to_string()]);
    for fixture in BUILT {
        let dir = root().join("fixtures").join(fixture);
        let project = workspace::read(&dir.display().to_string()).unwrap();
        ids.extend(engine::find_all(&project).mutants.into_iter().map(|m| m.id));
    }
    // The test-first pages name mutants of the code their cycles write, which the fixture hasn't
    // got.
    let cycle: BTreeSet<String> = ids.union(&test_first_ids()).cloned().collect();
    // The errors page names ids that are no mutant's on purpose, in the commands it runs.
    let wrong_on_purpose: BTreeSet<String> = ids
        .iter()
        .cloned()
        .chain(
            ERRORS
                .iter()
                .flat_map(|(_, args, _)| args.iter().map(|a| a.to_string())),
        )
        .collect();
    let pattern = regex::Regex::new(r"\bm[0-9a-f]{8}\b").unwrap();
    let mut wrong = Vec::new();
    for page in pages() {
        let known = if CYCLES.iter().any(|name| page == docs().join(name)) {
            &cycle
        } else if page == docs().join(ERRORS_PAGE) {
            &wrong_on_purpose
        } else {
            &ids
        };
        let mut text = std::fs::read_to_string(&page).unwrap();
        for block in blocks(&text).into_iter().rev() {
            if block
                .marker
                .as_ref()
                .is_some_and(|(kind, _)| kind == "output")
            {
                text.replace_range(block.text, "");
            }
        }
        for found in pattern.find_iter(&text) {
            if !known.contains(found.as_str()) {
                wrong.push(format!("{}: {}", page.display(), found.as_str()));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "ids that name no mutant: {}",
        wrong.join(", ")
    );
}

/// A card says what its mutant does. For every mutant the operators fixture's tests miss, this
/// writes the card's change into a copy of its file, as the card claims it, puts every copy in one
/// crate, and runs the fixture's tests in each: they must pass, as they did with the mutant.
#[test]
fn every_missed_card_on_the_operators_fixture_is_true() {
    let work = scratch("claims");
    let fixture = work.join("fixtures/operators");
    common::fresh(&root().join("fixtures/operators"), &fixture);
    let jsonl = ran(
        &work,
        &[
            "run",
            "fixtures/operators",
            "--format=jsonl",
            "--operators=extended",
        ],
    );
    let missed: BTreeSet<String> = events(&jsonl.stdout)
        .into_iter()
        .filter(|e| e["status"] == "Survived")
        .map(|e| e["id"].as_str().unwrap().to_string())
        .collect();
    assert!(missed.len() > 40, "{} missed", missed.len());
    let project = workspace::read(&fixture.display().to_string()).unwrap();
    let found = engine::find_all(&project);
    let allow = "#[allow(warnings, unconditional_panic, arithmetic_overflow)]";
    let mut claims = String::new();
    let mut named = BTreeMap::new();
    for mutant in found.mutants.iter().filter(|m| missed.contains(&m.id)) {
        let file = found.files.iter().find(|f| f.path == mutant.file).unwrap();
        let (range, with) = mutant.claim();
        let mut text = file.text.clone();
        text.replace_range(range, &with);
        claims.push_str(&format!(
            "{allow}\nmod v{} {{\n{text}\n}}\n",
            mutant.ordinal
        ));
        named.insert(format!("v{}", mutant.ordinal), mutant);
    }
    let dir = work.join("crate");
    workspace::write_if_changed(
        &dir.join("Cargo.toml"),
        "[package]\nname = \"claims\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .unwrap();
    workspace::write_if_changed(&dir.join("src/lib.rs"), &claims).unwrap();
    let output = Command::new(workspace::cargo())
        .args(["test", "--lib"])
        .current_dir(&dir)
        .env("CARGO_TARGET_DIR", dir.join("target"))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let wrong: Vec<String> = stdout
        .lines()
        .filter_map(|line| line.strip_prefix("test ")?.strip_suffix(" ... FAILED"))
        .map(|test| {
            let mutant = named[test.split("::").next().unwrap()];
            format!(
                "{} {} at {}:{}: the card says `{}`, and with it {test} fails",
                mutant.id,
                mutant.operator,
                mutant.file.display(),
                mutant.line,
                mutant.claim().1
            )
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The table of build files on the editions page is how mutastic routes: each row's file, alone in
/// a directory, picks that row's edition, and every build file mutastic reads has a row.
#[test]
fn the_editions_table_is_how_mutastic_routes() {
    use mutastic::edition::{self, Edition};
    let page = std::fs::read_to_string(docs().join("editions.md")).unwrap();
    let row = regex::Regex::new(r"(?m)^\| `(?<file>[^`]+)` \| (?<edition>Rust|Java) \|$").unwrap();
    let mut listed = BTreeSet::new();
    for found in row.captures_iter(&page) {
        let file = found["file"].to_string();
        let dir = scratch("routing").join(file.replace('.', "-"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(&file), "").unwrap();
        let routed = edition::route(&dir).unwrap().edition;
        let expected = if &found["edition"] == "Rust" {
            Edition::Rust
        } else {
            Edition::Java
        };
        assert_eq!(routed, expected, "{file}");
        listed.insert(file);
    }
    let read: BTreeSet<String> = edition::JVM_BUILDS
        .iter()
        .chain(&[edition::CARGO_BUILD])
        .map(|file| file.to_string())
        .collect();
    assert_eq!(listed, read);
}

/// Every setting the installer and the Java edition's fetch read is on their page.
#[test]
fn every_setting_read_is_documented() {
    let setting = regex::Regex::new(r"MUTASTIC_[A-Z_]+").unwrap();
    for (source, page) in [
        ("scripts/install.sh", "install.md"),
        ("scripts/install.ps1", "install.md"),
        ("crates/mutastic/src/java.rs", "editions.md"),
    ] {
        let read = std::fs::read_to_string(root().join(source)).unwrap();
        let shown = std::fs::read_to_string(docs().join(page)).unwrap();
        for name in setting.find_iter(&read) {
            assert!(
                shown.contains(&format!("`{}`", name.as_str())),
                "{source} reads {}, which {page} doesn't name",
                name.as_str()
            );
        }
    }
}

/// `docs/commands.json`, the docs site's command reference, and `docs/reference/commands.md`, the
/// repository's, are what the parser's declarations write, and every id their examples name is a
/// mutant of the pricing fixture.
#[test]
fn the_commands_reference_is_the_parsers_declarations() {
    let now = mutastic::commands::json();
    for (name, text) in [
        ("commands.json", &now),
        ("reference/commands.md", &mutastic::commands::markdown()),
    ] {
        let file = docs().join(name);
        let kept = std::fs::read_to_string(&file).unwrap_or_default();
        if kept != *text {
            assert!(
                writing(),
                "docs/{name} isn't what the CLI declares: MUTASTIC_DOCS_WRITE=1 cargo test --test docs"
            );
            std::fs::write(&file, text).unwrap();
        }
    }
    let pricing = root().join("fixtures/pricing").display().to_string();
    let ids: BTreeSet<String> = engine::find_all(&workspace::read(&pricing).unwrap())
        .mutants
        .into_iter()
        .map(|m| m.id)
        .collect();
    let pattern = regex::Regex::new(r"\bm[0-9a-f]{8}\b").unwrap();
    for found in pattern.find_iter(&now) {
        assert!(
            ids.contains(found.as_str()),
            "{} isn't a mutant of fixtures/pricing",
            found.as_str()
        );
    }
}

/// The `[lints...]` tables of a manifest, each line of each kept as it is, in order.
fn lint_tables(manifest: &str) -> BTreeMap<String, Vec<String>> {
    let mut tables: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut table = None;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            table = line.contains("lints").then(|| line.to_string());
            if let Some(table) = &table {
                tables.entry(table.clone()).or_default();
            }
        } else if let (Some(table), false) = (&table, line.is_empty() || line.starts_with('#')) {
            tables.get_mut(table).unwrap().push(line.to_string());
        }
    }
    tables
}

/// Every Rust file under `dir`, `target/` left out.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() && !path.ends_with("target") {
                stack.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }
    files
}

/// Unsafe code lives in one crate, `mutastic-terminal`, the audit boundary. The workspace forbids
/// it, which no `#[allow]` can undo, and every other member takes the workspace's lints. The
/// terminal crate copies them, unsafe code denied but for its `ffi` modules, with the lints an
/// audit asks for; and there, every `unsafe` block says why it's sound.
#[test]
fn unsafe_code_stays_in_mutastic_terminal() {
    let workspace = lint_tables(&std::fs::read_to_string(root().join("Cargo.toml")).unwrap());
    let rust = &workspace["[workspace.lints.rust]"];
    assert!(rust.iter().any(|line| line == r#"unsafe_code = "forbid""#));
    let terminal = root().join("crates/mutastic-terminal");
    let allow = regex::Regex::new(r"(?:allow|expect)\(\s*unsafe_code").unwrap();
    let on_ffi =
        regex::Regex::new(r#"#\[allow\(\s*unsafe_code,\s*reason = "[^"]+"\s*\)\]\s*mod ffi \{"#)
            .unwrap();
    for entry in std::fs::read_dir(root().join("crates")).unwrap() {
        let member = entry.unwrap().path();
        let manifest = std::fs::read_to_string(member.join("Cargo.toml")).unwrap();
        let tables = lint_tables(&manifest);
        let files = rust_files(&member);
        if member == terminal {
            let own = &tables["[lints.rust]"];
            assert!(own.iter().any(|line| line == r#"unsafe_code = "deny""#));
            assert!(own
                .iter()
                .any(|line| line == r#"unsafe_op_in_unsafe_fn = "deny""#));
            let clippy = &tables["[lints.clippy]"];
            for lint in [
                "undocumented_unsafe_blocks",
                "multiple_unsafe_ops_per_block",
                "missing_safety_doc",
            ] {
                assert!(
                    clippy.contains(&format!(r#"{lint} = "deny""#)),
                    "mutastic-terminal doesn't deny {lint}"
                );
            }
            for line in &workspace["[workspace.lints.clippy]"] {
                assert!(
                    clippy.contains(line),
                    "mutastic-terminal's lints don't have the workspace's `{line}`"
                );
            }
            for file in files {
                let text = std::fs::read_to_string(&file).unwrap();
                assert_eq!(
                    text.matches("unsafe {").count(),
                    text.matches("// SAFETY: ").count(),
                    "{}: an unsafe block without its reason",
                    file.display()
                );
                assert_eq!(
                    allow.find_iter(&text).count(),
                    on_ffi.find_iter(&text).count(),
                    "{}: unsafe code is allowed only on an `ffi` module",
                    file.display()
                );
            }
        } else {
            assert_eq!(
                tables.get("[lints]").map(Vec::as_slice),
                Some(&["workspace = true".to_string()][..]),
                "{} doesn't take the workspace's lints, which forbid unsafe code",
                member.display()
            );
            for file in files {
                let text = std::fs::read_to_string(&file).unwrap();
                assert!(
                    !allow.is_match(&text),
                    "{} allows unsafe code: it belongs in crates/mutastic-terminal",
                    file.display()
                );
            }
        }
    }
}

/// Every variable of the environment the command reads is on the reference's environment page.
#[test]
fn every_variable_read_is_declared() {
    let read = regex::Regex::new(r#"env::var(?:_os)?\("(?<name>[A-Z_]+)"\)"#).unwrap();
    let declared: BTreeSet<&str> = mutastic::commands::ENVIRONMENT
        .iter()
        .map(|entry| entry.name.split('=').next().unwrap())
        .collect();
    for source in [
        "crates/mutastic/src/cli.rs",
        "crates/mutastic/src/java.rs",
        "crates/mutastic/src/workspace.rs",
        "crates/mutastic/src/progress.rs",
        "crates/mutastic-terminal/src/lib.rs",
    ] {
        let text = std::fs::read_to_string(root().join(source)).unwrap();
        for found in read.captures_iter(&text) {
            let name = &found["name"];
            assert!(
                name == "HOME" || declared.contains(name),
                "{source} reads {name}, which commands::ENVIRONMENT doesn't declare"
            );
        }
    }
}
