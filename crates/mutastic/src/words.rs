//! What a card says about a mutant: the change, the gap and the fix, in the Java edition's words.
//! Words are written with backticks around code, as the agent format and JSON show them; the
//! human format drops the backticks.

use std::collections::BTreeMap;

use crate::sites::{Change, Mutant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Advice {
    /// What the change is: "`>=` becomes `>`".
    pub change: String,
    /// Beside the caret: "changed to >".
    pub caret: String,
    /// The gap: "no test checks `discount()` at `total == 100`".
    pub headline: String,
    pub notes: Vec<String>,
    /// The fix: "test `discount()` with `total = 100` and assert the result".
    pub help: String,
}

/// The advice for `mutant`, in function `name`, given the inputs its tests passed.
pub fn advise(mutant: &Mutant, name: &str, seen: &BTreeMap<String, Vec<i128>>) -> Advice {
    let mut advice = advise_change(mutant, name, seen);
    if let Some(near) = mutant.near_unsafe {
        advice.notes.push(format!(
            "this code is near unsafe code ({near}): a change no test notices here can break what the unsafe code relies on, so check that code with Miri too"
        ));
    }
    advice
}

fn advise_change(mutant: &Mutant, name: &str, seen: &BTreeMap<String, Vec<i128>>) -> Advice {
    let method = format!("`{name}()`");
    match &mutant.change {
        Change::Boundary { before, after } => boundary(&method, mutant, before, after, seen),
        Change::Always { condition, truth } => {
            let other = !truth;
            Advice {
                change: format!("{condition} is always {truth}"),
                caret: format!("now always {truth}"),
                headline: format!("no test needs {condition} to be {other} in {method}"),
                notes: inputs_seen(seen),
                help: format!(
                    "add a test where {condition} is {other}, and assert what {method} does then"
                ),
            }
        }
        Change::Reversed { before, after } => {
            let condition = format!("`{} {before} {}`", mutant.operands[0], mutant.operands[1]);
            Advice {
                change: format!("`{before}` becomes `{after}`"),
                caret: format!("changed to {after}"),
                headline: format!("{method} behaves the same with {condition} reversed"),
                notes: inputs_seen(seen),
                help: format!("assert the result on both sides of {condition}"),
            }
        }
        Change::Connector { before, after } => {
            let sides = format!(
                "one of `{}` and `{}`",
                mutant.operands[0], mutant.operands[1]
            );
            Advice {
                change: format!("`{before}` becomes `{after}`"),
                caret: format!("changed to {after}"),
                headline: format!("no test checks {method} with {sides} true and the other false"),
                notes: inputs_seen(seen),
                help: format!(
                    "add a test where {sides} is true and the other false, and assert what {method} does then"
                ),
            }
        }
        Change::Math { before, after } => {
            let (left, right) = (&mutant.operands[0], &mutant.operands[1]);
            Advice {
                change: format!("`{before}` becomes `{after}`"),
                caret: format!("changed to {after}"),
                headline: format!("tests pass even when `{before}` becomes `{after}` in {method}"),
                notes: inputs_seen(seen),
                help: format!(
                    "use inputs where `{left} {before} {right}` and `{left} {after} {right}` differ, and assert the result"
                ),
            }
        }
        Change::Returns { value } => returns(&method, value, seen),
        Change::Swapped { before, after } => Advice {
            change: format!("`{before}` becomes `{after}`"),
            caret: format!("changed to {after}"),
            headline: format!("tests pass even when `{before}` becomes `{after}` in {method}"),
            notes: inputs_seen(seen),
            help: format!(
                "use inputs where `{before}` and `{after}` give different results, and assert what {method} does"
            ),
        },
        Change::Removed { statement } => Advice {
            change: format!("`{statement}` is removed"),
            caret: "removed".into(),
            headline: format!("no test checks what `{statement}` does in {method}"),
            notes: inputs_seen(seen),
            help: format!("assert what `{statement}` changes, in a test of {method}"),
        },
        Change::Compared {
            before,
            after,
            differs,
        } => {
            let side = format!("`{} {differs} {}`", mutant.operands[0], mutant.operands[1]);
            Advice {
                change: format!("`{before}` becomes `{after}`"),
                caret: format!("changed to {after}"),
                headline: format!("no test checks {method} with {side}"),
                notes: inputs_seen(seen),
                help: format!("add a test where {side}, and assert what {method} does then"),
            }
        }
        Change::Parameter { name, value } => Advice {
            change: format!("`{name}` is `{value}` from the start of {method}"),
            caret: format!("now {value}"),
            headline: format!(
                "tests pass even when `{name}` is `{value}` from the start of {method}"
            ),
            notes: inputs_seen(seen),
            help: format!(
                "add a test where the `{name}` given changes what {method} does, and assert it"
            ),
        },
        Change::Replaced { before, after } if after.is_empty() => Advice {
            change: format!("`{before}` is left out"),
            caret: "left out".into(),
            headline: format!("tests pass even without `{before}` in {method}"),
            notes: inputs_seen(seen),
            help: format!(
                "use inputs that `{before}` makes a difference to, and assert what {method} does"
            ),
        },
        Change::Replaced { before, after } => Advice {
            change: format!("`{before}` becomes `{after}`"),
            caret: format!("changed to {after}"),
            headline: format!("tests pass even when `{before}` becomes `{after}` in {method}"),
            notes: inputs_seen(seen),
            help: format!(
                "use inputs where `{before}` and `{after}` differ, and assert what {method} does"
            ),
        },
    }
}

fn boundary(
    method: &str,
    mutant: &Mutant,
    before: &str,
    after: &str,
    seen: &BTreeMap<String, Vec<i128>>,
) -> Advice {
    if before.starts_with("..") {
        return range_end(method, mutant, before, after, seen);
    }
    let comparison = Comparison::of(&mutant.operands);
    let headline = match &comparison {
        Some(found) => format!(
            "no test checks {method} at `{} == {}`",
            found.name, found.value
        ),
        None => format!("no test checks the boundary of `{before}` in {method}"),
    };
    let notes = comparison
        .as_ref()
        .and_then(|found| {
            let values = found.away_from_boundary(seen)?;
            Some(vec![
                format!("every test calls {method} away from the boundary"),
                format!("inputs seen: {} (none is {})", join(&values), found.value),
            ])
        })
        .unwrap_or_else(|| inputs_seen(seen));
    let help = match &comparison {
        Some(found) => format!(
            "test {method} with `{} = {}` and assert the result",
            found.name, found.value
        ),
        None => format!(
            "add a test at the boundary value, where `{before}` and `{after}` disagree, and assert the result"
        ),
    };
    Advice {
        change: format!("`{before}` becomes `{after}`"),
        caret: format!("changed to {after}"),
        headline,
        notes,
        help,
    }
}

/// `lo..hi` to `lo..=hi`, or back: whether the range's end is in it.
fn range_end(
    method: &str,
    mutant: &Mutant,
    before: &str,
    after: &str,
    seen: &BTreeMap<String, Vec<i128>>,
) -> Advice {
    let end = mutant.operands.get(1).cloned().unwrap_or_default();
    Advice {
        change: format!("`{before}` becomes `{after}`"),
        caret: format!("changed to {after}"),
        headline: format!("no test checks {method} where the range's end, `{end}`, matters"),
        notes: inputs_seen(seen),
        help: format!(
            "add a test that reaches the range's end, `{end}`, and assert what {method} does there"
        ),
    }
}

fn returns(method: &str, value: &str, seen: &BTreeMap<String, Vec<i128>>) -> Advice {
    let today = match value {
        "true" => Some("false".to_string()),
        "false" => Some("true".to_string()),
        _ if value.starts_with('1') => Some(format!("0{}", &value[1..])),
        _ => None,
    };
    let mut notes = vec![format!(
        "tests reach this return, but none fails when it returns `{value}`"
    )];
    notes.extend(inputs_seen(seen));
    let help = match (&today, value) {
        (Some(was), _) => format!("add a test where {method} returns `{was}` here, and assert it"),
        (None, "None") => format!("assert the value {method} returns on this path"),
        (None, _) if value.starts_with('0') => format!(
            "assert the value {method} returns on this path, with inputs where it isn't `{value}`"
        ),
        (None, _) => format!(
            "assert the value {method} returns on this path, with inputs where it isn't empty"
        ),
    };
    Advice {
        change: format!("returns `{value}` instead"),
        caret: format!("now returns {value}"),
        headline: match &today {
            Some(was) => format!("no test checks {method} returning `{was}` here"),
            None => format!("no test checks what {method} returns here"),
        },
        notes,
        help,
    }
}

/// "inputs seen: total = 50, 250".
fn inputs_seen(seen: &BTreeMap<String, Vec<i128>>) -> Vec<String> {
    if seen.is_empty() {
        return Vec::new();
    }
    let each: Vec<String> = seen
        .iter()
        .map(|(name, values)| format!("{name} = {}", join(values)))
        .collect();
    vec![format!("inputs seen: {}", each.join("; "))]
}

fn join(values: &[i128]) -> String {
    values
        .iter()
        .map(i128::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// A comparison of a name with a number: `total >= 100`, or `100 <= total`.
struct Comparison {
    name: String,
    value: String,
}

impl Comparison {
    fn of(operands: &[String]) -> Option<Comparison> {
        let [left, right] = operands else {
            return None;
        };
        if is_number(left) && !is_number(right) {
            return Some(Comparison {
                name: right.clone(),
                value: left.clone(),
            });
        }
        Some(Comparison {
            name: left.clone(),
            value: right.clone(),
        })
    }

    /// The inputs the tests passed for the name, when it's a parameter compared with a number and
    /// none of them is that number.
    fn away_from_boundary(&self, seen: &BTreeMap<String, Vec<i128>>) -> Option<Vec<i128>> {
        let values = seen.get(&self.name)?;
        let value: i128 = number(&self.value)?;
        (!values.is_empty() && !values.contains(&value)).then(|| values.clone())
    }
}

/// `-12`, `100`, `100u32`, `1_000`: a whole number, as the source writes it.
fn number(text: &str) -> Option<i128> {
    let end = text
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(text.len());
    let (digits, suffix) = text.split_at(end);
    let whole = [
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
    ];
    if !suffix.is_empty() && !whole.contains(&suffix) {
        return None;
    }
    digits.replace('_', "").parse().ok()
}

fn is_number(text: &str) -> bool {
    number(text).is_some()
}

/// Code without its backticks, for people at a terminal.
pub fn plain(text: &str) -> String {
    text.replace('`', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_read_as_the_source_writes_them() {
        assert_eq!(number("100"), Some(100));
        assert_eq!(number("-12"), Some(-12));
        assert_eq!(number("1_000u32"), Some(1000));
        assert_eq!(number("total"), None);
        assert_eq!(number("100.0"), None);
    }
}
