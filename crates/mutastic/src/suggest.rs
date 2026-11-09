//! Suggested tests, found as the Java edition finds them: by calling the function with candidate
//! inputs, first as it is and then with the mutant in, and keeping the first input where the two
//! answer differently. The calls run in the function's probe, a test mutastic builds beside it in
//! its copy, so every value a suggested test writes is one the code really gave. A function that
//! answers one call two ways (a clock, a random number, shared state) gets no suggestion.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use crate::compile::Binary;
use crate::sites::callable::Callable;
use crate::testrun;

/// How many calls a suggestion may try, as the Java edition caps them.
const CANDIDATES: usize = 400;

/// How long one probe's run may take: its calls together, as it is or with the mutant.
const PROBE_TIME: Duration = Duration::from_secs(3);

/// What the probe found: the numbers (the receiver's first), what the call gives today, as a
/// literal, and what it gives with the mutant in: a literal, `panics` or `doesn't return`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub values: Vec<i128>,
    pub today: String,
    pub mutated: String,
}

/// A suggested test as code to paste: where it goes, its name and lines, and what it calls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuggestedTest {
    /// `mod rates::tests`, or where a new test module goes when there's none.
    pub place: String,
    /// The module's path, `rates::tests`.
    pub module: String,
    pub name: String,
    /// The test, one line each, unindented.
    pub lines: Vec<String>,
    pub call: String,
    pub today: String,
    pub mutated: String,
}

/// The inputs to try, each a value for every one of `types`: the constants near the change and
/// either side of each, the values the tests passed, then a few small numbers, as the Java edition
/// tries them, the earlier first, leaving out a value its type can't hold.
pub fn candidates(
    types: &[&str],
    constants: &[i128],
    inputs: &BTreeMap<String, Vec<i128>>,
) -> Vec<Vec<i128>> {
    let mut values: Vec<i128> = Vec::new();
    let near = constants
        .iter()
        .flat_map(|&constant| [constant, constant - 1, constant + 1]);
    let seen = inputs.values().flatten().copied();
    for value in near.chain(seen).chain([0, 1, 2, 3, -1, 10]) {
        if !values.contains(&value) {
            values.push(value);
        }
    }
    let each: Vec<Vec<i128>> = types
        .iter()
        .map(|ty| {
            values
                .iter()
                .copied()
                .filter(|&value| crate::sites::fits(value, ty))
                .collect()
        })
        .collect();
    let mut out = Vec::new();
    combine(&each, &mut Vec::new(), &mut out);
    out
}

/// Every way to pick one value per position, the earlier values first, up to `CANDIDATES`.
fn combine(each: &[Vec<i128>], prefix: &mut Vec<i128>, out: &mut Vec<Vec<i128>>) {
    if out.len() >= CANDIDATES {
        return;
    }
    let Some(values) = each.get(prefix.len()) else {
        out.push(prefix.clone());
        return;
    };
    for &value in values {
        prefix.push(value);
        combine(each, prefix, out);
        prefix.pop();
        if out.len() >= CANDIDATES {
            return;
        }
    }
}

/// The integer literals on `line`, as written: the constants near a change.
pub fn constants(line: &str) -> Vec<i128> {
    line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|word| word.starts_with(|c: char| c.is_ascii_digit()))
        .filter_map(crate::sites::integer_literal)
        .map(|(value, _)| value)
        .collect()
}

/// Where a probe runs: its binary, the directory, and its name.
pub struct Probe<'a> {
    pub binary: &'a Binary,
    pub cwd: &'a Path,
    pub name: &'a str,
}

/// Runs the probe on `candidates`, as the code is and with mutant `ordinal` in, and gives the first
/// input where they differ. Its files go in `dir`, named for the mutant.
pub fn find(
    probe: &Probe,
    dir: &Path,
    ordinal: u32,
    candidates: &[Vec<i128>],
) -> Option<Suggestion> {
    let input = dir.join(format!("{ordinal}.in"));
    let lines: Vec<String> = candidates
        .iter()
        .map(|values| {
            values
                .iter()
                .map(i128::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    std::fs::write(&input, lines.join("\n") + "\n").ok()?;
    let (today, _) = run(probe, &input, &dir.join(format!("{ordinal}.today")), None);
    // A call that answered twice differently: the code can't be pinned by an assertion, and any
    // difference the mutant seemed to make could be chance.
    if today.iter().any(|line| line == "x") {
        return None;
    }
    let (mutated, timed_out) = run(
        probe,
        &input,
        &dir.join(format!("{ordinal}.mutant")),
        Some(ordinal),
    );
    first_difference(candidates, &today, &mutated, timed_out)
}

/// The probe's lines, one per candidate it reached, and whether it ran out of time.
fn run(probe: &Probe, input: &Path, output: &Path, active: Option<u32>) -> (Vec<String>, bool) {
    let _ = std::fs::remove_file(output);
    let mut env = vec![
        ("MUTASTIC_PROBE", input.display().to_string()),
        ("MUTASTIC_PROBE_OUT", output.display().to_string()),
    ];
    match active {
        Some(ordinal) => env.push(("MUTASTIC_ACTIVE", ordinal.to_string())),
        None => env.push(("MUTASTIC_PROBE_TWICE", "1".into())),
    }
    let outcome = testrun::run(probe.binary, probe.cwd, &[probe.name], &env, PROBE_TIME);
    let text = std::fs::read_to_string(output).unwrap_or_default();
    // A line cut off by the time limit has no end yet: it isn't counted.
    let complete = text.rfind('\n').map_or("", |end| &text[..end]);
    let lines = complete.lines().map(str::to_string).collect();
    (lines, outcome.timed_out)
}

/// The first candidate the code answers today, where the mutant answers otherwise: another value,
/// a panic, or, where its run ran out of time, nothing.
fn first_difference(
    candidates: &[Vec<i128>],
    today: &[String],
    mutated: &[String],
    timed_out: bool,
) -> Option<Suggestion> {
    for (index, values) in candidates.iter().enumerate() {
        let Some(value) = today.get(index).and_then(|line| line.strip_prefix("r ")) else {
            continue;
        };
        let with_mutant = match mutated.get(index).map(String::as_str) {
            Some("p") => "panics".to_string(),
            Some(line) => match line.strip_prefix("r ") {
                Some(other) if other != value => other.to_string(),
                _ => continue,
            },
            None if timed_out => "doesn't return".to_string(),
            None => return None,
        };
        return Some(Suggestion {
            values: values.clone(),
            today: value.to_string(),
            mutated: with_mutant,
        });
    }
    None
}

/// The test to paste for `suggestion`, in the module beside the function's tests (`file` is the
/// function's), written as its tests are; `id` names the mutant when the card has others.
pub fn test(
    callable: &Callable,
    file: &Path,
    suggestion: &Suggestion,
    id: Option<&str>,
) -> SuggestedTest {
    let (place, module, prefix, existing) = match &callable.tests {
        Some(tests) => {
            let module = joined(&callable.scope, &tests.name);
            let prefix = if tests.glob { "" } else { "super::" };
            (format!("mod {module}"), module, prefix, tests.text.as_str())
        }
        None => {
            let inside = if callable.file_scope {
                file.display().to_string()
            } else {
                format!("mod {}", callable.scope)
            };
            (
                format!("a new #[cfg(test)] mod tests in {inside}"),
                joined(&callable.scope, "tests"),
                "super::",
                "",
            )
        }
    };
    let values: Vec<String> = suggestion.values.iter().map(i128::to_string).collect();
    let call = callable.call(&values, prefix);
    let name = free_name(&name(callable, &suggestion.values), existing);
    let with = id.map_or("this mutant".to_string(), str::to_string);
    let outcomes = format!(
        "today {} · with {with} {}",
        suggestion.today, suggestion.mutated
    );
    let assertion = match suggestion.today.as_str() {
        "true" => format!("assert!({call});"),
        "false" => format!("assert!(!{call});"),
        today => format!("assert_eq!({call}, {today});"),
    };
    SuggestedTest {
        place,
        module,
        name: name.clone(),
        lines: vec![
            "#[test]".to_string(),
            format!("fn {name}() {{"),
            format!("    {assertion} // {outcomes}"),
            "}".to_string(),
        ],
        call,
        today: suggestion.today.clone(),
        mutated: suggestion.mutated.clone(),
    }
}

fn joined(scope: &str, name: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        format!("{scope}::{name}")
    }
}

/// `late_fee_with_days_late_7`; `size_result` with no arguments; and the receiver's arguments
/// too when it takes some, `open_on_with_day_5_for_window_5_and_10`. Every argument is in the
/// name, so two calls never share one, and one call (for two mutants) is one test.
fn name(callable: &Callable, values: &[i128]) -> String {
    let taken = callable
        .receiver
        .as_ref()
        .map_or(0, |receiver| receiver.parameters.len());
    let (made, own) = values.split_at(taken.min(values.len()));
    let mut name = callable.name.clone();
    if own.is_empty() {
        name.push_str("_result");
    } else {
        let arguments: Vec<String> = callable
            .parameters
            .iter()
            .zip(own)
            .map(|((parameter, _), value)| format!("{parameter}_{}", number(*value)))
            .collect();
        name.push_str(&format!("_with_{}", arguments.join("_and_")));
    }
    if let (Some(receiver), false) = (&callable.receiver, made.is_empty()) {
        let arguments: Vec<String> = made.iter().map(|value| number(*value)).collect();
        name.push_str(&format!(
            "_for_{}_{}",
            snake(&receiver.type_name),
            arguments.join("_and_")
        ));
    }
    name
}

fn number(value: i128) -> String {
    if value < 0 {
        format!("minus_{}", value.unsigned_abs())
    } else {
        value.to_string()
    }
}

/// `http_server` for `HttpServer`.
fn snake(name: &str) -> String {
    let mut out = String::new();
    for (index, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// `name`, or `name_2`, `name_3`… if the test module already has a function called that.
fn free_name(name: &str, existing: &str) -> String {
    let taken = |candidate: &str| {
        existing.contains(&format!("fn {candidate}("))
            || existing.contains(&format!("fn {candidate} ("))
    };
    let mut free = name.to_string();
    let mut count = 2;
    while taken(&free) {
        free = format!("{name}_{count}");
        count += 1;
    }
    free
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sites::callable::{Made, Receiver, TestModule};

    fn late_fee() -> Callable {
        Callable {
            name: "late_fee".into(),
            receiver: None,
            owner: None,
            parameters: vec![("days_late".into(), "i64".into())],
            returns: "i64".into(),
            after: 0,
            cfgs: Vec::new(),
            scope: "rates".into(),
            file_scope: false,
            tests: Some(TestModule {
                name: "tests".into(),
                glob: true,
                text: "fn late_fee_with_days_late_7() {}".into(),
            }),
        }
    }

    #[test]
    fn candidates_start_near_the_change_and_fit_their_types() {
        let inputs = BTreeMap::from([("total".to_string(), vec![50, 250])]);
        let found = candidates(&["u32"], &[100], &inputs);
        let firsts: Vec<i128> = found.iter().map(|values| values[0]).collect();
        assert_eq!(firsts, [100, 99, 101, 50, 250, 0, 1, 2, 3, 10]);
        let pairs = candidates(&["i8", "i8"], &[], &BTreeMap::new());
        assert_eq!(pairs[..3], [vec![0, 0], vec![0, 1], vec![0, 2]]);
        assert_eq!(pairs.len(), 36);
        assert_eq!(constants("if days_late <= 0x7 && x > 12u8 {"), [7, 12]);
    }

    #[test]
    fn the_first_input_answered_two_ways_is_kept() {
        let candidates = vec![vec![1], vec![7], vec![8]];
        let today: Vec<String> = ["r 0", "r 0", "r 2"].map(String::from).to_vec();
        let mutated: Vec<String> = ["r 0", "r 5"].map(String::from).to_vec();
        assert_eq!(
            first_difference(&candidates, &today, &mutated, false),
            Some(Suggestion {
                values: vec![7],
                today: "0".into(),
                mutated: "5".into()
            })
        );
        let stuck: Vec<String> = ["r 0"].map(String::from).to_vec();
        assert_eq!(
            first_difference(&candidates, &today, &stuck, true).map(|s| s.mutated),
            Some("doesn't return".into())
        );
        assert_eq!(first_difference(&candidates, &today, &stuck, false), None);
        let panicked: Vec<String> = ["p"].map(String::from).to_vec();
        assert_eq!(
            first_difference(&candidates, &today, &panicked, false).map(|s| s.mutated),
            Some("panics".into())
        );
    }

    #[test]
    fn a_test_is_written_as_the_modules_tests_are() {
        let found = Suggestion {
            values: vec![7],
            today: "0".into(),
            mutated: "5".into(),
        };
        let written = test(&late_fee(), Path::new("src/lib.rs"), &found, None);
        assert_eq!(written.place, "mod rates::tests");
        assert_eq!(
            written.lines,
            [
                "#[test]",
                "fn late_fee_with_days_late_7_2() {",
                "    assert_eq!(late_fee(7), 0); // today 0 · with this mutant 5",
                "}"
            ]
        );
        let window = Callable {
            name: "open_on".into(),
            receiver: Some(Receiver {
                type_name: "OpenWindow".into(),
                made: Made::Fields(vec!["start".into(), "end".into()]),
                parameters: vec![("start".into(), "i32".into()), ("end".into(), "i64".into())],
            }),
            parameters: vec![("day".into(), "i32".into())],
            returns: "bool".into(),
            file_scope: true,
            scope: String::new(),
            tests: None,
            ..late_fee()
        };
        let open = Suggestion {
            values: vec![5, -1, 5],
            today: "false".into(),
            mutated: "true".into(),
        };
        let written = test(&window, Path::new("src/lib.rs"), &open, Some("m1234abcd"));
        assert_eq!(written.place, "a new #[cfg(test)] mod tests in src/lib.rs");
        assert_eq!(
            written.lines[1..3],
            [
                "fn open_on_with_day_5_for_open_window_5_and_minus_1() {",
                "    assert!(!super::OpenWindow { start: 5, end: -1 }.open_on(5)); // today false · with m1234abcd true"
            ]
        );
    }
}
