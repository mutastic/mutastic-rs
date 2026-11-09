//! The docs are kept true. Every output block in `docs/` and the README is what mutastic prints on
//! one of the fixtures: it's captured in `docs/output/`, and this fails while a fresh run prints
//! anything else.
//! Every Rust block is either an excerpt of a fixture these runs build, word for word, or the test
//! the getting-started page adds, which this adds and runs. Every link between pages resolves, every
//! `mutastic` command a page shows parses, and every id it names is a mutant of its fixture. And
//! every missed mutant's card on the operators fixture says something true: written into the
//! source as the card claims it, the fixture's tests still pass.
//!
//! To write the docs' copy again, after a change to what mutastic prints:
//! `MUTASTIC_DOCS_WRITE=1 cargo test --test docs`, then read the diff.

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
    "operators",
    "unsafety",
    "loops",
    "truth",
    "platforms",
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
/// mutastic's own workspace, kept from run to run so builds there stay warm.
fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join("mutastic-rs-docs").join(name)
}

/// Where the fixtures are copied to run, as `fixtures/<name>`, so commands print what a person
/// types at the repository's root, and the repository's own fixtures are left alone.
fn work() -> PathBuf {
    let work = scratch("docs");
    for name in BUILT.iter().chain(&["workspace"]) {
        sync(
            &root().join("fixtures").join(name),
            &work.join("fixtures").join(name),
        );
    }
    work
}

/// Copies a tree, writing only what changed, so the copy's build stays warm.
fn sync(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if name == "target" {
            continue;
        }
        let (source, destination) = (from.join(&name), to.join(&name));
        if entry.file_type().unwrap().is_dir() {
            sync(&source, &destination);
        } else {
            let bytes = std::fs::read(&source).unwrap();
            if std::fs::read(&destination).ok() != Some(bytes.clone()) {
                std::fs::write(&destination, bytes).unwrap();
            }
        }
    }
}

struct Ran {
    code: i32,
    stdout: String,
    stderr: String,
}

/// `mutastic` in `work`, as a person at a terminal runs it there (cards, unless `--format` says
/// otherwise), without colour.
fn mutastic(work: &Path, args: &[&str]) -> Ran {
    let output = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args(args)
        .current_dir(work)
        .env("MUTASTIC_FORMAT", "full")
        .env_remove("MUTASTIC_AGENT")
        .env_remove("FORCE_COLOR")
        .env("NO_COLOR", "1")
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

/// What changes from run to run or machine to machine: timings, when the run started, the
/// compiler's version, the number of workers, which follows the cores, and how many tests ran,
/// which follows timing too: a mutant tries the binary whose tests were fastest first, and stops at
/// the first that fails.
fn steady(text: &str) -> String {
    let rules = [
        (r#""testsRun": ?\d+"#, r#""testsRun":…"#),
        (r"Test runs  \d+", "Test runs  …"),
        (r"Start at  \d\d:\d\d:\d\d", "Start at  …"),
        (r"\b\d+m\d\ds\b", "…"),
        (r"\b\d+(\.\d+)?(ms|s)\b", "…"),
        (r#"Ms": ?\d+"#, r#"Ms":…"#),
        (
            r#""name": ?"rustc", ?"version": ?"[^"]*""#,
            r#""name":"rustc","version":…"#,
        ),
        (r"rustc \S+", "rustc …"),
        (r"\b\d+ workers?\b", "… workers"),
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

/// The card on `output` whose mutants include one of `ids`, from its headline to its last command.
fn card(output: &str, ids: &BTreeSet<String>) -> Option<String> {
    let lines: Vec<&str> = output.lines().collect();
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
                return Some(lines[first..=end].join("\n") + "\n");
            }
        }
    }
    None
}

/// The summary at the end of a run, from its `Mutants` line.
fn summary(output: &str) -> String {
    let at = output.find("   Mutants").unwrap();
    output[at..].to_string()
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
    let project =
        workspace::read(&work.join("fixtures").join(fixture).display().to_string()).unwrap();
    let found = engine::find_all(&project);
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
    shown.insert("concepts/run-progress".into(), run.stderr);
    // The line as mutastic's copy holds it, every mutant of it behind the switch.
    let copy =
        std::fs::read_to_string(work.join("fixtures/pricing/target/mutastic/copy/src/lib.rs"))
            .unwrap();
    let line = copy.lines().nth(25).unwrap();
    assert!(line.contains("total >= 100"), "{line}");
    shown.insert("concepts/schema-line".into(), format!("{}\n", line.trim()));
    let id = run
        .stdout
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
    shown.insert("getting-started/verify".into(), verify.stdout);
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
        .skip_while(|line| !line.trim_start().starts_with("Score "))
        .take_while(|line| !line.trim_start().starts_with("Next:"))
        .map(|line| format!("{line}\n"))
        .collect();
    assert!(verdict.contains("[PASS]"), "{output}");
    format!("{card}\n{verdict}")
}

/// The pricing fixture in each format but `full`, the `mutants` listing, and the help.
fn reference(work: &Path, shown: &mut BTreeMap<String, String>) {
    for format in ["concise", "agent", "jsonl"] {
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
        shown.insert(format!("reference/format-{format}"), run.stdout);
    }
    shown.insert(
        "reference/mutants".into(),
        ran(work, &["mutants", "fixtures/pricing"]).stdout,
    );
    shown.insert("reference/help".into(), ran(work, &["--help"]).stdout);
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
    shown.insert("concepts/run-started".into(), pretty(&events[0]));
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
    for (family, function, operator) in CARDS {
        let ids = ids_of(work, "operators", function, operator);
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
    // Each hanging mutant waits for its time limit: side by side, they wait together.
    let loops = ran(
        work,
        &[
            "run",
            "fixtures/loops",
            "--operators=stronger",
            "--workers=4",
        ],
    );
    assert!(loops.stdout.contains("timed out"), "{}", loops.stdout);
    shown.insert("troubleshooting/loops".into(), summary(&loops.stdout));
    // Code for another platform, and behind a feature that's off, is counted, not mutated.
    let platforms = ran(work, &["run", "fixtures/platforms"]);
    assert!(
        platforms
            .stdout
            .contains("not built here: `cfg(feature = \"simd\")` (2), `cfg(windows)`"),
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
}

#[test]
fn the_docs_show_what_mutastic_prints() {
    let work = work();
    let mut shown = BTreeMap::new();
    getting_started(&work, &mut shown);
    reference(&work, &mut shown);
    unsafety(&work, &mut shown);
    operators(&work, &mut shown);
    troubleshooting(&work, &mut shown);

    let output = docs().join("output");
    let mut stale = Vec::new();
    for (name, text) in &shown {
        let file = output.join(format!("{name}.txt"));
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
                stale.push(format!("docs/output/{name}.txt\n  {differs}"));
            }
        }
    }
    assert!(
        stale.is_empty(),
        "mutastic prints something else now: MUTASTIC_DOCS_WRITE=1 cargo test --test docs\n{}",
        stale.join("\n")
    );

    // Each page's output blocks are the kept copies, and every copy is on a page.
    let mut used = BTreeSet::new();
    for page in pages() {
        let mut text = std::fs::read_to_string(&page).unwrap();
        let mut edits = Vec::new();
        for block in blocks(&text) {
            let Some((kind, name)) = &block.marker else {
                continue;
            };
            if kind != "output" {
                continue;
            }
            assert!(
                shown.contains_key(name),
                "{}: no run here makes {name}",
                page.display()
            );
            used.insert(name.clone());
            let kept = std::fs::read_to_string(output.join(format!("{name}.txt"))).unwrap();
            if text[block.text.clone()] != kept {
                edits.push((block.text.clone(), kept));
            }
        }
        if edits.is_empty() {
            continue;
        }
        assert!(
            writing(),
            "{} shows output other than docs/output's: MUTASTIC_DOCS_WRITE=1 cargo test --test docs",
            page.display()
        );
        for (range, kept) in edits.into_iter().rev() {
            text.replace_range(range, &kept);
        }
        std::fs::write(&page, text).unwrap();
    }
    let unused: Vec<&String> = shown.keys().filter(|name| !used.contains(*name)).collect();
    assert!(unused.is_empty(), "output no page shows: {unused:?}");
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
                Some((kind, path)) if kind == "added to" => {
                    assert_eq!(
                        path, "fixtures/pricing/src/lib.rs",
                        "{place}: only the getting-started test is added and run"
                    );
                }
                _ => assert!(
                    block.language != "rust",
                    "{place}: a Rust block needs <!-- from: … --> or <!-- added to: … -->"
                ),
            }
        }
    }
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

#[test]
fn every_link_between_pages_resolves() {
    let mut broken = Vec::new();
    for page in pages() {
        let text = std::fs::read_to_string(&page).unwrap();
        let mut rest = text.as_str();
        while let Some(at) = rest.find("](") {
            rest = &rest[at + 2..];
            let target = &rest[..rest.find(')').unwrap()];
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

#[test]
fn every_command_shown_parses_and_names_real_mutants() {
    let mut wrong = Vec::new();
    let mut found: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for page in pages() {
        let text = std::fs::read_to_string(&page).unwrap();
        for block in blocks(&text) {
            if block.language != "sh" {
                continue;
            }
            for line in text[block.text.clone()].lines() {
                let line = line.split(" #").next().unwrap().trim();
                let words: Vec<String> = line.split_whitespace().map(str::to_string).collect();
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
                    "verify" => (
                        positional.first().map(|s| s.to_string()),
                        positional.get(1).map(|s| s.to_string()),
                    ),
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
                let ids = found.entry(dir.clone()).or_insert_with(|| {
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
    let pattern = regex::Regex::new(r"\bm[0-9a-f]{8}\b").unwrap();
    let mut wrong = Vec::new();
    for page in pages() {
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
            if !ids.contains(found.as_str()) {
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
    sync(&root().join("fixtures/operators"), &fixture);
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
