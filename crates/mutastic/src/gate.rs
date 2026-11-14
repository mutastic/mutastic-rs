//! `mutastic gate`: a coding agent's task, held to what mutastic says of the lines it changed, as
//! the Java edition holds it, so the agents' plugins work with either edition unchanged.
//!
//! `gate open` keeps where the task starts: the commit, the ignores, and how many tests run and how
//! many are ignored (`#[ignore]`). `gate check` asks whether it's done: the mutants on the lines
//! changed since are each caught or proposed for a person to ignore, no ignore was added, no test
//! was taken away or ignored, and no test that failed at an earlier check is gone. If not, it says
//! what's left, as the agent format does, so an agent can carry on; after `--tries` of those, it
//! lets the agent stop and says what's still missed, rather than loop. `gate close` forgets the
//! task.
//!
//! With `--hook=claude`, each reads a Claude Code hook's input on stdin and answers as a hook does:
//! `open` once per session, `check` with a block and its reason, or a message for the person. An
//! ignore the person accepts between turns, after a check let the agent stop and before the next
//! prompt, joins where the task starts; one made while the agent has the turn stays the agent's.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::failure::{Code, Failure};
use crate::since::git;
use crate::testrun::Names;

const FILE: &str = "gate.properties";

/// Before each test that failed at a check, in [`FILE`]: its body's fingerprint is the value.
const RED: &str = "red:";

/// Where a task started, and how often the gate has sent the agent back since it last passed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    /// The agent's session that opened it, when a hook did.
    pub session: String,
    pub base: String,
    pub ignored: BTreeSet<String>,
    pub shape: Shape,
    pub blocks: usize,
    /// The working tree when the gate last sent the agent back: empty when it hasn't since it last
    /// passed.
    pub tree: String,
    /// The tests that failed at a check since the gate opened, each with its body's fingerprint.
    pub red: BTreeMap<String, String>,
    /// The ignores when a hooked check last let the agent stop, until the next prompt: `None` while
    /// the agent has the turn. What's ignored after it is the person's, made between turns.
    pub stopped: Option<BTreeSet<String>>,
}

impl State {
    pub fn read(work: &Path) -> Option<State> {
        let text = std::fs::read_to_string(work.join(FILE)).ok()?;
        let mut state = State::default();
        for line in text.lines().filter(|line| !line.starts_with('#')) {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.to_string();
            match key {
                "session" => state.session = value,
                "base" => state.base = value,
                "ignored" => state.ignored = ids(&value),
                "stopped" => state.stopped = Some(ids(&value)),
                "tests" => state.shape.tests = value.parse().unwrap_or(0),
                "disabled" => state.shape.disabled = value.parse().unwrap_or(0),
                "counted" => state.shape.listed = value == "list",
                "blocks" => state.blocks = value.parse().unwrap_or(0),
                "tree" => state.tree = value,
                _ => {
                    if let Some(test) = key.strip_prefix(RED) {
                        state.red.insert(test.to_string(), value);
                    }
                }
            }
        }
        Some(state)
    }

    pub fn write(&self, work: &Path) -> Result<(), String> {
        let mut lines = vec![
            "# mutastic gate: where this task started".to_string(),
            format!("session={}", self.session),
            format!("base={}", self.base),
            format!(
                "ignored={}",
                self.ignored.iter().cloned().collect::<Vec<_>>().join(",")
            ),
            format!("tests={}", self.shape.tests),
            format!("disabled={}", self.shape.disabled),
            format!(
                "counted={}",
                if self.shape.listed { "list" } else { "source" }
            ),
            format!("blocks={}", self.blocks),
            format!("tree={}", self.tree),
        ];
        if let Some(stopped) = &self.stopped {
            lines.push(format!(
                "stopped={}",
                stopped.iter().cloned().collect::<Vec<_>>().join(",")
            ));
        }
        for (test, body) in &self.red {
            lines.push(format!("{RED}{test}={body}"));
        }
        std::fs::create_dir_all(work).map_err(|error| error.to_string())?;
        std::fs::write(work.join(FILE), lines.join("\n") + "\n").map_err(|error| error.to_string())
    }

    pub fn close(work: &Path) -> bool {
        std::fs::remove_file(work.join(FILE)).is_ok()
    }

    /// A new prompt, with what's ignored `now`: the ignores the person made between turns, after the
    /// agent stopped, are the task's to start from, as those there when it opened are; one the
    /// agent made in its turn stays the agent's, whatever the person says next. With no stop since
    /// the last prompt, the agent still has the turn, and nothing changes.
    pub fn prompted(&self, now: &BTreeSet<String>) -> State {
        let Some(stopped) = &self.stopped else {
            return self.clone();
        };
        State {
            ignored: now
                .iter()
                .filter(|id| self.ignored.contains(*id) || !stopped.contains(*id))
                .cloned()
                .collect(),
            stopped: None,
            ..self.clone()
        }
    }

    /// Whether the agent was sent back, and has changed nothing since.
    pub fn unchanged_since_blocked(&self, now: &str) -> bool {
        self.blocks > 0 && !self.tree.is_empty() && self.tree == now
    }
}

/// Ids as [`FILE`] keeps them: separated by commas.
fn ids(value: &str) -> BTreeSet<String> {
    value
        .split(',')
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect()
}

/// How many tests the package has, and how many of them are ignored.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shape {
    pub tests: usize,
    pub disabled: usize,
    /// Counted from the test binaries' lists, every `#[case]` and matrix cell a test; or, where
    /// the package didn't build when the task started, from the source's test attributes.
    pub listed: bool,
}

impl Shape {
    /// The tests the binaries list.
    pub fn listed(names: &Names) -> Shape {
        Shape {
            tests: names.run.len(),
            disabled: names.ignored.len(),
            listed: true,
        }
    }

    /// The test attributes in the package's sources: for a package that doesn't build.
    pub fn of(package: &Path) -> Shape {
        let mut shape = Shape::default();
        for file in sources(package) {
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            for attribute in attributes(&text) {
                if is_test(&attribute) {
                    shape.tests += 1;
                } else if attribute == "ignore" || attribute.starts_with("ignore=") {
                    shape.disabled += 1;
                }
            }
        }
        shape
    }

    /// Whether this has fewer tests running than `before`: fewer, or more of them ignored.
    pub fn fewer_than(&self, before: &Shape) -> bool {
        self.tests < before.tests || self.disabled > before.disabled
    }
}

/// The Rust files of a package that can hold tests: `src`, `tests`.
fn sources(package: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack: Vec<PathBuf> = ["src", "tests"]
        .iter()
        .map(|dir| package.join(dir))
        .collect();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// Each outer attribute's inside, whitespace left out: `test`, `tokio::test`, `ignore`.
fn attributes(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("#[") {
        let after = &rest[at + 2..];
        let end = after.find(']').unwrap_or(after.len());
        found.push(
            after[..end]
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect(),
        );
        rest = &after[end..];
    }
    found
}

/// `test`, `tokio::test`, `rstest`, `test_case(..)`: what marks a test, as test code is known.
fn is_test(attribute: &str) -> bool {
    let path = attribute.split(['(', '=']).next().unwrap_or(attribute);
    matches!(
        path.rsplit("::").next().unwrap_or(path),
        "test" | "rstest" | "test_case"
    )
}

/// The tests in one source file: each name, to its body's fingerprint.
pub fn tests(source: &str) -> BTreeMap<String, String> {
    let mut tests = BTreeMap::new();
    let mut rest = 0;
    while let Some(at) = source[rest..].find("#[") {
        let start = rest + at + 2;
        let end = source[start..]
            .find(']')
            .map_or(source.len(), |end| start + end);
        let attribute: String = source[start..end]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        rest = end;
        if !is_test(&attribute) {
            continue;
        }
        let Some(fn_at) = source[end..].find("fn ") else {
            break;
        };
        let name_start = end + fn_at + 3;
        let name: String = source[name_start..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            tests
                .entry(name)
                .or_insert_with(|| fingerprint(&body(source, name_start)));
        }
    }
    tests
}

/// The body of a function whose name starts at `from`: its braces and what's between them,
/// strings skipped, so a brace in one doesn't end it.
fn body(source: &str, from: usize) -> String {
    let Some(open) = source[from..].find('{').map(|at| from + at) else {
        return String::new();
    };
    let (mut depth, mut quoted, mut escaped) = (0i32, false, false);
    for (at, ch) in source[open..].char_indices() {
        if quoted {
            quoted = escaped || ch != '"';
            escaped = !escaped && ch == '\\';
        } else if ch == '"' {
            quoted = true;
        } else if ch == '{' || ch == '}' {
            depth += if ch == '{' { 1 } else { -1 };
            if depth == 0 {
                return source[open..=open + at].to_string();
            }
        }
    }
    source[open..].to_string()
}

/// A body, its layout left out, as a short hash: the same test reformatted is the same test.
fn fingerprint(body: &str) -> String {
    let compact: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    Sha256::digest(compact.as_bytes())
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Every test function in the package's sources, by name, to its body's fingerprint.
fn test_functions(package: &Path) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    for file in sources(package) {
        for (name, body) in tests(&std::fs::read_to_string(&file).unwrap_or_default()) {
            found.entry(name).or_insert(body);
        }
    }
    found
}

/// The function a test, as a run names it, is written as: its last segment for a plain `#[test]`,
/// and for a generated one, `rstest_tests::halves::case_1` or `rows::adds::test_case_1_four`, the
/// last segment that names a test function. `None` where no segment does.
fn written_as<'t>(test: &'t str, functions: &BTreeMap<String, String>) -> Option<&'t str> {
    test.rsplit("::")
        .find(|segment| functions.contains_key(*segment))
}

/// The function a test is written as, by name, in the package's sources now: what to look for at
/// another commit. Its last segment where no segment names a test function.
pub fn function_of(test: &str, package: &Path) -> String {
    let functions = test_functions(package);
    written_as(test, &functions)
        .or_else(|| test.rsplit("::").next())
        .unwrap_or(test)
        .to_string()
}

/// A test's fingerprint: its function's body's, and for a generated test, with the segments
/// after the function's name, so each case of one function is told apart. Empty where no segment
/// names a test function.
fn fingerprint_of(test: &str, functions: &BTreeMap<String, String>) -> String {
    let Some(function) = written_as(test, functions) else {
        return String::new();
    };
    let body = &functions[function];
    let segments: Vec<&str> = test.split("::").collect();
    let at = segments
        .iter()
        .rposition(|segment| *segment == function)
        .unwrap_or(0);
    let case = segments[at + 1..].join("::");
    if case.is_empty() {
        body.clone()
    } else {
        fingerprint(&format!("{body}::{case}"))
    }
}

/// The failing tests to remember, each with its fingerprint now.
pub fn remember(failing: &[String], package: &Path) -> BTreeMap<String, String> {
    let functions = test_functions(package);
    failing
        .iter()
        .map(|test| (test.clone(), fingerprint_of(test, &functions)))
        .collect()
}

/// The remembered tests no longer there, by name or by fingerprint: `now` are the tests the
/// binaries list. A move or a rename keeps a test's body, and a rename of its function keeps a
/// case's.
pub fn gone(red: &BTreeMap<String, String>, now: &Names, package: &Path) -> Vec<String> {
    let functions = test_functions(package);
    let bodies: BTreeSet<String> = now
        .run
        .iter()
        .chain(&now.ignored)
        .map(|test| fingerprint_of(test, &functions))
        .filter(|body| !body.is_empty())
        .collect();
    red.iter()
        .filter(|(test, body)| !now.has(test) && (body.is_empty() || !bodies.contains(*body)))
        .map(|(test, _)| test.clone())
        .collect()
}

/// Every test the package has now, from its binaries, built in mutastic's copy with no mutant in:
/// what the gate counts and looks tests up by, as a run names them.
pub fn listed(project: &crate::workspace::Project) -> Result<Names, Failure> {
    let found = crate::engine::find_all(project);
    let none = BTreeSet::new();
    let built = crate::compile::build(project, &found, &none, &none, &|_| {})?;
    Names::of(
        &built.binaries,
        &project.copy_dir().join(&project.package_dir),
    )
    .map_err(|listing| Code::Other.with(listing))
}

/// The source files, there when the task started, that it took an assertion out of: by name. Every
/// mutant is caught by a test made to fit a wrong change, so the score can't show it.
pub fn changed_expectations(package: &Path, base: &str) -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    let diff = git(
        package,
        &[
            "diff",
            "--unified=0",
            "--no-color",
            "--no-ext-diff",
            base,
            "--",
            ".",
        ],
    )
    .unwrap_or_default();
    let mut file = String::new();
    for line in diff.lines() {
        if let Some(name) = line.strip_prefix("--- ") {
            file = match name.strip_prefix("a/") {
                Some(name) => Path::new(name)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                None => String::new(),
            };
        } else if file.ends_with(".rs") && took_out_a_check(line) {
            files.insert(file.clone());
        }
    }
    files
}

/// A line taken out that said what to expect: an assertion, not a comment, and not a
/// `debug_assert!`, which is the code's own check.
fn took_out_a_check(line: &str) -> bool {
    let Some(removed) = line.strip_prefix('-') else {
        return false;
    };
    let removed = removed.trim_start();
    if removed.starts_with("--") || removed.starts_with("//") || removed.starts_with('*') {
        return false;
    }
    removed.contains("assert") && !removed.contains("debug_assert")
}

/// The session a Claude Code hook's input names, or empty.
pub fn session(hook_input: &str) -> String {
    serde_json::from_str::<serde_json::Value>(hook_input)
        .ok()
        .and_then(|input| input["session_id"].as_str().map(str::to_string))
        .unwrap_or_default()
}

/// The working tree as git would commit it, untracked files included and ignored ones not: the
/// same hash for the same files. A scratch index takes the files, so the real one is untouched.
pub fn tree(dir: &Path) -> String {
    let Some(real) = git(dir, &["rev-parse", "--git-path", "index"]) else {
        return String::new();
    };
    let index = std::env::temp_dir().join(format!("mutastic-gate-{}.index", std::process::id()));
    let real = dir.join(real.trim());
    if std::fs::copy(&real, &index).is_err() {
        let _ = std::fs::remove_file(&index);
    }
    // The copy keeps the index's time: git trusts a file whose size and time it has, unless the
    // file is as new as the index. A copy made now would hide a change made in the same second
    // that kept the size, where git compares times to the second.
    if let Ok(modified) = std::fs::metadata(&real).and_then(|meta| meta.modified()) {
        let _ = std::fs::File::options()
            .write(true)
            .open(&index)
            .and_then(|file| file.set_modified(modified));
    }
    let scratch = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_INDEX_FILE", &index)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    let tree = scratch(&["add", "--all"])
        .and_then(|_| scratch(&["write-tree"]))
        .unwrap_or_default();
    let _ = std::fs::remove_file(&index);
    tree
}

/// Whether the working tree is the commit `base`'s, file for file: a task that changed nothing
/// can't have broken a test.
pub fn untouched(dir: &Path, base: &str) -> bool {
    let at_base = git(dir, &["rev-parse", &format!("{base}^{{tree}}")]);
    at_base.is_some_and(|at_base| at_base.trim() == tree(dir))
}

/// Whether the task is done, what to say, and whether that's to the agent or past it, to the
/// person.
#[derive(Clone, Debug)]
pub struct Verdict {
    pub passes: bool,
    pub text: String,
    /// The tests that failed at this check, to remember.
    pub red: BTreeMap<String, String>,
}

impl Verdict {
    pub fn pass(text: String) -> Verdict {
        Verdict {
            passes: true,
            text,
            red: BTreeMap::new(),
        }
    }

    pub fn block(text: String) -> Verdict {
        Verdict {
            passes: false,
            text,
            red: BTreeMap::new(),
        }
    }
}

/// What a Claude Code hook answers: a block, whose reason is the agent's next instruction, or a
/// message for the person.
pub fn claude_hook(verdict: &Verdict) -> String {
    let text = serde_json::Value::String(verdict.text.clone());
    if verdict.passes {
        format!("{{\"systemMessage\": {text}}}")
    } else {
        format!("{{\"decision\": \"block\", \"reason\": {text}}}")
    }
}

/// Tests that caught mutants and then failed on their own with no mutant in, as the Java edition
/// says it: their catches don't count, and the task isn't done while they do.
pub fn false_caught(report: &crate::engine::Report) -> String {
    format!(
        "mutastic gate: a test that caught mutants on the lines the task changed fails on its own, with no mutant in, so its catches don't count and the task isn't done.\n\n{}",
        crate::report::failing_alone(report).trim_end()
    )
}

/// What holds the gate, every mutant by id, as the Java edition says it: those missed, those no
/// test reaches, and apart from them those proposed, which wait on a person, not the agent.
pub fn held_by(missed: &[String], uncovered: &[String], waiting: &[String]) -> String {
    let count = missed.len() + uncovered.len();
    let mut parts = Vec::new();
    if !missed.is_empty() {
        parts.push(format!("missed: {}", missed.join(", ")));
    }
    if !uncovered.is_empty() {
        parts.push(format!("no test reaches: {}", uncovered.join(", ")));
    }
    let proposed = if waiting.is_empty() {
        String::new()
    } else {
        format!(
            " Proposed, waiting on a person, and not holding it: {}.",
            waiting.join(", ")
        )
    };
    format!(
        "mutastic gate: {count} mutant{} on the lines the task changed {} it, so it isn't done ({}).{proposed} For each card: write the test, and run its `done when` before you move on; for a line no test reaches, write a test that reaches it; if a case can't happen, propose ignoring it, for a person to accept. {}",
        if count == 1 { "" } else { "s" },
        if count == 1 { "holds" } else { "hold" },
        parts.join("; "),
        crate::report::NO_REWRITE
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A change made in the second the task started, that keeps the file's size, is a change:
    /// where git compares times to the second (Linux's git, `core.checkStat=minimal`) a copy of
    /// the index made later took the file for the one it had. (macOS's copy keeps the time
    /// anyway, so it's on Linux this fails without the fix.)
    #[test]
    fn a_change_in_the_starting_second_is_seen() {
        let dir = std::env::temp_dir().join(format!("mutastic-racy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .current_dir(&dir)
                .output()
                .unwrap();
            assert!(output.status.success(), "git {args:?}");
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        };
        run(&["init", "-q"]);
        run(&["config", "core.checkStat", "minimal"]);
        std::fs::write(dir.join("threshold"), "50\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "start"]);
        let base = run(&["rev-parse", "HEAD"]);
        std::fs::write(dir.join("threshold"), "40\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        assert!(!untouched(&dir, &base));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The shape an eval met: both missed mutants proposed, and two no test reaches holding the
    /// gate. Every one that holds it is named, and the proposed ones aren't called missed.
    #[test]
    fn every_mutant_that_holds_the_gate_is_named() {
        let ids = |list: &[&str]| list.iter().map(|id| id.to_string()).collect::<Vec<_>>();
        let said = held_by(
            &[],
            &ids(&["m04b90863", "m304d2733"]),
            &ids(&["m1a2b3c4d", "m5e6f7a8b"]),
        );
        assert!(
            said.starts_with(
                "mutastic gate: 2 mutants on the lines the task changed hold it, so it isn't done \
                 (no test reaches: m04b90863, m304d2733). Proposed, waiting on a person, and not \
                 holding it: m1a2b3c4d, m5e6f7a8b."
            ),
            "{said}"
        );
        assert!(!said.contains("missed:"), "{said}");
        let one = held_by(&ids(&["m1a2b3c4d"]), &[], &[]);
        assert!(
            one.starts_with("mutastic gate: 1 mutant on the lines the task changed holds it, so it isn't done (missed: m1a2b3c4d). For each card"),
            "{one}"
        );
    }

    #[test]
    fn tests_are_known_by_name_and_body_however_laid_out() {
        let source = "#[test]\nfn adds() {\n    assert_eq!(add(1, 1), 2); // \"}\"\n}\n\
                      #[tokio::test]\nasync fn waits() { let s = \"{\"; }\n\
                      #[test]\n#[ignore]\nfn later() {}\nfn helper() {}\n";
        let found = tests(source);
        assert_eq!(found.keys().collect::<Vec<_>>(), ["adds", "later", "waits"]);
        let reformatted = tests("#[test] fn adds() { assert_eq!(add(1,1),2); // \"}\"\n }");
        assert_eq!(found["adds"], reformatted["adds"]);
        assert_eq!(attributes(source).iter().filter(|a| is_test(a)).count(), 3);
    }

    /// A generated case is looked up by its whole name, as libtest lists it: no function is
    /// called `case_1`. One that failed is there while it's listed, a rename of its function keeps
    /// it, and taking its row away loses it, though its function is still there.
    #[test]
    fn a_case_is_known_by_its_listed_name() {
        let package =
            std::env::temp_dir().join(format!("mutastic-gate-cases-{}", std::process::id()));
        std::fs::create_dir_all(package.join("src")).unwrap();
        let write = |name: &str| {
            let source = format!(
                "#[rstest]\n#[case(4, 2)]\n#[case(0, 0)]\nfn {name}(#[case] x: i64, #[case] half: i64) {{\n    assert_eq!(x / 2, half);\n}}\n"
            );
            std::fs::write(package.join("src/lib.rs"), source).unwrap();
        };
        let names = |run: &[&str]| Names {
            run: run.iter().map(|name| name.to_string()).collect(),
            ignored: BTreeSet::new(),
        };
        write("halves");
        let red = remember(&["tests::halves::case_1".to_string()], &package);
        assert_ne!(
            red["tests::halves::case_1"], "",
            "fingerprinted by its function"
        );
        let both = names(&["tests::halves::case_1", "tests::halves::case_2"]);
        assert!(gone(&red, &both, &package).is_empty(), "still listed");
        assert_eq!(function_of("tests::halves::case_1", &package), "halves");

        write("halving");
        let renamed = names(&["tests::halving::case_1", "tests::halving::case_2"]);
        assert!(
            gone(&red, &renamed, &package).is_empty(),
            "renamed, same case"
        );

        let red = remember(&["tests::halving::case_2".to_string()], &package);
        let one_row = names(&["tests::halving::case_1"]);
        assert_eq!(gone(&red, &one_row, &package), ["tests::halving::case_2"]);
        std::fs::remove_dir_all(&package).ok();
    }

    #[test]
    fn a_state_is_written_and_read_back() {
        let work = std::env::temp_dir().join(format!("mutastic-gate-{}", std::process::id()));
        let state = State {
            session: "abc".into(),
            base: "0123abcd".into(),
            ignored: ["m1".to_string(), "m2".to_string()].into(),
            shape: Shape {
                tests: 5,
                disabled: 1,
                listed: true,
            },
            blocks: 2,
            tree: "fedc".into(),
            red: [("tests::adds".to_string(), "00ff".to_string())].into(),
            stopped: Some(["m1".to_string()].into()),
        };
        state.write(&work).unwrap();
        assert_eq!(State::read(&work), Some(state));
        assert!(State::close(&work));
        assert_eq!(State::read(&work), None);
        std::fs::remove_dir_all(&work).ok();
    }

    /// Only what's ignored between turns, after the agent stopped, joins where the task starts: an
    /// ignore the agent made in its turn stays its own after the next prompt, and with no stop since
    /// the last prompt, the agent's turn goes on.
    #[test]
    fn only_an_ignore_made_between_turns_joins_the_start() {
        let set = |ids: &[&str]| ids.iter().map(|id| id.to_string()).collect::<BTreeSet<_>>();
        let state = State {
            ignored: set(&["m0", "gone"]),
            stopped: Some(set(&["m0", "gone", "agents"])),
            ..State::default()
        };
        let now = set(&["m0", "agents", "persons"]);
        let prompted = state.prompted(&now);
        assert_eq!(prompted.ignored, set(&["m0", "persons"]));
        assert_eq!(prompted.stopped, None);
        assert_eq!(
            prompted.prompted(&now),
            prompted,
            "no stop since: still the agent's turn"
        );
    }

    #[test]
    fn a_check_taken_out_of_a_test_is_seen() {
        assert!(took_out_a_check("-        assert_eq!(total(2), 4);"));
        assert!(!took_out_a_check("-        debug_assert!(x > 0);"));
        assert!(!took_out_a_check("-        // assert_eq!(total(2), 4);"));
        assert!(!took_out_a_check("+        assert_eq!(total(2), 4);"));
        assert!(!took_out_a_check("--- a/src/lib.rs"));
    }

    #[test]
    fn a_hook_answers_as_claude_code_reads_it() {
        assert_eq!(session(r#"{"session_id": "s-1", "cwd": "/x"}"#), "s-1");
        assert_eq!(
            claude_hook(&Verdict::block("left: \"a\"\nb".into())),
            r#"{"decision": "block", "reason": "left: \"a\"\nb"}"#
        );
        assert_eq!(
            claude_hook(&Verdict::pass("done".into())),
            r#"{"systemMessage": "done"}"#
        );
    }
}
