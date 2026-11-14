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
    /// The covering test the receiver is made as, `tests::small_orders`, when the call makes it
    /// the same way: its reader knows that one, and a field it keeps has the tests' value.
    pub built_by: Option<String>,
}

/// How a covering test makes the receiver a call needs, read from its source: `Delivery::new(50)`
/// or `Pair(1, 2)`, with whole numbers written out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Built {
    /// The test, as the run names it.
    pub test: String,
    pub values: Vec<i128>,
}

/// The first of `covered_by` whose function, in the test module beside `callable`, makes its
/// receiver with whole numbers alone.
pub fn built(callable: &Callable, covered_by: &[String]) -> Option<Built> {
    let receiver = callable.receiver.as_ref()?;
    let (opener, closer, fields) = match &receiver.made {
        crate::sites::callable::Made::New => (format!("{}::new(", receiver.type_name), ')', None),
        crate::sites::callable::Made::Tuple => (format!("{}(", receiver.type_name), ')', None),
        crate::sites::callable::Made::Fields(names) => {
            (format!("{} {{", receiver.type_name), '}', Some(names))
        }
        crate::sites::callable::Made::Unit => return None,
    };
    let tests = &callable.tests.as_ref()?.text;
    let arity = receiver.parameters.len();
    if arity == 0 {
        return None;
    }
    covered_by.iter().find_map(|test| {
        // A generated case, `adds::case_1`, is written as the function a segment before it names.
        let body = test.rsplit("::").find_map(|name| fn_body(tests, name))?;
        let values = first_made(body, &opener, closer, fields.map(Vec::as_slice), arity)?;
        Some(Built {
            test: test.clone(),
            values,
        })
    })
}

/// The body of `fn name(..) { .. }` in `text`.
fn fn_body<'t>(text: &'t str, name: &str) -> Option<&'t str> {
    let at = text.find(&format!("fn {name}("))?;
    let open = at + text[at..].find('{')?;
    let mut depth = 0;
    for (index, c) in text[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[open..open + index]);
                }
            }
            _ => {}
        }
    }
    None
}

/// The values of the first `opener ..closer` in `body` made of `arity` whole numbers alone: in
/// order, or for a struct literal, by `fields`, `Delivery { free_from: 20 }`.
fn first_made(
    body: &str,
    opener: &str,
    closer: char,
    fields: Option<&[String]>,
    arity: usize,
) -> Option<Vec<i128>> {
    let mut from = 0;
    while let Some(found) = body[from..].find(opener) {
        let at = from + found;
        let whole = !body[..at]
            .chars()
            .last()
            .is_some_and(|c| c.is_alphanumeric() || c == '_');
        let after = &body[at + opener.len()..];
        if let (true, Some(close)) = (whole, after.find(closer)) {
            let values = match fields {
                Some(names) => by_field(&after[..close], names),
                None => after[..close]
                    .split(',')
                    .map(|word| crate::sites::integer_literal(word).map(|(value, _)| value))
                    .collect(),
            };
            if let Some(values) = values.filter(|values| values.len() == arity) {
                return Some(values);
            }
        }
        from = at + opener.len();
    }
    None
}

/// `free_from: 20, days: 3` as each of `names`'s value, in their order.
fn by_field(text: &str, names: &[String]) -> Option<Vec<i128>> {
    let given: Vec<(&str, &str)> = text
        .split(',')
        .filter(|part| !part.trim().is_empty())
        .map(|part| part.split_once(':'))
        .collect::<Option<_>>()?;
    names
        .iter()
        .map(|name| {
            let (_, value) = given.iter().find(|(field, _)| field.trim() == name)?;
            crate::sites::integer_literal(value).map(|(value, _)| value)
        })
        .collect()
}

/// The inputs to try with the receiver made as `made`, first: as [`candidates`], its values first.
pub fn candidates_made(
    types: &[&str],
    constants: &[i128],
    inputs: &BTreeMap<String, Vec<i128>>,
    made: &[i128],
) -> Vec<Vec<i128>> {
    let mut first: Vec<i128> = made.to_vec();
    let rest: Vec<i128> = constants.to_vec();
    first.extend(rest);
    let own = &types[made.len().min(types.len())..];
    candidates(own, &first, inputs)
        .into_iter()
        .map(|values| [made, values.as_slice()].concat())
        .collect()
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
/// tries them, the earlier first, leaving out a value its type can't hold. Where a property test
/// passed more values than a card shows, the small numbers come before the values seen.
pub fn candidates(
    types: &[&str],
    constants: &[i128],
    inputs: &BTreeMap<String, Vec<i128>>,
) -> Vec<Vec<i128>> {
    let mut values: Vec<i128> = Vec::new();
    let near = constants
        .iter()
        .flat_map(|&constant| [constant, constant - 1, constant + 1]);
    let small = [0, 1, 2, 3, -1, 10];
    // A name a property test passed hundreds of values: the values are a generator's, and a test
    // named for one reads as noise, so the small numbers come before them.
    let generated = inputs
        .values()
        .any(|values| values.len() > crate::words::INPUTS_SHOWN);
    let seen: Vec<i128> = inputs.values().flatten().copied().collect();
    let (first, then) = if generated {
        (small.to_vec(), seen)
    } else {
        (seen, small.to_vec())
    };
    for value in near.chain(first).chain(then) {
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

/// The values of the `const`s `line` names that `text` declares, `FREE_FROM` for `const FREE_FROM:
/// i32 = 100;`: constants near a change, as the Java edition finds the ones javac wrote in.
pub fn named_constants(line: &str, text: &str) -> Vec<i128> {
    line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|word| word.starts_with(|c: char| c.is_ascii_uppercase()))
        .filter(|word| {
            word.chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        })
        .filter_map(|word| declared_value(text, word))
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
            built_by: None,
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
    named: Option<&Named>,
    edge: Option<&Edge>,
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
    let mut values: Vec<String> = suggestion.values.iter().map(i128::to_string).collect();
    if let Some(named) = named {
        for (&at, written) in &named.arguments {
            values[at] = written.replace("{prefix}", prefix);
        }
    }
    let call = callable.call(&values, prefix);
    let (made_as, call) = made_as_a_test(callable, suggestion, &values, prefix, call);
    let name = match named {
        Some(named) => format!("{}_at_{}", callable.name, named.name.to_lowercase()),
        None => where_it_sits(callable, &suggestion.values, edge)
            .unwrap_or_else(|| name(callable, &suggestion.values)),
    };
    let name = free_name(&name, existing);
    let with = id.map_or("this mutant".to_string(), str::to_string);
    let note = named
        .filter(|named| !named.note.is_empty())
        .map_or(String::new(), |named| format!("{} · ", named.note));
    let outcomes = format!(
        "{note}today {} · with {with} {}",
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
        lines: [
            vec!["#[test]".to_string(), format!("fn {name}() {{")],
            named
                .and_then(|named| named.local.clone())
                .map(|local| format!("    {local}"))
                .into_iter()
                .chain(made_as.map(|line| format!("    {line}")))
                .collect(),
            vec![format!("    {assertion} // {outcomes}"), "}".to_string()],
        ]
        .concat(),
        call,
        today: suggestion.today.clone(),
        mutated: suggestion.mutated.clone(),
    }
}

/// A receiver made as a covering test makes it, declared as that test does, `let delivery =
/// Delivery::new(50); // as tests::small_orders builds it`, and the call on it; else the call as
/// it is.
fn made_as_a_test(
    callable: &Callable,
    suggestion: &Suggestion,
    values: &[String],
    prefix: &str,
    call: String,
) -> (Option<String>, String) {
    let (Some(test), Some(receiver)) = (&suggestion.built_by, &callable.receiver) else {
        return (None, call);
    };
    let made = receiver.make(
        &values[..receiver.parameters.len().min(values.len())],
        prefix,
    );
    let Some(rest) = call.strip_prefix(&made) else {
        return (None, call);
    };
    let local = snake(&receiver.type_name);
    (
        Some(format!("let {local} = {made}; // as {test} builds it")),
        format!("{local}{rest}"),
    )
}

/// `free_shipping` for `FreeShipping`.
fn snake(type_name: &str) -> String {
    let mut out = String::new();
    for (index, c) in type_name.chars().enumerate() {
        if c.is_ascii_uppercase() && index > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

fn joined(scope: &str, name: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        format!("{scope}::{name}")
    }
}

/// The literal a condition compares a parameter with: `total` and 100 in `total >= 100`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    pub parameter: String,
    pub value: i128,
}

/// The edge a condition's mutant compares, from its operator and operands; none for another
/// mutant, whose operands (`amount * 10`) have no edge.
pub fn edge(operator: &str, operands: &[String]) -> Option<Edge> {
    let condition = [
        crate::sites::BOUNDARY,
        crate::sites::COMPARISON,
        crate::sites::OTHER_COMPARISON,
        crate::sites::REMOVE_CONDITIONAL,
    ]
    .contains(&operator);
    let [left, right] = operands else {
        return None;
    };
    let literal = |text: &str| crate::sites::integer_literal(text).map(|(value, _)| value);
    let (parameter, value) = match (literal(left), literal(right)) {
        (None, Some(value)) => (left, value),
        (Some(value), None) => (right, value),
        _ => return None,
    };
    condition.then(|| Edge {
        parameter: parameter.clone(),
        value,
    })
}

/// The name of a call of one argument, said by where its value sits rather than the value, which
/// goes stale when the spec moves: zero or below, `fee_when_amount_is_zero`, first, since that's
/// the call's alone, so every mutant suggesting it names it the same; then the edge of the
/// literal the condition compares it with, or one either side, `loyalty_credit_at_the_total_edge`,
/// `…_just_below_the_total_edge`. As the Java edition names it. None otherwise.
fn where_it_sits(callable: &Callable, values: &[i128], edge: Option<&Edge>) -> Option<String> {
    let taken = callable
        .receiver
        .as_ref()
        .map_or(0, |receiver| receiver.parameters.len());
    let ([value], [(parameter, _)]) =
        (&values[taken.min(values.len())..], &callable.parameters[..])
    else {
        return None;
    };
    if taken > 0 {
        return None;
    }
    let name = &callable.name;
    if *value <= 0 {
        let class = if *value == 0 { "zero" } else { "negative" };
        return Some(format!("{name}_when_{parameter}_is_{class}"));
    }
    let edge = edge.filter(|edge| &edge.parameter == parameter)?;
    let side = match *value - edge.value {
        0 => "at",
        -1 => "just_below",
        1 => "just_above",
        _ => return None,
    };
    Some(format!("{name}_{side}_the_{parameter}_edge"))
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
        let arguments: Vec<String> = receiver
            .parameters
            .iter()
            .zip(made)
            .map(|((parameter, _), value)| format!("{parameter}_{}", number(*value)))
            .collect();
        name.push_str(&format!("_for_{}", arguments.join("_and_")));
    }
    name
}

/// The value a suggested call is at the edge of, when the source names it, which the test uses
/// and is named after, as the Java edition names it: a `const` the comparison reads, passed its
/// value; or a field the receiver is made with, passed the value the function's argument is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    /// `FREE_FROM`, or `free_from`.
    pub name: String,
    /// What the test's comment says of it, `FREE_FROM = 100`; empty when nothing.
    pub note: String,
    /// The line the test declares the value in, when it passes it twice: `let free_from = 20;`.
    pub local: Option<String>,
    /// The call's values written otherwise, by index (the receiver's first), `{prefix}` where a
    /// path from the test module goes.
    pub arguments: BTreeMap<usize, String>,
    /// What the name is, and its value where it's known, for a card's evidence: `FREE_FROM
    /// (FREE_FROM = 100)`, `free_from = 50 (Delivery's field, from Delivery::new(50))`.
    pub meaning: String,
}

/// How a call says the value the comparison `operands` is at, if the source names it: a `const`
/// declared in `text`, the function's file, which every test module under it can name; or a field
/// of the receiver.
pub fn named(
    callable: &Callable,
    operands: &[String],
    text: &str,
    values: &[i128],
    built_by: Option<&str>,
) -> Option<Named> {
    let [left, right] = operands else {
        return None;
    };
    let taken = callable
        .receiver
        .as_ref()
        .map_or(0, |receiver| receiver.parameters.len());
    for (parameter, other) in [(left, right), (right, left)] {
        let Some(at) = callable
            .parameters
            .iter()
            .position(|(name, _)| name == parameter)
        else {
            continue;
        };
        let Some(&value) = values.get(taken + at) else {
            continue;
        };
        let found = match other.strip_prefix("self.") {
            Some(field) => tied(callable, field, values, taken + at, built_by),
            None => constant(callable, other, text, value, taken + at),
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

/// A field the receiver is made with the same value as the argument at `at`: one local, passed
/// to both.
fn tied(
    callable: &Callable,
    field: &str,
    values: &[i128],
    at: usize,
    built_by: Option<&str>,
) -> Option<Named> {
    let receiver = callable.receiver.as_ref()?;
    let made = receiver
        .parameters
        .iter()
        .position(|(name, _)| name == field)?;
    let parameter = callable.parameters.get(at - receiver.parameters.len())?;
    // One `let` passes as both only if they're one type: Rust doesn't widen.
    if values.get(made) != values.get(at) || receiver.parameters[made].1 != parameter.1 {
        return None;
    }
    let owner = &receiver.type_name;
    if built_by.is_some() {
        // Made as a covering test makes it: its value is the test's, said, not a local's.
        let taken = receiver.parameters.len();
        let made_as = receiver.make(
            &values[..taken]
                .iter()
                .map(i128::to_string)
                .collect::<Vec<_>>(),
            "",
        );
        return Some(Named {
            name: field.to_string(),
            note: format!("{} == {field}", parameter.0),
            local: None,
            arguments: BTreeMap::new(),
            meaning: format!("{field} = {} ({owner}'s field, from {made_as})", values[at]),
        });
    }
    Some(Named {
        name: field.to_string(),
        note: String::new(),
        local: Some(format!("let {field} = {};", values[at])),
        arguments: BTreeMap::from([(made, field.to_string()), (at, field.to_string())]),
        meaning: format!("{field} ({owner}'s field, set when it's made)"),
    })
}

/// A `const` of `value`, declared in `text`, that `written` names: `FREE_FROM`, `Self::FREE_FROM`
/// or `Pricing::FREE_FROM`.
fn constant(
    callable: &Callable,
    written: &str,
    text: &str,
    value: i128,
    at: usize,
) -> Option<Named> {
    let name = written.rsplit("::").next()?;
    let is_constant = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        && name.starts_with(|c: char| c.is_ascii_uppercase());
    if !is_constant || declared_value(text, name)? != value {
        return None;
    }
    let path = match written.strip_prefix("Self::") {
        Some(rest) => {
            let owner = callable
                .receiver
                .as_ref()
                .map(|receiver| receiver.type_name.clone())
                .or_else(|| callable.owner.clone())?;
            format!("{{prefix}}{owner}::{rest}")
        }
        None if written.contains("::") => written.to_string(),
        None => format!("{{prefix}}{written}"),
    };
    let meaning = format!("{name} ({} = {value})", path.replace("{prefix}", ""));
    Some(Named {
        name: name.to_string(),
        note: format!("{name} = {value}"),
        local: None,
        arguments: BTreeMap::from([(at, path)]),
        meaning,
    })
}

/// The value `const NAME: T = value;` gives, when `text` declares it once, as a whole number.
fn declared_value(text: &str, name: &str) -> Option<i128> {
    let declaration = format!("const {name}:");
    let mut found = text.match_indices(&declaration);
    let (at, _) = found.next()?;
    if found.next().is_some() {
        return None;
    }
    let rest = &text[at + declaration.len()..];
    let value = rest[rest.find('=')? + 1..rest.find(';')?].trim();
    crate::sites::integer_literal(value).map(|(value, _)| value)
}

fn number(value: i128) -> String {
    if value < 0 {
        format!("minus_{}", value.unsigned_abs())
    } else {
        value.to_string()
    }
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

    fn delivery() -> Callable {
        Callable {
            name: "is_free".into(),
            receiver: Some(Receiver {
                type_name: "Delivery".into(),
                made: Made::New,
                parameters: vec![("free_from".into(), "u32".into())],
            }),
            parameters: vec![("total".into(), "u32".into())],
            returns: "bool".into(),
            tests: Some(TestModule {
                name: "tests".into(),
                glob: true,
                text: "#[test]\nfn other() { let d = Delivery::new(70); }\n\
                       #[test]\nfn small_orders() {\n    let delivery = Delivery::new(50);\n    \
                       assert!(!delivery.is_free(20));\n}\n"
                    .into(),
            }),
            ..late_fee()
        }
    }

    /// A covering test's own function says how it makes the receiver, with whole numbers alone.
    #[test]
    fn a_covering_test_makes_the_receiver() {
        let callable = delivery();
        assert_eq!(
            built(&callable, &["tests::small_orders".to_string()]),
            Some(Built {
                test: "tests::small_orders".into(),
                values: vec![50]
            })
        );
        assert_eq!(built(&callable, &["tests::gone".to_string()]), None);
        // A generated case, rstest's or test-case's, is written as its function.
        assert_eq!(
            built(&callable, &["tests::small_orders::case_1".to_string()]).map(|made| made.values),
            Some(vec![50])
        );
        let literal = Callable {
            receiver: Some(Receiver {
                type_name: "Delivery".into(),
                made: Made::Fields(vec!["free_from".into()]),
                parameters: vec![("free_from".into(), "u32".into())],
            }),
            tests: Some(TestModule {
                name: "tests".into(),
                glob: true,
                text: "fn big_orders() { let d = Delivery { free_from: 20 }; }".into(),
            }),
            ..delivery()
        };
        assert_eq!(
            built(&literal, &["rates::tests::big_orders".to_string()]).map(|made| made.values),
            Some(vec![20])
        );
        assert_eq!(
            built(&late_fee(), &["tests::small_orders".to_string()]),
            None
        );
        let made = candidates_made(&["u32", "u32"], &[], &BTreeMap::new(), &[50]);
        assert_eq!(made[0], [50, 50]);
        assert!(made.iter().all(|values| values[0] == 50));
    }

    /// Made as a covering test makes it, the receiver is declared as that test does, and the value
    /// the call is at is the test's, said.
    #[test]
    fn a_receiver_made_as_a_test_makes_it_says_which_test() {
        let callable = delivery();
        let found = Suggestion {
            values: vec![50, 50],
            today: "true".into(),
            mutated: "false".into(),
            built_by: Some("tests::small_orders".into()),
        };
        let operands = ["total".to_string(), "self.free_from".to_string()];
        let named = named(
            &callable,
            &operands,
            "",
            &found.values,
            found.built_by.as_deref(),
        )
        .unwrap();
        assert_eq!(
            named.meaning,
            "free_from = 50 (Delivery's field, from Delivery::new(50))"
        );
        let written = test(
            &callable,
            Path::new("src/lib.rs"),
            &found,
            None,
            Some(&named),
            None,
        );
        assert_eq!(
            written.lines,
            [
                "#[test]",
                "fn is_free_at_free_from() {",
                "    let delivery = Delivery::new(50); // as tests::small_orders builds it",
                "    assert!(delivery.is_free(50)); // total == free_from · today true · with this mutant false",
                "}"
            ]
        );
        let unbuilt = super::named(&callable, &operands, "", &found.values, None).unwrap();
        assert_eq!(
            unbuilt.meaning,
            "free_from (Delivery's field, set when it's made)"
        );
    }

    #[test]
    fn a_generator_s_values_come_after_the_small_numbers() {
        let many = BTreeMap::from([("a".to_string(), (-500..-400).collect::<Vec<i128>>())]);
        let found = candidates(&["i64"], &[], &many);
        assert_eq!(found[..3], [vec![0], vec![1], vec![2]]);
        let few = BTreeMap::from([("a".to_string(), vec![-450, 7])]);
        assert_eq!(candidates(&["i64"], &[], &few)[0], [-450]);
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
                mutated: "5".into(),
                built_by: None,
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
            built_by: None,
        };
        let written = test(
            &late_fee(),
            Path::new("src/lib.rs"),
            &found,
            None,
            None,
            None,
        );
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
            built_by: None,
        };
        let written = test(
            &window,
            Path::new("src/lib.rs"),
            &open,
            Some("m1234abcd"),
            None,
            None,
        );
        assert_eq!(written.place, "a new #[cfg(test)] mod tests in src/lib.rs");
        assert_eq!(
            written.lines[1..3],
            [
                "fn open_on_with_day_5_for_start_5_and_end_minus_1() {",
                "    assert!(!super::OpenWindow { start: 5, end: -1 }.open_on(5)); // today false · with m1234abcd true"
            ]
        );
    }
}
