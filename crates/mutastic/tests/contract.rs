//! The Rust edition keeps the editions' contract, `mutastic.contract/v1`, which lives in the Java
//! repository beside the events schema (`docs/public/contract/v1.json`): its command line,
//! its help's layout, the error code and exit code of each error case, the human summary's rows,
//! the agent format's lines, the events' key paths and order, and mutation.json's key paths. Each
//! is found by running the built binary on a copy of `fixtures/pricing`, as a person does, and
//! compared with the contract. A difference the contract doesn't record fails, naming the item and
//! what to do; so does a recorded one that has gone, so the list only shrinks for real.
//!
//! The contract is the one at the commit `contract.json` pins, fetched once and kept under the
//! target directory. `MUTASTIC_CONTRACT` names a file to check against instead: a working copy
//! while a change to the contract is being written, or mutastic main's, as the weekly drift job
//! does. CONTRIBUTING.md says how it works, and how to add an option to both editions.
//!
//! The Java edition runs the same checks on its own binary and sample (its `ContractTest`), and
//! neither shares the other's code, so an edition in another language can pass them too.

// On Linux and macOS: the contract is about what mutastic says, which is the same on Windows, and
// the checks compare paths as they print there.
#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    reason = "a helper here asserts as the tests it serves do: a value that isn't there fails the test"
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;
use sha2::Digest;

mod common;

const EDITION: &str = "rust";

/// The fixture's missed mutant.
const ID: &str = "mb2f8ae57";

/// Where the contract is pinned: the Java repository, the commit, and the file in it.
fn pin() -> Value {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contract.json");
    serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

/// The contract: `MUTASTIC_CONTRACT`'s file, or the pinned commit's, fetched once.
fn contract() -> &'static Value {
    static CONTRACT: OnceLock<Value> = OnceLock::new();
    CONTRACT.get_or_init(|| {
        let text = match std::env::var_os("MUTASTIC_CONTRACT") {
            Some(path) => std::fs::read_to_string(path).unwrap(),
            None => pinned(),
        };
        serde_json::from_str(&text).unwrap()
    })
}

fn pinned() -> String {
    let pin = pin();
    let commit = pin["commit"].as_str().unwrap();
    let kept = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("contract-{commit}.json"));
    if let Ok(text) = std::fs::read_to_string(&kept) {
        return text;
    }
    let url = format!(
        "{}/{commit}/{}",
        pin["raw"].as_str().unwrap(),
        pin["path"].as_str().unwrap()
    );
    let output = Command::new("curl")
        .args(["--fail", "--silent", "--show-error", "--location", &url])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "couldn't fetch the contract from {url}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    std::fs::write(&kept, &text).unwrap();
    text
}

/// One thing found to differ from the contract: its item, `option:--report:choice:xml`, and what
/// was seen.
struct Found {
    item: String,
    what: String,
}

fn found(item: impl Into<String>, what: impl Into<String>) -> Found {
    Found {
        item: item.into(),
        what: what.into(),
    }
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|array| {
            array
                .iter()
                .map(|each| each.as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// An item pattern, `*` matching anything.
fn glob(pattern: &str) -> Regex {
    let parts: Vec<String> = pattern.split('*').map(regex::escape).collect();
    Regex::new(&format!("^{}$", parts.join(".*"))).unwrap()
}

fn names(patterns: &[String], item: &str) -> bool {
    patterns.iter().any(|pattern| glob(pattern).is_match(item))
}

/// What fails: each item found that no difference for this edition names, and each difference for
/// this edition with items on one of `surfaces` (`help`, `error`) none of which was found. Java's
/// `EditionContract` judges the same way.
fn unrecorded(found: &[Found], surfaces: &[&str]) -> Vec<String> {
    let file = "docs/public/contract/v1.json in mutastic/mutastic";
    let mine: Vec<&Value> = contract()["differences"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|difference| {
            strings(&difference["editions"])
                .iter()
                .any(|e| e == EDITION)
        })
        .collect();
    let mut failures = Vec::new();
    for each in found {
        if !mine
            .iter()
            .any(|difference| names(&strings(&difference["items"]), &each.item))
        {
            failures.push(format!(
                "{}: {}. Make the Rust edition match, or record it under differences in {file} \
                 (engine with a reason, dated with an until, pending with an issue), then move \
                 contract.json's pin to that commit",
                each.item, each.what
            ));
        }
    }
    for difference in mine {
        let here: Vec<String> = strings(&difference["items"])
            .into_iter()
            .filter(|item| {
                surfaces
                    .iter()
                    .any(|surface| item.starts_with(&format!("{surface}:")))
            })
            .collect();
        if !here.is_empty() && !found.iter().any(|each| names(&here, &each.item)) {
            failures.push(format!(
                "{} now matches for rust: take rust out of its editions, or the entry out, in \
                 {file}, then move contract.json's pin to that commit",
                difference["id"].as_str().unwrap()
            ));
        }
    }
    failures
}

fn assert_kept(found: &[Found], surfaces: &[&str]) {
    let failures = unrecorded(found, surfaces);
    assert!(
        failures.is_empty(),
        "the Rust edition differs from the editions' contract:\n  - {}",
        failures.join("\n  - ")
    );
}

/// Where the checks run: a directory with no project in or above it, the fixture's copy, in git,
/// and an empty directory.
struct Place {
    here: PathBuf,
    fixture: PathBuf,
    empty: PathBuf,
}

fn place() -> &'static Place {
    static PLACE: OnceLock<Place> = OnceLock::new();
    PLACE.get_or_init(|| {
        let (here, _) = common::slot(&common::slots(), "contract");
        let fixture = common::own_copy("pricing", "contract-fixture");
        if !fixture.join(".git").exists() {
            git(&fixture, &["init", "-q", "-b", "main"]);
            std::fs::write(fixture.join(".gitignore"), "target/\n").unwrap();
            git(&fixture, &["add", "-A"]);
            git(
                &fixture,
                &[
                    "-c",
                    "user.name=t",
                    "-c",
                    "user.email=t@t",
                    "commit",
                    "-qm",
                    "start",
                ],
            );
        }
        let empty = here.join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        Place {
            here,
            fixture,
            empty,
        }
    })
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

struct Run {
    exit: i32,
    out: String,
    err: String,
}

/// mutastic as a person runs it, from a directory with no project, and with only `env` of
/// mutastic's own variables set.
fn mutastic(args: &[String], env: &[(String, String)]) -> Run {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mutastic"));
    command
        .args(args)
        .current_dir(&place().here)
        .env("NO_COLOR", "1")
        .env("MUTASTIC_PROGRESS", "none");
    for (name, _) in std::env::vars() {
        if name.starts_with("MUTASTIC_") || name.starts_with("CLAUDE") || name == "CI" {
            command.env_remove(name);
        }
    }
    command.env("MUTASTIC_PROGRESS", "none");
    for (name, value) in env {
        command.env(name, value);
    }
    let output = command.output().unwrap();
    Run {
        exit: output.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&output.stdout).into_owned(),
        err: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn run(args: &[&str]) -> Run {
    mutastic(
        &args.iter().map(ToString::to_string).collect::<Vec<_>>(),
        &[],
    )
}

fn fixture() -> String {
    place().fixture.display().to_string()
}

/// A full run as a person at a terminal sees it, which the summary, report and events checks
/// share: the events' run follows it, so it has a last run to compare with.
fn full() -> &'static Run {
    static FULL: OnceLock<Run> = OnceLock::new();
    FULL.get_or_init(|| {
        mutastic(
            &["run".to_string(), fixture()],
            &[("MUTASTIC_FORMAT".to_string(), "full".to_string())],
        )
    })
}

fn model() -> Value {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/commands.json");
    serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

fn by_name(array: &Value) -> BTreeMap<String, &Value> {
    array
        .as_array()
        .unwrap()
        .iter()
        .map(|each| (each["name"].as_str().unwrap().to_string(), each))
        .collect()
}

fn text<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field].as_str().unwrap_or("")
}

/// Each of the contract's names this edition hasn't, and each of its own the contract hasn't.
fn named(found: &mut Vec<Found>, surface: &str, contract: &[String], mine: &[String]) {
    for name in contract.iter().filter(|name| !mine.contains(name)) {
        found.push(self::found(
            format!("{surface}:{name}"),
            "the contract has it, and this edition doesn't",
        ));
    }
    for name in mine.iter().filter(|name| !contract.contains(name)) {
        found.push(self::found(
            format!("{surface}:{name}"),
            "this edition has it, and the contract doesn't",
        ));
    }
}

#[test]
fn the_contract_names_this_edition_and_its_ids_are_one_each() {
    let editions = strings(&contract()["editions"]);
    assert!(editions.iter().any(|edition| edition == EDITION));
    let mut ids: Vec<&str> = contract()["differences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|difference| difference["id"].as_str().unwrap())
        .collect();
    let all = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), all, "a difference's id is used twice");
}

#[test]
fn the_schemas_are_the_contracts() {
    let schema = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/schema");
    let mut found = Vec::new();
    for (name, copy) in [
        ("events", schema.join("events-v1.json")),
        (
            "report",
            schema.join("report/mutation-testing-report-schema.json"),
        ),
        ("sarif", schema.join("sarif-2.1.0.json")),
    ] {
        let hash = sha2::Sha256::digest(std::fs::read(&copy).unwrap());
        let hash: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
        if hash != text(&contract()["schemas"][name], "sha256") {
            found.push(self::found(
                format!("schema:{name}"),
                format!("tests/schema's copy has sha256 {hash}"),
            ));
        }
    }
    assert_kept(&found, &["schema"]);
}

#[test]
fn the_command_line_is_the_contracts() {
    let model = model();
    let mut found = Vec::new();
    commands(&mut found, &by_name(&model["commands"]));
    options(&mut found, &by_name(&model["options"]));
    let environment: Vec<String> = model["environment"]
        .as_array()
        .unwrap()
        .iter()
        .map(|variable| {
            text(variable, "name")
                .split('=')
                .next()
                .unwrap()
                .to_string()
        })
        .collect();
    named(
        &mut found,
        "env",
        &strings(&contract()["environment"]),
        &environment,
    );
    let exits: Vec<String> = contract()["exitCodes"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    let mine: Vec<String> = model["exitCodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|exit| text(exit, "name").to_string())
        .collect();
    named(&mut found, "exit", &exits, &mine);
    assert_kept(
        &found,
        &[
            "command", "synopsis", "takes", "order", "option", "env", "exit",
        ],
    );
}

fn commands(found: &mut Vec<Found>, model: &BTreeMap<String, &Value>) {
    let shared = by_name(&contract()["commands"]);
    for command in contract()["commands"].as_array().unwrap() {
        let name = text(command, "name");
        match model.get(name) {
            None => found.push(self::found(
                format!("command:{name}"),
                "the contract's command isn't in commands.json",
            )),
            Some(mine) => self::command(found, command, mine),
        }
    }
    for name in model.keys().filter(|name| !shared.contains_key(*name)) {
        found.push(self::found(
            format!("command:{name}"),
            "a command the contract doesn't have",
        ));
    }
}

fn command(found: &mut Vec<Found>, command: &Value, mine: &Value) {
    let name = text(command, "name");
    let mut synopsis = text(mine, "synopsis").to_string();
    for slot in contract()["engineSlots"].as_array().unwrap() {
        synopsis = synopsis.replace(&format!(" {}", text(slot, "slot")), "");
    }
    if synopsis != text(command, "synopsis") {
        found.push(self::found(
            format!("synopsis:{name}"),
            format!(
                "`{synopsis}`, the contract's `{}`",
                text(command, "synopsis")
            ),
        ));
    }
    let takes = strings(&command["options"]);
    let mine_takes = strings(&mine["options"]);
    for option in takes.iter().filter(|option| !mine_takes.contains(option)) {
        found.push(self::found(
            format!("takes:{name}:{option}"),
            format!("{name} doesn't take {option}"),
        ));
    }
    for option in mine_takes.iter().filter(|option| !takes.contains(option)) {
        found.push(self::found(
            format!("takes:{name}:{option}"),
            format!("{name} takes {option}, which the contract doesn't give it"),
        ));
    }
    let in_mine: Vec<&String> = mine_takes.iter().filter(|o| takes.contains(o)).collect();
    let in_contract: Vec<&String> = takes.iter().filter(|o| mine_takes.contains(o)).collect();
    if in_mine != in_contract {
        found.push(self::found(
            format!("order:{name}"),
            format!("options in the order {in_mine:?}, the contract's {in_contract:?}"),
        ));
    }
}

fn options(found: &mut Vec<Found>, model: &BTreeMap<String, &Value>) {
    let shared = by_name(&contract()["options"]);
    for option in contract()["options"].as_array().unwrap() {
        let name = text(option, "name");
        match model.get(name) {
            None => found.push(self::found(
                format!("option:{name}"),
                "the contract's option isn't in commands.json",
            )),
            Some(mine) => self::option(found, option, mine),
        }
    }
    for name in model.keys().filter(|name| !shared.contains_key(*name)) {
        found.push(self::found(
            format!("option:{name}"),
            "an option the contract doesn't have",
        ));
    }
}

fn option(found: &mut Vec<Found>, option: &Value, mine: &Value) {
    let name = format!("option:{}", text(option, "name"));
    for field in ["kind", "value"] {
        if text(option, field) != text(mine, field) {
            found.push(self::found(
                format!("{name}:{field}"),
                format!(
                    "`{}`, the contract's `{}`",
                    text(mine, field),
                    text(option, field)
                ),
            ));
        }
    }
    named(
        found,
        &format!("{name}:choice"),
        &strings(&option["choices"]),
        &strings(&mine["choices"]),
    );
    let environment = strings(&option["environment"]);
    let mine_environment: Vec<String> = text(mine, "environment")
        .split(',')
        .map(str::trim)
        .filter(|each| !each.is_empty())
        .map(ToString::to_string)
        .collect();
    if environment != mine_environment {
        found.push(self::found(
            format!("{name}:environment"),
            format!("{mine_environment:?}, the contract's {environment:?}"),
        ));
    }
    if let Some(default) = option["default"].as_str() {
        let said = format!("{default} unless set");
        if !text(mine, "help")
            .replace(", unless set", " unless set")
            .contains(&said)
        {
            found.push(self::found(
                format!("{name}:default"),
                format!("its help doesn't say `{said}`"),
            ));
        }
    }
}

#[test]
fn the_help_is_laid_out_as_the_contract_says() {
    let help = &contract()["help"];
    let mut found = Vec::new();
    top_help(&mut found, &help["top"]);
    let none = run(&[]);
    if i64::from(none.exit) != help["none"]["exit"].as_i64().unwrap()
        || !none.out.is_empty()
        || none.err.is_empty()
    {
        found.push(self::found(
            "help:none",
            format!(
                "with no arguments it exits {}, {} characters on stdout and {} on stderr",
                none.exit,
                none.out.len(),
                none.err.len()
            ),
        ));
    }
    let model = model();
    let commands = by_name(&model["commands"]);
    for command in contract()["commands"].as_array().unwrap() {
        let name = text(command, "name");
        let options = commands
            .get(name)
            .map(|mine| strings(&mine["options"]))
            .unwrap_or_default();
        command_help(&mut found, &help["command"], name, &options);
    }
    assert_kept(&found, &["help"]);
}

fn finds(shape: &Value, field: &str, line: &str) -> bool {
    Regex::new(text(shape, field)).unwrap().is_match(line)
}

fn top_help(found: &mut Vec<Found>, top: &Value) {
    let run = run(&["--help"]);
    let lines: Vec<&str> = run.out.lines().collect();
    let (Some(first), Some(last)) = (lines.first(), lines.last()) else {
        found.push(self::found(
            "help:top:exit",
            "--help prints nothing on stdout",
        ));
        return;
    };
    if run.exit != 0 {
        found.push(self::found(
            "help:top:exit",
            format!("--help exits {}", run.exit),
        ));
    }
    if !finds(top, "first", first) {
        found.push(self::found(
            "help:top:tagline",
            format!("its first line is `{first}`"),
        ));
    }
    for section in strings(&top["sections"]) {
        if !lines.contains(&section.as_str()) {
            found.push(self::found(
                format!("help:top:section:{section}"),
                format!("no `{section}` section"),
            ));
        }
    }
    if !finds(top, "last", last) {
        found.push(self::found(
            "help:top:docs",
            format!("its last line is `{last}`"),
        ));
    }
}

fn command_help(found: &mut Vec<Found>, shape: &Value, command: &str, options: &[String]) {
    let mut args: Vec<&str> = command.split(' ').collect();
    args.push("--help");
    let run = run(&args);
    let lines: Vec<&str> = run.out.lines().collect();
    let item = format!("help:{command}");
    let Some(first) = lines.first() else {
        found.push(self::found(
            format!("{item}:exit"),
            format!("it exits {} with nothing on stdout", run.exit),
        ));
        return;
    };
    if run.exit != 0 {
        found.push(self::found(
            format!("{item}:exit"),
            format!("it exits {}", run.exit),
        ));
    }
    if !finds(shape, "first", first) {
        found.push(self::found(
            format!("{item}:usage"),
            format!("its first line is `{first}`"),
        ));
    }
    if !lines.iter().any(|line| finds(shape, "exits", line)) {
        found.push(self::found(format!("{item}:exits"), "no `Exits` paragraph"));
    }
    for section in strings(&shape["sections"]) {
        if !lines.contains(&section.as_str()) {
            found.push(self::found(
                format!("{item}:section:{section}"),
                format!("no `{section}` section"),
            ));
        }
    }
    if !lines.iter().any(|line| finds(shape, "docs", line)) {
        found.push(self::found(format!("{item}:docs"), "no `Docs:` link"));
    }
    let width = usize::try_from(shape["width"].as_u64().unwrap()).unwrap();
    let longest = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    if longest > width {
        found.push(self::found(
            format!("{item}:width"),
            format!("a line of {longest} columns, over {width}"),
        ));
    }
    for option in options {
        let listed = Regex::new(&format!(r"^\s+{}\b", regex::escape(option))).unwrap();
        if !lines.iter().any(|line| listed.is_match(line)) {
            found.push(self::found(
                format!("{item}:option:{option}"),
                format!("{option} is in commands.json, not in the help"),
            ));
        }
    }
}

#[test]
fn each_error_ends_as_the_contract_says() {
    let mut found = Vec::new();
    for case in contract()["errorCases"].as_array().unwrap() {
        error_case(&mut found, case);
    }
    assert_kept(&found, &["error"]);
}

fn error_case(found: &mut Vec<Found>, case: &Value) {
    let dir = if case["setup"].is_string() {
        red_baseline()
    } else {
        fixture()
    };
    let args: Vec<String> = strings(&case["args"])
        .iter()
        .map(|arg| {
            arg.replace("{dir}", &dir)
                .replace("{id}", ID)
                .replace("{empty}", &place().empty.display().to_string())
        })
        .collect();
    let env: Vec<(String, String)> = case["env"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, value)| (name.clone(), value.as_str().unwrap().to_string()))
        .collect();
    let run = mutastic(&args, &env);
    let item = format!("error:{}", text(case, "id"));
    let exit = case["exit"].as_i64().unwrap();
    if i64::from(run.exit) != exit {
        found.push(self::found(
            format!("{item}:exit"),
            format!("it exits {}, the contract's {exit}", run.exit),
        ));
    }
    if let Some(code) = case["code"].as_str() {
        error_code(found, &item, code, &run);
    }
}

fn error_code(found: &mut Vec<Found>, item: &str, code: &str, run: &Run) {
    let schema = common::schema();
    let events: Vec<Value> = run
        .out
        .lines()
        .filter(|line| line.starts_with('{'))
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for event in &events {
        let errors: Vec<String> = schema.iter_errors(event).map(|e| e.to_string()).collect();
        if !errors.is_empty() {
            found.push(self::found(
                format!("{item}:schema"),
                format!("an event the events schema doesn't describe: {errors:?}"),
            ));
        }
    }
    let said = events
        .iter()
        .rev()
        .find(|event| event["reason"] == "run-failed")
        .map_or("no run-failed event", |event| text(event, "code"));
    if said != code {
        found.push(self::found(
            format!("{item}:code"),
            format!("{said}, the contract's {code}"),
        ));
    }
    if run.err.lines().any(|line| line.starts_with("usage: ")) {
        found.push(self::found(
            format!("{item}:usage-line"),
            "a `usage:` line follows the error, not the one command to run",
        ));
    }
    if !run.err.lines().any(|line| line.starts_with("mutastic: ")) {
        found.push(self::found(
            format!("{item}:stderr"),
            "no line on stderr starts `mutastic: `",
        ));
    }
}

/// A copy of the fixture with a test that fails with no mutant in: the pricing test expects 26.
fn red_baseline() -> String {
    static RED: OnceLock<PathBuf> = OnceLock::new();
    RED.get_or_init(|| {
        let red = common::own_copy("pricing", "contract-red");
        let test = red.join("tests/pricing.rs");
        let source = std::fs::read_to_string(&test).unwrap();
        std::fs::write(&test, source.replace("), 25);", "), 26);")).unwrap();
        red
    })
    .display()
    .to_string()
}

#[test]
fn the_summary_has_the_contracts_rows() {
    let mut found = Vec::new();
    summary(&mut found, &rows(&full().out));
    assert_kept(&found, &["summary"]);
}

/// The summary's rows: a label right-aligned in the first ten columns, two spaces, its value.
fn rows(output: &str) -> Vec<(String, String)> {
    let label = Regex::new(r"^ *[A-Z][a-z]+(?: [a-z]+)?$").unwrap();
    output
        .lines()
        .filter(|line| line.len() > 12 && line.is_char_boundary(10) && line.is_char_boundary(12))
        .filter(|line| &line[10..12] == "  " && !line[12..].starts_with(' '))
        .filter(|line| label.is_match(&line[..10]))
        .map(|line| (line[..10].trim().to_string(), line[12..].to_string()))
        .collect()
}

fn summary(found: &mut Vec<Found>, rows: &[(String, String)]) {
    let specs = contract()["output"]["summary"]["rows"].as_array().unwrap();
    let labels: Vec<&str> = specs.iter().map(|row| text(row, "label")).collect();
    let fits = |spec: &Value, label: &str| {
        text(spec, "label") == label
            || (spec["pattern"].is_string()
                && !labels.contains(&label)
                && Regex::new(text(spec, "pattern")).unwrap().is_match(label))
    };
    let mut at = 0;
    for (label, value) in rows {
        let Some(matched) = (at..specs.len()).find(|&index| fits(&specs[index], label)) else {
            found.push(if labels.contains(&label.as_str()) {
                self::found(
                    "summary:order",
                    format!("{label} is out of the contract's order"),
                )
            } else {
                self::found(
                    format!("summary:label:{label}"),
                    "a row the contract doesn't have",
                )
            });
            continue;
        };
        required(found, &specs[at..matched]);
        let shape = specs[matched]["shape"].as_str();
        if shape.is_some_and(|shape| !Regex::new(shape).unwrap().is_match(value)) {
            found.push(self::found(
                format!("summary:shape:{label}"),
                format!("`{value}`"),
            ));
        }
        at = matched + 1;
    }
    required(found, &specs[at..]);
}

fn required(found: &mut Vec<Found>, skipped: &[Value]) {
    for row in skipped.iter().filter(|row| row["optional"] != true) {
        found.push(self::found(
            format!("summary:label:{}", text(row, "label")),
            "no such row",
        ));
    }
}

#[test]
fn the_agent_format_has_the_contracts_lines() {
    let run = run(&["run", &fixture(), "--format=agent"]);
    let masked = masked(&run.out);
    let lines: Vec<&str> = masked.lines().collect();
    let agent = &contract()["output"]["agent"];
    let mut found = Vec::new();
    let mut at = 0;
    if sequence(&lines, &mut at, &agent["head"], true, &mut found) {
        while at < lines.len() && sequence(&lines, &mut at, &agent["entry"], false, &mut found) {}
        // The fence's close, after the entries: what quotes the project ends there.
        sequence(&lines, &mut at, &agent["tail"], true, &mut found);
        if at < lines.len() {
            found.push(self::found(
                "agent:extra",
                format!("line {} `{}` fits no shape", at + 1, lines[at]),
            ));
        }
    }
    assert_kept(&found, &["agent"]);
}

/// The ids as `<id>`, and the fixture's directory, as given or relative to here, as `<dir>`.
fn masked(output: &str) -> String {
    let fixture = fixture();
    let relative = place()
        .fixture
        .strip_prefix(&place().here)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| fixture.clone());
    let ids = Regex::new(r"\bm[0-9a-f]{8}\b").unwrap();
    let dir = output.replace(&fixture, "<dir>");
    let text = ids.replace_all(&dir, "<id>");
    text.lines()
        .map(|line| {
            let line = line
                .replace(&format!(" {relative} "), " <dir> ")
                .replace(&format!(" {relative}`"), " <dir>`");
            match line.strip_suffix(&format!(" {relative}")) {
                Some(start) => format!("{start} <dir>"),
                None => line,
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The shapes in order, each as often as its `repeat` says; false, and what failed, when a shape
/// that must come doesn't. Not `required`: a sequence that doesn't start isn't a failure.
fn sequence(
    lines: &[&str],
    at: &mut usize,
    shapes: &Value,
    required: bool,
    found: &mut Vec<Found>,
) -> bool {
    let start = *at;
    for shape in shapes.as_array().unwrap() {
        let repeat = shape["repeat"].as_str().unwrap_or("1");
        let pattern = Regex::new(text(shape, "pattern")).unwrap();
        let mut count = 0;
        while *at < lines.len()
            && pattern.is_match(lines[*at])
            && (count == 0 || "*+".contains(repeat))
        {
            *at += 1;
            count += 1;
        }
        if count == 0 && "1+".contains(repeat) {
            if !required && *at == start {
                return false;
            }
            found.push(self::found(
                format!("agent:{}", text(shape, "name")),
                format!(
                    "line {} `{}`",
                    *at + 1,
                    lines.get(*at).copied().unwrap_or("<end>")
                ),
            ));
            return false;
        }
    }
    true
}

/// Every key path in `value`, `a.b` for an object's, `a[].b` within an array's items, and a file's
/// name as `*` under mutation.json's `files` and `testFiles`.
fn paths(value: &Value, at: &str, into: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            for (key, inner) in object {
                let key = if at == "files" || at == "testFiles" {
                    "*"
                } else {
                    key
                };
                let path = if at.is_empty() {
                    key.to_string()
                } else {
                    format!("{at}.{key}")
                };
                if !into.contains(&path) {
                    into.push(path.clone());
                }
                paths(inner, &path, into);
            }
        }
        Value::Array(array) => {
            for item in array {
                paths(item, &format!("{at}[]"), into);
            }
        }
        _ => {}
    }
}

#[test]
fn the_events_have_the_contracts_key_paths() {
    full();
    let run = run(&["run", &fixture(), "--format=jsonl"]);
    let events: Vec<Value> = run
        .out
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let schema = common::schema();
    for event in &events {
        let errors: Vec<String> = schema.iter_errors(event).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{event} {errors:?}");
    }
    let mut by_type: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for event in &events {
        paths(
            event,
            "",
            by_type
                .entry(text(event, "reason").to_string())
                .or_default(),
        );
    }
    let shared = contract()["events"]["keyPaths"].as_object().unwrap();
    let mut found = Vec::new();
    for (reason, paths) in shared {
        let mine = by_type.get(reason).cloned().unwrap_or_default();
        named(
            &mut found,
            &format!("event:{reason}"),
            &strings(paths),
            &mine,
        );
    }
    for (reason, paths) in by_type.iter().filter(|(r, _)| !shared.contains_key(*r)) {
        for path in paths {
            found.push(self::found(
                format!("event:{reason}:{path}"),
                "a key path the contract doesn't have",
            ));
        }
    }
    order(&mut found, &events);
    assert_kept(&found, &["event", "event-order"]);
}

/// Decision 7: run-started before the first mutant-finished, each mutant-finished before the
/// mutants phase ends, and run-finished or run-failed last.
fn order(found: &mut Vec<Found>, events: &[Value]) {
    let reasons: Vec<String> = events
        .iter()
        .map(|event| match event["phase"].as_str() {
            Some(phase) => format!("{}/{phase}", text(event, "reason")),
            None => text(event, "reason").to_string(),
        })
        .collect();
    let position = |reason: &str| reasons.iter().position(|r| r == reason);
    let first_mutant = position("mutant-finished");
    if first_mutant.is_some_and(|mutant| position("run-started").is_some_and(|s| s > mutant)) {
        found.push(self::found(
            "event-order:run-started",
            "a mutant-finished comes before run-started",
        ));
    }
    let last_mutant = reasons.iter().rposition(|r| r == "mutant-finished");
    let mutants_end = position("phase-finished/mutants");
    if last_mutant.is_some_and(|mutant| mutants_end.is_some_and(|end| mutant > end)) {
        found.push(self::found(
            "event-order:live",
            "a mutant-finished comes after the mutants phase ends",
        ));
    }
    if let Some(last) = reasons.last() {
        if last != "run-finished" && last != "run-failed" {
            found.push(self::found(
                "event-order:last",
                format!("the last event is {last}"),
            ));
        }
    }
}

#[test]
fn the_report_has_the_contracts_key_paths() {
    let reports = Regex::new(r"Reports  (?<json>\S+\.json)").unwrap();
    let json = reports
        .captures(&full().out)
        .map(|found| place().fixture.join(&found["json"]))
        .unwrap();
    let report: Value = serde_json::from_str(&std::fs::read_to_string(json).unwrap()).unwrap();
    let schema: Value = serde_json::from_str(include_str!(
        "schema/report/mutation-testing-report-schema.json"
    ))
    .unwrap();
    let errors: Vec<String> = jsonschema::validator_for(&schema)
        .unwrap()
        .iter_errors(&report)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    let mut mine = Vec::new();
    paths(&report, "", &mut mine);
    let mut found = Vec::new();
    named(
        &mut found,
        "report",
        &strings(&contract()["report"]["keyPaths"]),
        &mine,
    );
    assert_kept(&found, &["report"]);
}
