//! What `mutastic next` says: the missed mutants as a queue, the one most worth a test at its head,
//! so a person or an agent works through them one at a time instead of reading a list. The order
//! is the run's own ([`Standing::order`]); `--top` spreads what it shows over the code, at most
//! [`PER_FUNCTION`] item a function and [`PER_FILE`] a file, so one function with many misses can't
//! fill it. The words are the Java edition's `NextText`.
//!
//! Where the package has a baseline (`mutastic baseline`), the lines with a new miss come before
//! those whose every mutant the baseline already has, each in the run's order.
//!
//! The mutants on one line are one item, as they share a card. A mutant proposed for ignoring
//! (`ignore --propose`) isn't queued: the next step is a person's, to accept or withdraw it, and
//! handing it back would have an agent decline it again. It's counted as waiting instead.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde_json::{json, Value};

use super::{
    agent_entry, brief_card, card, id_list, is_legacy, mutant_event, new_first, ranked, Entry,
    Format, Standing, Style, View, DECLINE, EDITION, RULE, SCHEMA,
};
use crate::engine::{Report, Status};

/// The most `--top` shows: more than a few at a time go unread.
pub const MOST: usize = 25;

/// The order's name in `--format=jsonl`, the Java edition's: it changes only with a release.
pub const ORDER: &str = "run-1";

/// The most items `--top` shows from one function.
pub const PER_FUNCTION: usize = 1;

/// The most items `--top` shows from one file.
pub const PER_FILE: usize = 2;

/// What `next` was asked for, besides the run it ranks.
pub struct Asked<'a> {
    /// The ids proposed for ignoring, which wait on a person and aren't queued.
    pub proposed: &'a BTreeSet<String>,
    /// The ids in the package's baseline, whose misses come after the new ones: none without one.
    pub baseline: &'a BTreeSet<String>,
    /// The package's name, as the header says it.
    pub project: &'a str,
    /// The directory `mutastic` was pointed at, as the commands offered name it.
    pub dir: &'a str,
    pub format: Format,
    pub style: Style,
    /// How many items to show: `--top`, 1 unless given.
    pub top: usize,
}

/// The queue: every missed line but those proposed for ignoring, new misses before the baseline's,
/// then in the run's order; how many missed mutants wait on a person; and how many of the lines
/// the baseline already has.
struct Queue<'a> {
    entries: Vec<Entry<'a>>,
    waiting: usize,
    legacy: usize,
}

impl<'a> Queue<'a> {
    fn of(view: &View<'a>, proposed: &BTreeSet<String>, baseline: &BTreeSet<String>) -> Queue<'a> {
        let ranked = ranked(
            view,
            view.entries_where(|result| !proposed.contains(view.id(result))),
        );
        let entries = new_first(view, ranked, baseline);
        let legacy = entries
            .iter()
            .filter(|entry| is_legacy(view, entry, baseline))
            .count();
        let waiting = view
            .report
            .results
            .iter()
            .filter(|result| result.status == Status::Survived)
            .filter(|result| proposed.contains(view.id(result)))
            .count();
        Queue {
            entries,
            waiting,
            legacy,
        }
    }

    /// How many mutants are queued.
    fn mutants(&self) -> usize {
        self.entries.iter().map(|entry| 1 + entry.also.len()).sum()
    }

    /// The first `count` items, spread over the code.
    fn top(&self, view: &View, count: usize) -> Vec<&Entry<'a>> {
        let places = self.entries.iter().map(|entry| {
            let mutant = view.mutant(entry.main);
            (mutant.function, mutant.file.clone())
        });
        spread(places, count)
            .into_iter()
            .map(|at| &self.entries[at])
            .collect()
    }
}

/// Which of the items, at `places` (each its function and its file) in the queue's order, `--top`
/// shows: the first `count`, passing over any whose function already has [`PER_FUNCTION`] shown or
/// whose file has [`PER_FILE`].
pub fn spread<F: Ord, P: Ord>(places: impl Iterator<Item = (F, P)>, count: usize) -> Vec<usize> {
    let mut shown = Vec::new();
    let mut by_function: BTreeMap<F, usize> = BTreeMap::new();
    let mut by_file: BTreeMap<P, usize> = BTreeMap::new();
    for (at, (function, file)) in places.enumerate() {
        if shown.len() == count {
            break;
        }
        let in_function = by_function.get(&function).copied().unwrap_or(0);
        let in_file = by_file.get(&file).copied().unwrap_or(0);
        if in_function < PER_FUNCTION && in_file < PER_FILE {
            shown.push(at);
            *by_function.entry(function).or_default() += 1;
            *by_file.entry(file).or_default() += 1;
        }
    }
    shown
}

/// Why an item is where it is, one thing the order went by: its name in the events, what it says,
/// and the value it was ranked on.
#[derive(Clone, Debug, PartialEq)]
pub struct Factor {
    pub signal: &'static str,
    pub why: String,
    pub value: Value,
}

/// Why an item standing at `standing` is where it is, factor by factor, in the order they're
/// applied: first, where there's a baseline, whether the baseline had it (`legacy`) or it's a new
/// miss; where it is in the code goes without saying.
pub fn factors(standing: &Standing, legacy: Option<bool>) -> Vec<Factor> {
    let baseline = legacy.map(|legacy| {
        if legacy {
            Factor {
                signal: "baseline",
                why: "in the baseline: missed before".into(),
                value: json!("legacy"),
            }
        } else {
            Factor {
                signal: "baseline",
                why: "not in the baseline: a new miss".into(),
                value: json!("new"),
            }
        }
    });
    let operator = if standing.prior == 2 {
        Factor {
            signal: "operator",
            why: "a condition or a removal: it changes what the code decides or does".into(),
            value: json!("condition-or-removal"),
        }
    } else {
        Factor {
            signal: "operator",
            why: "arithmetic or another change: more often noise".into(),
            value: json!("other"),
        }
    };
    let covering = standing.covering;
    let one = covering == 1;
    let tests = Factor {
        signal: "coveringTests",
        why: format!(
            "{covering} test{} run{} it, and none checks it",
            if one { "" } else { "s" },
            if one { "s" } else { "" }
        ),
        value: json!(covering),
    };
    baseline.into_iter().chain([operator, tests]).collect()
}

/// Whether the baseline had an entry's every mutant, where there's a baseline to say.
fn legacy(view: &View, entry: &Entry, asked: &Asked) -> Option<bool> {
    (!asked.baseline.is_empty()).then(|| is_legacy(view, entry, asked.baseline))
}

/// `next`'s answer in the format asked, and whether anything is queued. `report` is the run it
/// ranks; none when the last `next` left nothing uncaught and the code is as it was then, so
/// nothing ran.
pub fn next(report: Option<&Report>, asked: &Asked) -> (String, bool) {
    let Some(report) = report else {
        return (nothing_left(asked, 0), false);
    };
    let view = View { report };
    let queue = Queue::of(&view, asked.proposed, asked.baseline);
    if queue.entries.is_empty() {
        return (nothing_left(asked, queue.waiting), false);
    }
    let text = match asked.format {
        Format::Jsonl => jsonl(&view, &queue, asked),
        Format::Agent => agent(&view, &queue, asked),
        Format::Full | Format::Brief | Format::Concise | Format::Grouped => {
            human(&view, &queue, asked)
        }
    };
    (text, true)
}

/// What's said when nothing missed is left, in each format.
fn nothing_left(asked: &Asked, waiting: usize) -> String {
    let words = format!(
        "nothing {} is left in {}{}",
        asked.style.words.undetected(),
        asked.project,
        waiting_on_a_person(waiting, " but ")
    );
    match asked.format {
        Format::Jsonl => format!("{}\n", finished(0, 0, 0, 0, waiting)),
        Format::Agent => format!("mutastic next: {words}\n"),
        Format::Full | Format::Brief | Format::Concise | Format::Grouped => {
            format!("Next: {words}\n")
        }
    }
}

/// For people: how many are queued, then each item's card with why it's ranked there, and what to
/// run for the next one.
fn human(view: &View, queue: &Queue, asked: &Asked) -> String {
    let style = asked.style;
    let shown = queue.top(view, asked.top);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{}",
        style.bold(&format!("Next: {}", header(queue, asked, shown.len())))
    );
    for (at, entry) in shown.iter().enumerate() {
        if asked.format == Format::Brief {
            brief_card(&mut out, view, entry, style);
        } else {
            card(&mut out, view, entry, style);
        }
        let _ = writeln!(out, " {}", style.bold(&ranked_because(at + 1)));
        for factor in factors(&view.standing(entry), legacy(view, entry, asked)) {
            let _ = writeln!(out, "   {}", factor.why);
        }
    }
    let _ = writeln!(
        out,
        "\nThen: {}, for the next one",
        super::next_command(asked.dir)
    );
    out
}

/// For an agent: one task, the head of the queue, whole: the gap, why it's first, the rules, when
/// it's done, how to decline it, and to ask for the next one after.
fn agent(view: &View, queue: &Queue, asked: &Asked) -> String {
    let Some(head) = queue.entries.first() else {
        return nothing_left(asked, queue.waiting);
    };
    let whys: Vec<String> = factors(&view.standing(head), legacy(view, head, asked))
        .into_iter()
        .map(|factor| factor.why)
        .collect();
    let ids = id_list(view, head);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "mutastic next: {}; close this one, then ask for the next",
        header(queue, asked, 1)
    );
    let _ = writeln!(out, "why first: {}", whys.join("; "));
    let _ = writeln!(
        out,
        "rules: {RULE}, and never delete, disable or loosen a test; it's done when `{}` passes",
        view.command("verify", &ids)
    );
    let _ = writeln!(out, "decline: {DECLINE} ({})\n", propose(view, &ids));
    out.push_str(&agent_entry(view, head, asked.style.words));
    let _ = writeln!(
        out,
        "\nthen: run `{}` again for the next one; when it says nothing is left, you're done",
        super::next_command(asked.dir)
    );
    out
}

/// How to propose ignoring the line's mutants instead, each on its own: `` `mutastic ignore …` ``,
/// or for several, the command with `<id>`, for each of them.
fn propose(view: &View, ids: &str) -> String {
    if ids.contains(',') {
        format!(
            "`{}`, for each of {}",
            view.command("propose", "<id>"),
            ids.replace(',', ", ")
        )
    } else {
        format!("`{}`", view.command("propose", ids))
    }
}

/// For tools: a `next-item` for each item shown, its lead mutant as `mutant-finished` has it, with
/// every id on its line and where it stands and why; then `next-finished`.
fn jsonl(view: &View, queue: &Queue, asked: &Asked) -> String {
    let shown = queue.top(view, asked.top);
    let mut lines: Vec<Value> = Vec::new();
    for (at, entry) in shown.iter().enumerate() {
        let mut event = mutant_event(view, entry.main);
        event["reason"] = json!("next-item");
        event["ids"] = json!(id_list(view, entry).split(',').collect::<Vec<_>>());
        let factors: Vec<Value> = factors(&view.standing(entry), legacy(view, entry, asked))
            .into_iter()
            .map(|factor| json!({ "signal": factor.signal, "value": factor.value, "why": factor.why }))
            .collect();
        event["rank"] = json!({ "position": at + 1, "order": ORDER, "factors": factors });
        lines.push(event);
    }
    lines.push(finished(
        queue.entries.len(),
        queue.mutants(),
        shown.len(),
        queue.legacy,
        queue.waiting,
    ));
    lines.iter().map(|line| format!("{line}\n")).collect()
}

/// `next-finished`: how many items and mutants are queued, how many items were shown, how many of
/// the queued items the baseline already has, and how many missed mutants wait on a person.
fn finished(queued: usize, mutants: usize, shown: usize, legacy: usize, waiting: usize) -> Value {
    json!({
        "schema": SCHEMA,
        "edition": EDITION,
        "reason": "next-finished",
        "queued": queued,
        "mutants": mutants,
        "shown": shown,
        "legacy": legacy,
        "waiting": waiting,
    })
}

/// `1 of 12 lines with missed mutants in pricing`, or `the first 2 of 12 …`, how many are new where
/// there's a baseline to say, and what waits on a person.
fn header(queue: &Queue, asked: &Asked, shown: usize) -> String {
    let all = queue.entries.len();
    let new = if asked.baseline.is_empty() {
        String::new()
    } else {
        format!(
            " ({} new, {} in the baseline)",
            all - queue.legacy,
            queue.legacy
        )
    };
    format!(
        "{} of {all} line{} with {} mutants in {}{new}{}",
        if shown == 1 {
            "1".to_string()
        } else {
            format!("the first {shown}")
        },
        if all == 1 { "" } else { "s" },
        asked.style.words.undetected(),
        asked.project,
        waiting_on_a_person(queue.waiting, "; ")
    )
}

/// `; 2 mutants proposed for ignoring, waiting on a person to accept them (…)`, after `joined`:
/// what the queue leaves out, since the next step there is a person's. Nothing when there's none.
pub fn waiting_on_a_person(waiting: usize, joined: &str) -> String {
    if waiting == 0 {
        return String::new();
    }
    let one = waiting == 1;
    format!(
        "{joined}{waiting} {} proposed for ignoring, waiting on a person to accept {} ({})",
        if one { "mutant" } else { "mutants" },
        if one { "it" } else { "them" },
        crate::ignores::PROPOSALS
    )
}

/// `Ranked first because`, `Ranked 2nd because`.
pub fn ranked_because(position: usize) -> String {
    let place = if position == 1 {
        "first".to_string()
    } else {
        ordinal(position)
    };
    format!("Ranked {place} because")
}

/// `2nd`, `11th`, `23rd`.
pub fn ordinal(number: usize) -> String {
    let suffix = match (number % 100, number % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{number}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standing(prior: u8, covering: usize, file: &str, line: usize) -> Standing {
        Standing {
            prior,
            covering,
            file: file.into(),
            line,
        }
    }

    /// The queue's order, as the Java edition's: a condition or a removal before arithmetic,
    /// whatever reaches it; then the line more tests run; then where it is in the code.
    #[test]
    fn the_queue_is_in_the_runs_order() {
        let mut queue = [
            standing(1, 3, "src/lib.rs", 6),
            standing(2, 1, "src/lib.rs", 11),
            standing(2, 3, "src/lib.rs", 15),
            standing(2, 3, "src/a.rs", 40),
            standing(2, 3, "src/lib.rs", 2),
        ];
        queue.sort_by(Standing::order);
        let places: Vec<(&str, usize)> = queue
            .iter()
            .map(|standing| (standing.file.as_str(), standing.line))
            .collect();
        assert_eq!(
            places,
            [
                ("src/a.rs", 40),
                ("src/lib.rs", 2),
                ("src/lib.rs", 15),
                ("src/lib.rs", 11),
                ("src/lib.rs", 6),
            ]
        );
    }

    /// `--top` takes at most one item a function and two a file, passing over the rest in order.
    #[test]
    fn top_spreads_over_functions_and_files() {
        let places = [
            ("a::f", "a.rs"),
            ("a::f", "a.rs"),
            ("a::g", "a.rs"),
            ("a::h", "a.rs"),
            ("b::f", "b.rs"),
            ("b::g", "b.rs"),
        ];
        assert_eq!(spread(places.into_iter(), 25), [0, 2, 4, 5]);
        assert_eq!(spread(places.into_iter(), 3), [0, 2, 4]);
        assert_eq!(spread(places.into_iter(), 1), [0]);
        assert_eq!(
            spread(std::iter::empty::<(u8, u8)>(), 5),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn each_factor_says_what_it_was_ranked_on() {
        let condition = factors(&standing(2, 1, "src/lib.rs", 1), None);
        assert_eq!(
            condition,
            [
                Factor {
                    signal: "operator",
                    why: "a condition or a removal: it changes what the code decides or does"
                        .into(),
                    value: json!("condition-or-removal"),
                },
                Factor {
                    signal: "coveringTests",
                    why: "1 test runs it, and none checks it".into(),
                    value: json!(1),
                },
            ]
        );
        let other = factors(&standing(1, 3, "src/lib.rs", 1), None);
        assert_eq!(other[0].value, json!("other"));
        assert_eq!(
            other[0].why,
            "arithmetic or another change: more often noise"
        );
        assert_eq!(other[1].why, "3 tests run it, and none checks it");
        let new = factors(&standing(1, 3, "src/lib.rs", 1), Some(false));
        assert_eq!(
            (new[0].signal, new[0].why.as_str(), &new[0].value),
            ("baseline", "not in the baseline: a new miss", &json!("new"))
        );
        let legacy = factors(&standing(1, 3, "src/lib.rs", 1), Some(true));
        assert_eq!(
            (legacy[0].why.as_str(), &legacy[0].value),
            ("in the baseline: missed before", &json!("legacy"))
        );
        assert_eq!(legacy.len(), 3);
    }

    #[test]
    fn positions_read_as_words() {
        assert_eq!(ranked_because(1), "Ranked first because");
        assert_eq!(ranked_because(2), "Ranked 2nd because");
        let ordinals: Vec<String> = [3, 4, 11, 12, 13, 21, 22, 23, 101, 111]
            .into_iter()
            .map(ordinal)
            .collect();
        assert_eq!(
            ordinals,
            ["3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "23rd", "101st", "111th"]
        );
    }

    #[test]
    fn what_waits_on_a_person_is_counted() {
        assert_eq!(waiting_on_a_person(0, "; "), "");
        assert_eq!(
            waiting_on_a_person(1, " but "),
            " but 1 mutant proposed for ignoring, waiting on a person to accept it (.mutastic/proposals.yaml)"
        );
        assert_eq!(
            waiting_on_a_person(2, "; "),
            "; 2 mutants proposed for ignoring, waiting on a person to accept them (.mutastic/proposals.yaml)"
        );
    }
}
