//! Property tests: the frameworks a package's tests use, what mutastic can set for each, and the
//! input a failing property names.
//!
//! A property draws its inputs at random. Drawn afresh in each process, a mutant only some inputs
//! catch is caught in one run and missed in the next, and the flaky check reads a property that
//! fails for a rare input as a flaky test. So where a framework takes its seed from the
//! environment, every test process of a run, the coverage pass's and each mutant's, gets the same
//! one, and every run the same one unless the person sets it: caught then means caught, every
//! time. Where a framework can't be seeded from outside, the run says so rather than guess.
//!
//! On a mutant's run, a property's first counterexample is enough to call the mutant caught, so
//! shrinking it, often the costly part, is turned off where the framework lets it be. The input
//! that failed goes on the card, as the evidence a property gives that no other test does.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::workspace::Project;

/// A property-testing framework a package's tests can use, in the order they're named.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Framework {
    Proptest,
    Quickcheck,
    Arbtest,
    Bolero,
}

impl Framework {
    const ALL: [Framework; 4] = [
        Framework::Proptest,
        Framework::Quickcheck,
        Framework::Arbtest,
        Framework::Bolero,
    ];

    /// Its crate's name, as a package depends on it.
    pub fn name(self) -> &'static str {
        match self {
            Framework::Proptest => "proptest",
            Framework::Quickcheck => "quickcheck",
            Framework::Arbtest => "arbtest",
            Framework::Bolero => "bolero",
        }
    }
}

/// The frameworks a package's manifest depends on, in any section: dev-dependencies are where
/// they usually are. `dependencies` is the package's list as `cargo metadata` gives it.
pub fn used(dependencies: &Value) -> BTreeSet<Framework> {
    let names: BTreeSet<&str> = dependencies
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|dependency| dependency["name"].as_str())
        .collect();
    Framework::ALL
        .into_iter()
        .filter(|framework| names.contains(framework.name()))
        .collect()
}

/// The seed proptest draws its cases from in mutastic's runs, where the person hasn't set
/// `PROPTEST_RNG_SEED`: any fixed value, kept so a project's results stay as they were.
pub const PROPTEST_SEED: &str = "0";

/// The seed proptest draws from in this run: the person's `PROPTEST_RNG_SEED`, else
/// [`PROPTEST_SEED`]. `None` when theirs isn't a whole number proptest reads, which it warns about
/// and then draws at random.
pub fn proptest_seed() -> Option<String> {
    match std::env::var("PROPTEST_RNG_SEED") {
        Ok(set) => set.parse::<u64>().is_ok().then_some(set),
        Err(std::env::VarError::NotPresent) => Some(PROPTEST_SEED.to_string()),
        Err(std::env::VarError::NotUnicode(_)) => None,
    }
}

/// The first proptest that reads `PROPTEST_RNG_SEED`: before it, every runner seeds itself from
/// the system, and the variable is only warned about.
const SEEDED_FROM: (u64, u64) = (1, 7);

/// What every test process of a run is given: proptest's seed, unless the person set one, and
/// its saved failures off. A failing proptest saves its case, and every later run replays it
/// first: a run's own failures would decide its next runs, the flaky check's and the guard's
/// among them.
pub const EVERY_RUN: [(&str, &str); 1] = [("PROPTEST_DISABLE_FAILURE_PERSISTENCE", "1")];

/// What a mutant's test process is given besides: no shrinking, as the first counterexample
/// already catches the mutant.
pub const MUTANT_RUN: [(&str, &str); 1] = [("PROPTEST_MAX_SHRINK_ITERS", "0")];

/// One framework the package uses that mutastic can't seed, and the version that's why, when it's
/// a proptest from before [`SEEDED_FROM`].
struct Unseeded {
    framework: Framework,
    version: Option<String>,
}

impl Unseeded {
    /// `quickcheck`, or `proptest 1.6.0`.
    fn said(&self) -> String {
        match &self.version {
            Some(version) => format!("{} {version}", self.framework.name()),
            None => self.framework.name().to_string(),
        }
    }
}

/// The frameworks the package uses whose seeds mutastic can't set. Only proptest takes one from
/// the environment for its whole search, and only from 1.7.0: quickcheck seeds each run from the
/// system, and arbtest's and bolero's variables replay one case rather than seed the search.
fn unseeded(project: &Project) -> Vec<Unseeded> {
    let old_proptest = old_proptest(&lock(project));
    project
        .properties
        .iter()
        .filter_map(|&framework| match framework {
            Framework::Proptest if proptest_seed().is_none() => Some(Unseeded {
                framework,
                version: None,
            }),
            Framework::Proptest => old_proptest.clone().map(|version| Unseeded {
                framework,
                version: Some(version),
            }),
            _ => Some(Unseeded {
                framework,
                version: None,
            }),
        })
        .collect()
}

/// Whether proptest draws from the seed mutastic sets in this package: none of its proptests is
/// from before [`SEEDED_FROM`].
pub fn seeds_proptest(project: &Project) -> bool {
    old_proptest(&lock(project)).is_none()
}

/// The package's `Cargo.lock`, as mutastic's copy of it builds with: Cargo writes one there when
/// the person has none. Empty when neither has been written yet.
fn lock(project: &Project) -> String {
    [project.copy_dir(), project.workspace.clone()]
        .iter()
        .find_map(|dir| std::fs::read_to_string(dir.join("Cargo.lock")).ok())
        .unwrap_or_default()
}

/// The oldest proptest `lock` resolves to, when it's one from before [`SEEDED_FROM`].
fn old_proptest(lock: &str) -> Option<String> {
    let lines: Vec<&str> = lock.lines().map(str::trim).collect();
    lines
        .windows(2)
        .filter(|pair| pair[0] == "name = \"proptest\"")
        .filter_map(|pair| pair[1].strip_prefix("version = \"")?.strip_suffix('"'))
        .filter(|version| {
            let mut parts = version.split('.').map(|part| part.parse::<u64>().ok());
            let (Some(Some(major)), Some(Some(minor))) = (parts.next(), parts.next()) else {
                return false;
            };
            (major, minor) < SEEDED_FROM
        })
        .min()
        .map(str::to_string)
}

/// `run-started`'s `propertyTests`, for a package that uses a framework: which, the seed of those
/// mutastic seeds, and those it can't.
pub fn started(project: &Project) -> Option<Value> {
    if project.properties.is_empty() {
        return None;
    }
    let unseeded = unseeded(project);
    let unseeded: Vec<&str> = unseeded.iter().map(|f| f.framework.name()).collect();
    let mut started = json!({
        "frameworks": project.properties.iter().map(|f| f.name()).collect::<Vec<_>>(),
    });
    let seeded: Vec<&str> = project
        .properties
        .iter()
        .map(|f| f.name())
        .filter(|name| !unseeded.contains(name))
        .collect();
    if let (false, Some(seed)) = (seeded.is_empty(), proptest_seed()) {
        started["seed"] = json!(seed);
        started["seeded"] = json!(seeded);
    }
    if !unseeded.is_empty() {
        started["unseeded"] = json!(unseeded);
    }
    Some(started)
}

/// The note a run ends with when the package's property tests include some mutastic can't seed:
/// their results can change from run to run, and the run says so rather than hide it.
pub fn note(project: &Project) -> Option<String> {
    let unseeded: Vec<String> = unseeded(project).iter().map(Unseeded::said).collect();
    naming(&unseeded)
}

/// The note, naming the frameworks it's about: `quickcheck`, `proptest 1.6.0 and quickcheck`.
fn naming(unseeded: &[String]) -> Option<String> {
    let (last, rest) = unseeded.split_last()?;
    let named = if rest.is_empty() {
        last.clone()
    } else {
        format!("{} and {last}", rest.join(", "))
    };
    Some(format!(
        "note: property tests in {named} run with their own seeds, which mutastic can't set: a \
         mutant only some of their inputs catch can be caught in one run and missed in the next"
    ))
}

/// `` with `xs = [0, 0]` ``, as a line says the input after the test that caught a mutant, its
/// text shown as text; nothing when no input was named.
pub fn said(input: Option<&str>) -> String {
    input.map_or_else(String::new, |input| {
        format!(" with `{}`", crate::untrusted::value(input))
    })
}

/// Whether a test's output is a failure of a framework whose shrinking a mutant's run turns off,
/// proptest's: its input is the first that failed, which a run with shrinking on makes smaller.
pub fn unshrunk(output: &str) -> bool {
    output.contains(PROPTEST_FAILURE)
}

/// What proptest's failure says before its input.
const PROPTEST_FAILURE: &str = "minimal failing input: ";

/// The most of an input a run keeps: a property can draw a long vector.
const LONGEST: usize = 300;

/// The input the failing property `test` (libtest's name) says it failed on, from a test
/// process's output, on one line: `xs = [0, 0]`. Only from the frameworks that print one:
/// proptest's `minimal failing input:`, quickcheck's `Arguments: (…)` and bolero's `Input:`.
pub fn failing_input(output: &str, test: &str) -> Option<String> {
    let section = section(output, test);
    let lines: Vec<&str> = section.lines().collect();
    let input = lines.iter().enumerate().find_map(|(at, line)| {
        if let Some(first) = line.strip_prefix(PROPTEST_FAILURE) {
            // `{:#?}`, a line to each field, then the runner's counts, each after a tab.
            let rest = lines[at + 1..]
                .iter()
                .take_while(|line| !line.starts_with('\t') && !line.is_empty());
            return Some(one_line(std::iter::once(first).chain(rest.copied())));
        }
        if let Some((_, arguments)) = line
            .strip_prefix("[quickcheck] TEST FAILED")
            .and_then(|rest| rest.split_once(". Arguments: ("))
        {
            return arguments.strip_suffix(')').map(str::to_string);
        }
        if line.trim_end() == "Input:" && section.contains(BOLERO_FAILURE) {
            let block = lines[at + 1..].iter().take_while(|line| !line.is_empty());
            return Some(one_line(block.copied()));
        }
        None
    })?;
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    Some(match input.char_indices().nth(LONGEST) {
        Some((end, _)) => format!("{}…", &input[..end]),
        None => input.to_string(),
    })
}

/// The banner bolero's failure starts with, before its `Input:`: a test's own output can say
/// `Input:` too.
const BOLERO_FAILURE: &str = "======================== Test Failure ========================";

/// What libtest printed of one test's run: its `---- name stdout ----` section, up to the next
/// test's or the list of failures; the whole output when it has no such section.
fn section<'a>(output: &'a str, test: &str) -> &'a str {
    let header = format!("---- {test} stdout ----");
    let Some(start) = output.find(&header) else {
        return output;
    };
    let after = &output[start + header.len()..];
    let end = after
        .match_indices('\n')
        .map(|(at, _)| at + 1)
        .find(|&at| after[at..].starts_with("---- ") || after[at..].starts_with("failures:"))
        .unwrap_or(after.len());
    &after[..end]
}

/// `{:#?}`'s lines as `{:?}` writes them: `[\n    0,\n    0,\n]` is `[0, 0]`, and `Point {\n    x:
/// 1,\n}` is `Point { x: 1 }`. Pretty Debug opens a bracket at a line's end and closes it at the
/// next line's start, and a string never spans lines, so only layout is taken out.
fn one_line<'a>(lines: impl Iterator<Item = &'a str>) -> String {
    let lines: Vec<&str> = lines
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let mut joined = String::new();
    for (at, line) in lines.iter().enumerate() {
        let next_closes = lines
            .get(at + 1)
            .is_some_and(|next| next.starts_with([']', ')', '}']));
        let line = match line.strip_suffix(',') {
            Some(kept) if next_closes => kept,
            _ => line,
        };
        if let Some(previous) = joined.chars().last() {
            let tight = matches!(previous, '[' | '(') || line.starts_with([']', ')']);
            if !tight {
                joined.push(' ');
            }
        }
        joined.push_str(line);
    }
    joined
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manifest_names_the_frameworks_it_depends_on() {
        let dependencies = json!([
            {"name": "serde", "kind": null},
            {"name": "quickcheck", "kind": "dev"},
            {"name": "proptest", "kind": "dev"},
            {"name": "quickcheck_macros", "kind": "dev"},
        ]);
        assert_eq!(
            used(&dependencies),
            BTreeSet::from([Framework::Proptest, Framework::Quickcheck])
        );
        assert!(used(&json!([{"name": "proptest-derive"}])).is_empty());
    }

    #[test]
    fn the_note_names_the_frameworks_mutastic_cant_seed() {
        assert_eq!(naming(&[]), None);
        assert_eq!(
            naming(&["quickcheck".to_string()]).unwrap(),
            "note: property tests in quickcheck run with their own seeds, which mutastic can't \
             set: a mutant only some of their inputs catch can be caught in one run and missed in \
             the next"
        );
        let three = ["proptest 1.6.0", "quickcheck", "bolero"].map(str::to_string);
        assert!(naming(&three)
            .unwrap()
            .starts_with("note: property tests in proptest 1.6.0, quickcheck and bolero run"));
    }

    #[test]
    fn a_proptest_before_1_7_takes_no_seed() {
        let lock = |versions: &[&str]| {
            versions
                .iter()
                .map(|v| format!("[[package]]\nname = \"proptest\"\nversion = \"{v}\"\n"))
                .collect::<String>()
                + "\n[[package]]\nname = \"rand\"\nversion = \"0.8.5\"\n"
        };
        assert_eq!(old_proptest(&lock(&["1.6.0"])).as_deref(), Some("1.6.0"));
        assert_eq!(old_proptest(&lock(&["0.10.1"])).as_deref(), Some("0.10.1"));
        assert_eq!(old_proptest(&lock(&["1.7.0"])), None);
        assert_eq!(
            old_proptest(&lock(&["1.12.0", "1.5.0"])).as_deref(),
            Some("1.5.0")
        );
        assert_eq!(old_proptest(&lock(&[])), None);
    }

    #[test]
    fn proptests_input_is_put_on_one_line() {
        let output = "running 2 tests\ntest props::other ... FAILED\ntest props::reverses_twice ... FAILED\n\n\
            failures:\n\n---- props::other stdout ----\n\nthread 'props::other' panicked at src/lib.rs:9:5:\n\
            Test failed: no.\nminimal failing input: n = 7\n\tsuccesses: 0\n\tlocal rejects: 0\n\tglobal rejects: 0\n\n\
            ---- props::reverses_twice stdout ----\n\nthread 'props::reverses_twice' panicked at src/lib.rs:20:5:\n\
            Test failed: assertion failed: `(left == right)`\n  left: `[0]`,\n right: `[0, 0]` at src/lib.rs:22.\n\
            minimal failing input: xs = [\n    0,\n    0,\n], label = \"a,\\n]\"\n\tsuccesses: 3\n\tlocal rejects: 0\n\tglobal rejects: 0\n\n\n\
            failures:\n    props::other\n    props::reverses_twice\n";
        assert_eq!(
            failing_input(output, "props::reverses_twice").unwrap(),
            "xs = [0, 0], label = \"a,\\n]\""
        );
        assert_eq!(failing_input(output, "props::other").unwrap(), "n = 7");
    }

    #[test]
    fn a_struct_and_nested_vectors_read_as_debug_writes_them() {
        let output = "minimal failing input: p = Point {\n    x: 1,\n    tags: [\n        \"a\",\n    ],\n}, \
            ys = [\n    [\n        1,\n        2,\n    ],\n    [],\n]\n\tsuccesses: 0\n";
        assert_eq!(
            failing_input(output, "any").unwrap(),
            "p = Point { x: 1, tags: [\"a\"] }, ys = [[1, 2], []]"
        );
    }

    #[test]
    fn quickchecks_arguments_and_bolero_input_are_read() {
        let quickcheck =
            "---- t stdout ----\nthread 't' panicked at /quickcheck-1.1.0/src/tester.rs:181:28:\n\
            [quickcheck] TEST FAILED. Arguments: ([1, 0], 3)\nnote: run with `RUST_BACKTRACE=1`\n";
        assert_eq!(failing_input(quickcheck, "t").unwrap(), "[1, 0], 3");
        let panicked = "[quickcheck] TEST FAILED (runtime error). Arguments: (-4)\nError: \"attempt to negate\"\n";
        assert_eq!(failing_input(panicked, "t").unwrap(), "-4");
        let bolero = "\n======================== Test Failure ========================\n\n\
            BOLERO_RANDOM_SEED=1234\n\nInput: \n(\n    3,\n    [\n        1,\n    ],\n)\n\nError: \npanicked\n";
        assert_eq!(failing_input(bolero, "t").unwrap(), "(3, [1])");
    }

    #[test]
    fn a_test_that_names_no_input_has_none() {
        assert_eq!(failing_input("assertion failed: x > 1\n", "t"), None);
        assert_eq!(failing_input("Input:\n7\n\n", "t"), None, "not bolero's");
        assert_eq!(
            failing_input("arbtest failed!\n    Seed: 0xa88e234400000020\n", "t"),
            None
        );
    }

    #[test]
    fn a_long_input_is_cut() {
        let output = format!(
            "minimal failing input: xs = {:?}\n\tsuccesses: 0\n",
            vec![7; 400]
        );
        let input = failing_input(&output, "t").unwrap();
        assert_eq!(input.chars().count(), LONGEST + 1);
        assert!(input.ends_with('…'));
    }
}
