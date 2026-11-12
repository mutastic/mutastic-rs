//! The formats: `full` cards for people, `concise` lines for logs, `agent` text for coding agents,
//! and `jsonl`, the `mutastic.events/v1` stream for tools. The words are the Java edition's.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::ops::Range;

use serde_json::{json, Value};

use crate::engine::{MutantResult, Report, Status};
use crate::ignores;
use crate::sites::{self, Change, Crate, Mutant};
use crate::suggest::{self, SuggestedTest};
use crate::words::{self, Advice};

pub mod files;
pub mod next;
pub mod sarif;

pub const SCHEMA: &str = "mutastic.events/v1";
/// The events format's name for this edition, on every event.
pub const EDITION: &str = "rust";
pub const RULE: &str =
    "add or change tests only; never change production code to make a mutant go away: tell the person";

/// The same, said to an agent the gate holds, as the Java edition says it: production code changed
/// to pass a test the spec asks for is the task, but code changed so a mutant can't exist hides
/// what the mutant found.
pub const NO_REWRITE: &str =
    "Never change production code to make a mutant go away: tell the person.";
pub const DECLINE: &str = "if a mutant's case can't happen in practice, or the change is one no caller can see, don't contrive a test: propose ignoring it, for a person to accept, as the kind it is (equivalent, unreachable, telemetry, deprecated or accepted-risk), with the reason in two sentences: the one case where it differs, and why that can't happen";
/// The reason a proposal gives, as the Java edition's: the one case where the mutant differs, and
/// why that can't happen, for a person to check in place of a test.
pub const REASON: &str = "Differs only when <case>. That can't happen because <why>.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Full,
    Brief,
    Concise,
    Grouped,
    Agent,
    Jsonl,
}

impl Format {
    pub fn parse(name: &str) -> Option<Format> {
        match name {
            "full" => Some(Format::Full),
            "brief" => Some(Format::Brief),
            "concise" => Some(Format::Concise),
            "grouped" => Some(Format::Grouped),
            "agent" => Some(Format::Agent),
            "jsonl" => Some(Format::Jsonl),
            _ => None,
        }
    }
}

/// ANSI colour, only where it's wanted: the Java edition's colours, code for code. Every colour has
/// a glyph or a word beside it, so the output reads the same without it. With it, whether file
/// locations are links a terminal can open, and the words a person reads for a mutant's fate.
#[derive(Clone, Copy, Debug, Default)]
pub struct Style {
    pub color: bool,
    pub links: bool,
    pub words: Vocabulary,
}

/// The words people read for a mutant's fate, as the Java edition has them. `plain` (caught, missed)
/// is the default, because "your tests missed this" needs no metaphor explained; `classic` (killed,
/// survived) is the literature's. Machine formats always use the standard statuses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Vocabulary {
    #[default]
    Plain,
    Classic,
}

impl Vocabulary {
    pub fn parse(name: &str) -> Option<Vocabulary> {
        match name {
            "plain" => Some(Vocabulary::Plain),
            "classic" => Some(Vocabulary::Classic),
            _ => None,
        }
    }

    /// `caught`, or `killed`.
    pub fn detected(self) -> &'static str {
        match self {
            Vocabulary::Plain => "caught",
            Vocabulary::Classic => "killed",
        }
    }

    /// `missed`, or `survived`.
    pub fn undetected(self) -> &'static str {
        match self {
            Vocabulary::Plain => "missed",
            Vocabulary::Classic => "survived",
        }
    }

    /// The heading over the missed mutants: `Missed`, or `Survivors`.
    pub fn heading(self) -> &'static str {
        match self {
            Vocabulary::Plain => "Missed",
            Vocabulary::Classic => "Survivors",
        }
    }

    /// `with misses`, or `with survivors`, of files.
    pub fn with_misses(self) -> &'static str {
        match self {
            Vocabulary::Plain => "with misses",
            Vocabulary::Classic => "with survivors",
        }
    }

    /// `Every mutant was caught.`, or `Every mutant was killed.`
    pub fn all_detected(self) -> &'static str {
        match self {
            Vocabulary::Plain => "Every mutant was caught.",
            Vocabulary::Classic => "Every mutant was killed.",
        }
    }

    /// `not covered`, or `no coverage`.
    pub fn not_covered(self) -> &'static str {
        match self {
            Vocabulary::Plain => "not covered",
            Vocabulary::Classic => "no coverage",
        }
    }
}

/// Label backgrounds.
const RED: &str = "41";
const GREEN: &str = "42";
const CYAN: &str = "46";

impl Style {
    /// `text` linking to `file`, when links are on, as the Java edition writes them: the line
    /// number stays in the text, as whether terminals honour one in a `file:` URI varies.
    fn link(self, file: &std::path::Path, text: &str) -> String {
        let path = crate::workspace::slashed(file).replace('\\', "/");
        let path = if path.starts_with('/') {
            path
        } else {
            format!("/{path}")
        };
        self.link_url(&format!("file://{path}"), text)
    }

    /// `text` linking to `url`, when links are on.
    fn link_url(self, url: &str, text: &str) -> String {
        if self.links {
            format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
        } else {
            text.to_string()
        }
    }

    fn paint(self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }
    fn red(self, text: &str) -> String {
        self.paint("1;31", text)
    }
    fn green(self, text: &str) -> String {
        self.paint("32", text)
    }
    fn amber(self, text: &str) -> String {
        self.paint("33", text)
    }
    fn blue(self, text: &str) -> String {
        self.paint("1;34", text)
    }
    fn dim(self, text: &str) -> String {
        self.paint("2", text)
    }
    fn bold(self, text: &str) -> String {
        self.paint("1", text)
    }
    /// A label, `MISSED` on red: black bold text on a block of colour, `[MISSED]` without colour.
    fn label(self, text: &str, background: &str) -> String {
        if self.color {
            format!("\x1b[{background};30;1m {text} \x1b[0m")
        } else {
            format!("[{text}]")
        }
    }
    /// Backticks mark code in the words; at a terminal code is bold instead.
    fn code(self, text: &str) -> String {
        text.split('`')
            .enumerate()
            .map(|(index, part)| {
                if index % 2 == 1 {
                    self.bold(part)
                } else {
                    part.to_string()
                }
            })
            .collect()
    }
}

/// One entry: a line's missed mutants, written about the first.
struct Entry<'a> {
    main: &'a MutantResult,
    also: Vec<&'a MutantResult>,
}

struct View<'a> {
    report: &'a Report,
}

impl<'a> View<'a> {
    fn mutant(&self, result: &MutantResult) -> &'a Mutant {
        &self.report.found.mutants[result.mutant]
    }

    fn advice(&self, result: &MutantResult) -> Advice {
        let mutant = self.mutant(result);
        let name = &self.report.found.functions[mutant.function].name;
        let mut advice = words::advise(mutant, name, &result.inputs);
        if let Some(named) = self.named(result).filter(|named| !named.meaning.is_empty()) {
            named_notes(&mut advice.notes, &named, result);
        }
        if let Some(unpassed) = self.beyond_callers(result) {
            advice.notes.push(format!(
                "no caller of `{name}()` passed {unpassed}, which a test of it alone would need"
            ));
            advice.help = format!(
                "if a caller can pass {unpassed}, test that caller with it; if none can, the mutant \
                 can't happen: propose ignoring it as unreachable"
            );
        }
        advice
    }

    /// The arguments of the suggested call no covering test gave, `days_late = -1`, when the
    /// function is the crate's own: only its callers give it inputs, and a test of it alone on one
    /// they never pass may test what can't happen. None for a function anyone can call, or a call
    /// the callers made.
    fn beyond_callers(&self, result: &MutantResult) -> Option<String> {
        let suggestion = result.suggestion.as_ref()?;
        let function = &self.report.found.functions[self.mutant(result).function];
        if function.public {
            return None;
        }
        let arguments = &suggestion.values[suggestion
            .values
            .len()
            .checked_sub(function.numbers.len())?..];
        let unpassed: Vec<String> = function
            .numbers
            .iter()
            .zip(arguments)
            .filter(|((name, _), value)| {
                !result
                    .inputs
                    .get(name)
                    .is_some_and(|seen| seen.contains(value))
            })
            .map(|((name, _), value)| format!("`{name} = {value}`"))
            .collect();
        (!unpassed.is_empty()).then(|| unpassed.join(" and "))
    }

    /// The value the suggested call is at, when the source names it.
    fn named(&self, result: &MutantResult) -> Option<suggest::Named> {
        let suggestion = result.suggestion.as_ref()?;
        let mutant = self.mutant(result);
        let callable = self.report.found.functions[mutant.function]
            .callable
            .as_ref()?;
        suggest::named(
            callable,
            &mutant.operands,
            self.text(mutant),
            &suggestion.values,
            suggestion.built_by.as_deref(),
        )
    }

    fn text(&self, mutant: &Mutant) -> &'a str {
        self.report
            .found
            .files
            .iter()
            .find(|file| file.path == mutant.file)
            .map(|file| file.text.as_str())
            .unwrap_or_default()
    }

    fn file(&self, mutant: &Mutant) -> String {
        mutant.file.display().to_string()
    }

    fn id(&self, result: &MutantResult) -> &'a str {
        &self.mutant(result).id
    }

    /// Each of an entry's mutants in order, with its test: none when there's no suggestion, or
    /// when an earlier mutant's test is the same call, since pasting it twice wouldn't compile. On
    /// a card with others, each test's comment names the mutant it was found for.
    fn each_mutant<'e>(&self, entry: &Entry<'e>) -> Vec<(&'e MutantResult, Option<SuggestedTest>)> {
        let shared = !entry.also.is_empty();
        let mut names: Vec<String> = Vec::new();
        let mut each = Vec::new();
        for result in std::iter::once(entry.main).chain(entry.also.iter().copied()) {
            let test = self
                .suggested_test(result, shared)
                .filter(|test| !names.contains(&test.name));
            if let Some(test) = &test {
                names.push(test.name.clone());
            }
            each.push((result, test));
        }
        each
    }

    fn suggested_test(&self, result: &MutantResult, named: bool) -> Option<SuggestedTest> {
        if self.beyond_callers(result).is_some() {
            return None;
        }
        let suggestion = result.suggestion.as_ref()?;
        let mutant = self.mutant(result);
        let callable = self.report.found.functions[mutant.function]
            .callable
            .as_ref()?;
        let at = self.named(result);
        Some(suggest::test(
            callable,
            &mutant.file,
            suggestion,
            named.then_some(mutant.id.as_str()),
            at.as_ref(),
        ))
    }

    /// The missed mutants, a line's together, the line's most telling first.
    fn entries(&self) -> Vec<Entry<'a>> {
        self.entries_where(|_| true)
    }

    /// The missed mutants `kept` keeps, a line's together, the line's most telling first.
    fn entries_where(&self, kept: impl Fn(&MutantResult) -> bool) -> Vec<Entry<'a>> {
        let mut by_line: BTreeMap<(String, usize), Vec<&'a MutantResult>> = BTreeMap::new();
        for result in &self.report.results {
            if result.status == Status::Survived && kept(result) {
                let mutant = self.mutant(result);
                by_line
                    .entry((self.file(mutant), mutant.line))
                    .or_default()
                    .push(result);
            }
        }
        by_line
            .into_values()
            .map(|mut results| {
                // The lead, whose card it is: a boundary, whose edge is the most exact thing to
                // test, then the order found, as the Java edition's `Finding` leads.
                results.sort_by_key(|r| (self.mutant(r).operator != sites::BOUNDARY, r.mutant));
                let main = results.remove(0);
                Entry {
                    main,
                    also: results,
                }
            })
            .collect()
    }

    /// The lines of the statement the mutant changes, as written and as the mutant has them (none
    /// when it takes the statement out), without the indent they share and cut short around the
    /// change: what every card shows in its `-` and `+` lines.
    fn changed_lines(&self, mutant: &Mutant) -> Shown {
        let text = self.text(mutant);
        let (range, with) = mutant.claim();
        let extent = statement_extent(text, &range);
        let starts = sites::line_starts(text);
        let first = starts[sites::line_of(&starts, extent.start.min(range.start)) - 1];
        let end = extent.end.max(range.end);
        let last = text[end..].find('\n').map_or(text.len(), |at| end + at);
        let before: Vec<&str> = text[first..last].lines().collect();
        let mut changed = format!(
            "{}{with}{}",
            &text[first..range.start],
            &text[range.end..last]
        );
        if matches!(mutant.change, Change::Always { .. }) {
            changed = simplified(&changed);
        }
        let after: Vec<&str> = changed
            .lines()
            .enumerate()
            .filter(|(at, line)| {
                !line.trim().is_empty() || before.get(*at).is_some_and(|was| was.trim().is_empty())
            })
            .map(|(_, line)| line)
            .collect();
        Shown::new(&before, &after)
    }

    fn command(&self, verb: &str, ids: &str) -> String {
        mutant_command(verb, ids, &self.report.project.given)
    }

    /// Where an entry stands in the order a run ranks its misses.
    fn standing(&self, entry: &Entry) -> Standing {
        let mutant = self.mutant(entry.main);
        Standing {
            prior: std::iter::once(entry.main)
                .chain(entry.also.iter().copied())
                .map(|r| prior(self.mutant(r)))
                .max()
                .unwrap_or(1),
            covering: entry.main.covered_by.len(),
            file: self.file(mutant),
            line: mutant.line,
        }
    }
}

/// The commands offered with each missed mutant, by the verb [`mutant_command`] takes: what proves
/// a fix, what explains it, what ignores it or proposes ignoring it, and what runs it again.
pub const MUTANT_COMMANDS: &[&str] = &["verify", "explain", "ignore", "propose", "run"];

/// The command for `ids` (split by commas) in the package at `dir`, by one of
/// [`MUTANT_COMMANDS`]: `mutastic verify mb2f8ae57 fixtures/pricing`. Every command the output
/// offers for a mutant is written here, so a test can check each one parses.
pub fn mutant_command(verb: &str, ids: &str, dir: &str) -> String {
    match verb {
        "verify" => format!("mutastic verify {ids} {dir}"),
        "explain" => format!("mutastic explain {ids} {dir}"),
        "ignore" | "propose" => {
            let how = if verb == "propose" {
                format!(" --propose --as=<equivalent|unreachable|…> --reason=\"{REASON}\"")
            } else {
                " --reason=\"<why>\"".to_string()
            };
            match ids.split_once(',') {
                // One decision each: a card with several mutants names them all.
                Some(_) => format!(
                    "mutastic ignore <id>{how} {dir} for each of {}",
                    ids.replace(',', ", ")
                ),
                None => format!("mutastic ignore {ids}{how} {dir}"),
            }
        }
        _ => format!("mutastic run --only={ids} {dir}"),
    }
}

/// `mutastic next fixtures/pricing`: what `next` offers to run again, for the next item.
pub fn next_command(dir: &str) -> String {
    format!("mutastic next {dir}")
}

/// Where an entry stands in the order a run ranks its misses, as the Java edition's `Finding.ranked`
/// orders them and `mutastic next` queues them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Standing {
    /// 2 for a condition or a removal, 1 for anything else: the best of the line's mutants.
    pub prior: u8,
    /// How many tests run the line: many run it and none checks it.
    pub covering: usize,
    /// Where it is in the code: its file and line.
    pub file: String,
    pub line: usize,
}

impl Standing {
    /// The order: a condition or a removal first, then the line more tests run, then where it is
    /// in the code. The Java edition puts the misses a baseline doesn't have before all of these:
    /// that step goes first here once the Rust edition has `mutastic baseline`.
    pub fn order(&self, other: &Standing) -> std::cmp::Ordering {
        other
            .prior
            .cmp(&self.prior)
            .then(other.covering.cmp(&self.covering))
            .then_with(|| (&self.file, self.line).cmp(&(&other.file, other.line)))
    }
}

/// What the name a card's call is at is, beside the inputs seen, as the Java edition says it:
/// `inputs seen: total = 20, 80, with free_from = 50 (Delivery's field, from Delivery::new(50))`
/// and `none has total == free_from` when the receiver is made as a covering test makes it, or
/// `the edge is FREE_FROM (FREE_FROM = 100)`.
fn named_notes(notes: &mut Vec<String>, named: &suggest::Named, result: &MutantResult) {
    let built = result
        .suggestion
        .as_ref()
        .is_some_and(|found| found.built_by.is_some());
    let seen = notes
        .iter()
        .position(|note| note.starts_with("inputs seen: "));
    match (built, seen) {
        (true, Some(at)) => {
            notes[at] = format!("{}, with {}", notes[at], named.meaning);
            let parameter = named.note.split(" == ").next().unwrap_or_default();
            let value = named
                .meaning
                .split(" = ")
                .nth(1)
                .and_then(|rest| rest.split(' ').next())
                .and_then(|value| value.parse::<i128>().ok());
            let inputs = result.inputs.get(parameter);
            if let (Some(value), Some(inputs)) = (value, inputs) {
                if !inputs.is_empty() && !inputs.contains(&value) {
                    notes.push(format!("none has {}", named.note));
                }
            }
        }
        _ => notes.push(format!("the edge is {}", named.meaning)),
    }
}

/// 2 for a condition or a removal, 1 for anything else, as the Java edition's `Finding.prior`: a
/// condition or a removal that goes unnoticed changes what the code decides or does, while
/// arithmetic and the rest are more often noise. Rust's own operators go with their Java family:
/// remove-struct-field with the removals; bitwise with arithmetic; reverse-ordering with the
/// library swaps; unwrap-try with remove-throw; default-parameter with null-parameter; swap-break
/// with remove-switch-arm.
fn prior(mutant: &Mutant) -> u8 {
    match mutant.operator {
        sites::BOUNDARY
        | sites::COMPARISON
        | sites::OTHER_COMPARISON
        | sites::REMOVE_CONDITIONAL
        | sites::LOGICAL_CONNECTOR
        | sites::RETURN_VALUE
        | sites::REMOVE_VOID_CALL
        | sites::REMOVE_DISCARDED_CALL
        | sites::REMOVE_ASSIGNMENT
        | sites::REMOVE_FIELD_WRITE
        | sites::REMOVE_STRUCT_FIELD => 2,
        _ => 1,
    }
}

/// `entries`, those most worth a test first, as the Java edition's `Finding.ranked`: by the best
/// prior among an entry's mutants, then the one more tests reach (many tests run it and none checks
/// it), then where it is in the code ([`Standing::order`]).
fn ranked<'a>(view: &View<'a>, entries: Vec<Entry<'a>>) -> Vec<Entry<'a>> {
    let mut standings: Vec<(Standing, Entry<'a>)> = entries
        .into_iter()
        .map(|entry| (view.standing(&entry), entry))
        .collect();
    standings.sort_by(|(a, _), (b, _)| a.order(b));
    standings.into_iter().map(|(_, entry)| entry).collect()
}

fn covered_by(result: &MutantResult) -> String {
    let count = result.covered_by.len();
    if count == 1 {
        format!(
            "covered by {} (its 1 covering test still passes)",
            result.covered_by[0]
        )
    } else {
        format!(
            "covered by {} (all {count} covering tests still pass)",
            result.covered_by.join(", ")
        )
    }
}

fn percent(score: Option<f64>) -> String {
    score.map_or("none".into(), |s| format!("{s:.1}%"))
}

/// `340ms`, `2.1s`, `1m39s`, as the Java edition writes them.
pub fn duration(took: std::time::Duration) -> String {
    let millis = took.as_millis();
    if millis < 1000 {
        format!("{millis}ms")
    } else if millis < 60_000 {
        format!("{:.1}s", millis as f64 / 1000.0)
    } else {
        format!("{}m{:02}s", millis / 60_000, (millis / 1000) % 60)
    }
}

/// The whole result in `format`.
pub fn render(report: &Report, format: Format, style: Style) -> String {
    match format {
        Format::Full | Format::Brief | Format::Concise | Format::Grouped => {
            human(report, style, format)
        }
        Format::Agent => agent(report, style),
        Format::Jsonl => jsonl(report),
    }
}

/// `discount()`: a function as the cards name it.
fn method(found: &Crate, mutant: &Mutant) -> String {
    format!("{}()", found.functions[mutant.function].name)
}

/// One file's line, as its last mutant finishes: `✓` and its count when nothing was missed, `❯`
/// with each miss under it and each line no test reaches otherwise.
pub fn file_finished(
    project: &str,
    found: &Crate,
    results: &[MutantResult],
    style: Style,
) -> String {
    let Some(first) = results.first() else {
        return String::new();
    };
    let missed: Vec<&MutantResult> = results
        .iter()
        .filter(|r| r.status == Status::Survived)
        .collect();
    let uncovered: Vec<&MutantResult> = results
        .iter()
        .filter(|r| r.status == Status::NoCoverage)
        .collect();
    let mut counts = vec![style.dim(&format!(
        "{} mutant{}",
        results.len(),
        if results.len() == 1 { "" } else { "s" }
    ))];
    if !missed.is_empty() {
        counts.push(style.red(&format!("{} {}", missed.len(), style.words.undetected())));
    }
    if !uncovered.is_empty() {
        counts.push(style.amber(&format!(
            "{} {}",
            uncovered.len(),
            style.words.not_covered()
        )));
    }
    let took: std::time::Duration = results.iter().map(|r| r.took).sum();
    let mut out = format!(
        " {} {} {} {}{}{} {}\n",
        if missed.is_empty() && uncovered.is_empty() {
            style.green("✓")
        } else {
            style.red("❯")
        },
        style.label(project, CYAN),
        found.mutants[first.mutant].file.display(),
        style.dim("("),
        counts.join(&style.dim(" | ")),
        style.dim(")"),
        style.dim(&duration(took))
    );
    for result in missed {
        let mutant = &found.mutants[result.mutant];
        let name = &found.functions[mutant.function].name;
        let _ = writeln!(
            out,
            "   {} {} {} {}",
            style.red("×"),
            method(found, mutant),
            style.dim(&format!(":{}", mutant.line)),
            style.code(&words::advise(mutant, name, &result.inputs).change)
        );
    }
    let mut by_line: Vec<((String, usize), usize)> = Vec::new();
    for result in uncovered {
        let mutant = &found.mutants[result.mutant];
        let place = (method(found, mutant), mutant.line);
        match by_line.iter_mut().find(|(seen, _)| *seen == place) {
            Some((_, count)) => *count += 1,
            None => by_line.push((place, 1)),
        }
    }
    for ((name, line), count) in by_line {
        let _ = writeln!(
            out,
            "   {} {name} {} {}",
            style.amber("○"),
            style.dim(&format!(":{line}")),
            style.dim(&format!(
                "no test reaches this line{}",
                if count > 1 {
                    format!(" ({count} mutants)")
                } else {
                    String::new()
                }
            ))
        );
    }
    out
}

fn human(report: &Report, style: Style, format: Format) -> String {
    let view = View { report };
    let mut out = String::new();
    let entries = view.entries();
    if !entries.is_empty() {
        let _ = writeln!(
            out,
            "\n{}{}",
            style.red(&style.bold(&format!(" {} ", style.words.heading()))),
            style.dim(&"─".repeat(60))
        );
        if format == Format::Grouped {
            grouped(&mut out, &view, &entries, style);
        } else if matches!(format, Format::Full | Format::Brief) {
            let mut by_file: BTreeMap<String, Vec<&Entry>> = BTreeMap::new();
            for entry in &entries {
                by_file
                    .entry(view.file(view.mutant(entry.main)))
                    .or_default()
                    .push(entry);
            }
            for (file, entries) in by_file {
                let missed: usize = entries.iter().map(|e| 1 + e.also.len()).sum();
                let functions: std::collections::BTreeSet<usize> = entries
                    .iter()
                    .map(|e| view.mutant(e.main).function)
                    .collect();
                let _ = writeln!(
                    out,
                    "\n {} {}  {}",
                    style.blue("❯"),
                    style.bold(&style.link(&view.report.project.workspace.join(&file), &file)),
                    style.dim(&format!(
                        "{missed} {} in {} function{}",
                        style.words.undetected(),
                        functions.len(),
                        if functions.len() == 1 { "" } else { "s" }
                    ))
                );
                for entry in entries {
                    if format == Format::Brief {
                        brief_card(&mut out, &view, entry, style);
                    } else {
                        card(&mut out, &view, entry, style);
                    }
                }
            }
        } else {
            for entry in &entries {
                let mutant = view.mutant(entry.main);
                let _ = writeln!(
                    out,
                    " {} {} {} {}",
                    style.red("✗"),
                    style.link(
                        &view.report.project.workspace.join(&mutant.file),
                        &format!("{}:{}", view.file(mutant), mutant.line)
                    ),
                    style.code(&view.advice(entry.main).headline),
                    style.dim(&id_list(&view, entry))
                );
            }
        }
    }
    not_covered(&mut out, &view, style);
    warnings(&mut out, report, style);
    summary(&mut out, report, style);
    out
}

/// The run's warnings, before the summary: each flaky test left out, as the Java edition shows them.
fn warnings(out: &mut String, report: &Report, style: Style) {
    let likely_flaky = likely_flaky(report);
    if report.flaky.is_empty() && likely_flaky.is_empty() {
        return;
    }
    out.push('\n');
    for flaky in &report.flaky {
        let _ = writeln!(
            out,
            " {} {} failed, then passed when run again; left out of this run{}",
            style.amber("! flaky:"),
            flaky.test,
            flaky_reason(flaky)
        );
    }
    for (test, mutants) in &likely_flaky {
        let _ = writeln!(
            out,
            " {} {test}; with the code unchanged, whether it catches {} changed since the last run",
            style.amber("! likely flaky:"),
            mutants.join(", ")
        );
    }
}

/// Each test whose catches changed since the last run with the code and mutastic unchanged, and
/// the mutants it changed.
fn likely_flaky(report: &Report) -> std::collections::BTreeMap<String, Vec<String>> {
    report
        .comparison
        .as_ref()
        .map(crate::record::Comparison::likely_flaky)
        .unwrap_or_default()
}

/// `; it reaches SystemTime::now`: what it reaches that changes from run to run, its own code first.
fn flaky_reason(flaky: &crate::flaky::Flaky) -> String {
    if !flaky.reaches_in_test.is_empty() {
        format!(
            "; its own code reaches {}",
            flaky.reaches_in_test.join(", ")
        )
    } else if !flaky.reaches.is_empty() {
        format!("; it reaches {}", flaky.reaches.join(", "))
    } else {
        String::new()
    }
}

/// What to do about a flaky test, by what mutastic saw, in the Java edition's words: its own code
/// varies, the code it runs varies, or nothing is known. A test's failure in Rust is a panic,
/// never an exception from the network or a file, so the Java edition's advice for those has no
/// counterpart.
fn flaky_advice(flaky: &crate::flaky::Flaky) -> String {
    if !flaky.reaches_in_test.is_empty() {
        format!(
            "Its own code reaches {}, which answers differently every run, so it makes its own flakiness: making that the same every time is likely safe, and if it stands in for something real (a slow service), keep that checked by a test of its own.",
            flaky.reaches_in_test.join(", ")
        )
    } else if !flaky.reaches.is_empty() {
        format!(
            "The code it runs reaches {}, which answers differently every run: making that the same every time (a seed, a fixed clock, through the code's own seam) is likely safe, so long as the test still checks what it did.",
            flaky.reaches.join(", ")
        )
    } else {
        "mutastic can't tell why: leave it unless the cause is plain in the code, and say in your final message that it's flaky.".to_string()
    }
}

/// `, or only a flaky one does`, after "no test reaches these lines", when a flaky test was left out.
fn or_only_flaky(report: &Report) -> &'static str {
    if report.flaky.is_empty() {
        ""
    } else {
        ", or only a flaky one does"
    }
}

/// `┌─ src/lib.rs:26`, then the changed line between the lines either side, the change marked under
/// it; gives the gutter the rest of the card is indented by. With `docs`, the mark links to its
/// operator's section in the docs when links are on, as the Java edition's full card does.
fn frame(
    out: &mut String,
    view: &View,
    mutant: &Mutant,
    advice: &Advice,
    docs: Option<&str>,
    style: Style,
) -> String {
    let text = view.text(mutant);
    let lines: Vec<&str> = text.lines().collect();
    let width = (mutant.line + 1).to_string().len();
    let g = " ".repeat(width + 2);
    let bar = style.dim("│");
    let _ = writeln!(
        out,
        "{g}{} {}",
        style.dim("┌─"),
        style.link(
            &view.report.project.workspace.join(&mutant.file),
            &format!("{}:{}", view.file(mutant), mutant.line)
        )
    );
    let _ = writeln!(out, "{g}{bar}");
    for number in mutant.line.saturating_sub(1).max(1)..=(mutant.line + 1).min(lines.len()) {
        let _ = writeln!(
            out,
            " {} {bar} {}",
            style.dim(&format!("{number:>width$}")),
            lines[number - 1]
        );
        if number == mutant.line {
            let caret_width = text[mutant.caret.clone()]
                .lines()
                .next()
                .unwrap_or_default()
                .chars()
                .count()
                .max(1);
            let marked = style.red(&format!("{} {}", "^".repeat(caret_width), advice.caret));
            let _ = writeln!(
                out,
                "{g}{bar} {}{}",
                " ".repeat(mutant.column - 1),
                docs.map_or_else(|| marked.clone(), |url| style.link_url(url, &marked))
            );
        }
    }
    let _ = writeln!(out, "{g}{bar}");
    g
}

/// `= mutant  - the statement` and `+ the statement as the mutant has it`; no `+` for a statement
/// the mutant takes out.
fn before_and_after(out: &mut String, g: &str, shown: &Shown, style: Style) {
    for (at, line) in shown.before.iter().enumerate() {
        let lead = if at == 0 {
            format!("{} mutant  ", style.dim("="))
        } else {
            "          ".to_string()
        };
        let _ = writeln!(out, "{g}{lead}{}{line}", style.red("- "));
    }
    for line in &shown.after {
        let _ = writeln!(out, "{g}          {}{line}", style.green("+ "));
    }
}

/// What a card shows of a mutant's change: its statement's lines as written, and as the mutant
/// has them, none when it takes the statement out.
struct Shown {
    before: Vec<String>,
    after: Vec<String>,
}

impl Shown {
    /// The most of a statement a card shows; a longer one is cut short, around its change.
    const MOST_LINES: usize = 6;

    fn new(before: &[&str], after: &[&str]) -> Shown {
        let indent = before
            .iter()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.len() - line.trim_start().len())
            .min()
            .unwrap_or(0);
        let changed = before
            .iter()
            .zip(after)
            .take_while(|(was, now)| was == now)
            .count();
        Shown {
            before: cut(before, changed.min(before.len().saturating_sub(1)), indent),
            after: cut(after, changed.min(after.len().saturating_sub(1)), indent),
        }
    }
}

/// `lines` without `indent`, and only [`Shown::MOST_LINES`] of them, from just before `changed`,
/// with `…` for what's left out.
fn cut(lines: &[&str], changed: usize, indent: usize) -> Vec<String> {
    let from = changed
        .saturating_sub(1)
        .min(lines.len().saturating_sub(Shown::MOST_LINES));
    let to = lines.len().min(from + Shown::MOST_LINES);
    let mut shown = Vec::new();
    if from > 0 {
        shown.push("…".to_string());
    }
    for line in &lines[from..to] {
        let leading = line.len() - line.trim_start().len();
        shown.push(line[leading.min(indent)..].trim_end().to_string());
    }
    if to < lines.len() {
        shown.push("…".to_string());
    }
    shown
}

/// Where the statement a change is in starts and ends, from the syntax tree: the nearest
/// statement, match arm or block's last expression around it, or an `if` or `while` up to the
/// end of its condition, for a change in the condition.
fn statement_extent(text: &str, range: &Range<usize>) -> Range<usize> {
    use ra_ap_syntax::ast::{self, AstNode};
    use ra_ap_syntax::{Edition, NodeOrToken, SourceFile, SyntaxKind, TextRange, TextSize};
    let to_size = |at: usize| TextSize::from(u32::try_from(at).unwrap_or(u32::MAX));
    let file = SourceFile::parse(text, Edition::CURRENT).tree();
    let target = TextRange::new(to_size(range.start), to_size(range.end.max(range.start)));
    if file.syntax().text_range().end() < target.end() {
        return range.clone();
    }
    let node = match file.syntax().covering_element(target) {
        NodeOrToken::Node(node) => node,
        NodeOrToken::Token(token) => match token.parent() {
            Some(parent) => parent,
            None => return range.clone(),
        },
    };
    let span = |from: TextSize, to: TextSize| usize::from(from)..usize::from(to);
    for ancestor in node.ancestors() {
        let Some(parent) = ancestor.parent() else {
            break;
        };
        let condition = ast::IfExpr::cast(parent.clone())
            .and_then(|found| found.condition())
            .or_else(|| ast::WhileExpr::cast(parent.clone()).and_then(|found| found.condition()));
        if condition.is_some_and(|found| found.syntax() == &ancestor) {
            return span(parent.text_range().start(), ancestor.text_range().end());
        }
        if ast::Stmt::can_cast(ancestor.kind())
            || ancestor.kind() == SyntaxKind::MATCH_ARM
            || parent.kind() == SyntaxKind::STMT_LIST
        {
            return span(ancestor.text_range().start(), ancestor.text_range().end());
        }
    }
    range.clone()
}

/// `!true` as `false` and `!false` as `true`, the rest as written: a condition forced to a value
/// reads as that value.
fn simplified(text: &str) -> String {
    let mut out = text.to_string();
    for (negated, value) in [
        ("!(true)", "false"),
        ("!(false)", "true"),
        ("!true", "false"),
        ("!false", "true"),
    ] {
        out = replace_words(&out, negated, value);
    }
    out
}

/// `text` with each `found` replaced where it isn't run on into a name: `!true` but not `!trueish`.
fn replace_words(text: &str, found: &str, with: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find(found) {
        let after = &rest[at + found.len()..];
        let runs_on = after
            .chars()
            .next()
            .is_some_and(|next| next.is_alphanumeric() || next == '_');
        out.push_str(&rest[..at]);
        out.push_str(if runs_on { found } else { with });
        rest = after;
    }
    out.push_str(rest);
    out
}

fn id_list(view: &View, entry: &Entry) -> String {
    std::iter::once(entry.main)
        .chain(entry.also.iter().copied())
        .map(|r| view.id(r).to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// The missed mutants of each file, by function, a line each: where, what, and the id, as the Java
/// edition's `grouped` shows them.
fn grouped(out: &mut String, view: &View, entries: &[Entry], style: Style) {
    let mut by_file: BTreeMap<String, BTreeMap<usize, Vec<&MutantResult>>> = BTreeMap::new();
    for entry in entries {
        for result in std::iter::once(entry.main).chain(entry.also.iter().copied()) {
            let mutant = view.mutant(result);
            by_file
                .entry(view.file(mutant))
                .or_default()
                .entry(mutant.function)
                .or_default()
                .push(result);
        }
    }
    for (file, functions) in by_file {
        let linked = style.link(&view.report.project.workspace.join(&file), &file);
        let _ = writeln!(out, "\n {}", style.bold(&linked));
        for (function, mut results) in functions {
            results.sort_by_key(|result| {
                let mutant = view.mutant(result);
                (mutant.line, mutant.column, mutant.ordinal)
            });
            let name = &view.report.found.functions[function].name;
            let _ = writeln!(
                out,
                "   {}() {}",
                name,
                style.red(&format!("{} {}", results.len(), style.words.undetected()))
            );
            for result in results {
                let mutant = view.mutant(result);
                let _ = writeln!(
                    out,
                    "     {}   {} {} {}",
                    style.dim(&format!("{}:{}", mutant.line, mutant.column)),
                    style.red("✗"),
                    style.code(&view.advice(result).headline),
                    style.dim(&mutant.id)
                );
            }
        }
    }
}

/// A card's head: where the mutant is and what's seen there, and how to see the whole card, as
/// the Java edition's `brief` shows it.
fn brief_card(out: &mut String, view: &View, entry: &Entry, style: Style) {
    let result = entry.main;
    let mutant = view.mutant(result);
    let advice = view.advice(result);
    let _ = writeln!(
        out,
        "\n {} {}",
        style.label(
            &format!("✗ {}", style.words.undetected().to_uppercase()),
            RED
        ),
        style.code(&advice.headline)
    );
    let g = frame(out, view, mutant, &advice, None, style);
    let equals = style.dim("=");
    for (index, note) in advice.notes.iter().enumerate() {
        if index == 0 {
            let _ = writeln!(
                out,
                "{g}{equals} {}: {}",
                style.blue(&style.bold("note")),
                style.code(note)
            );
        } else {
            let _ = writeln!(out, "{g}        {}", style.dim(&style.code(note)));
        }
    }
    let ids = id_list(view, entry);
    let _ = writeln!(
        out,
        "{g}{equals} id: {ids} · {} {}",
        view.command("run", &ids),
        style.dim("--format=full for the whole card")
    );
}

fn card(out: &mut String, view: &View, entry: &Entry, style: Style) {
    let result = entry.main;
    let mutant = view.mutant(result);
    let advice = view.advice(result);
    let shared = !entry.also.is_empty();
    let _ = writeln!(
        out,
        "\n {} {}{}",
        style.label(
            &format!("✗ {}", style.words.undetected().to_uppercase()),
            RED
        ),
        style.code(&advice.headline),
        if shared {
            style.dim(&format!(" · {}", mutant.id))
        } else {
            String::new()
        }
    );
    let docs = crate::operator_docs::url(mutant.operator);
    let g = frame(out, view, mutant, &advice, docs.as_deref(), style);
    let equals = style.dim("=");
    before_and_after(out, &g, &view.changed_lines(mutant), style);
    for other in &entry.also {
        let other_mutant = view.mutant(other);
        let _ = writeln!(
            out,
            "{g}{equals} also    {} {} {}",
            style.red("✗"),
            style.code(&view.advice(other).headline),
            style.dim(&format!("· {}", other_mutant.id))
        );
        let shown = view.changed_lines(other_mutant);
        if shown.after.is_empty() {
            let _ = writeln!(
                out,
                "{g}{equals} change  {}",
                style.code(&view.advice(other).change)
            );
        }
        for line in &shown.after {
            let _ = writeln!(out, "{g}          {}{line}", style.green("+ "));
        }
    }
    let mut notes: Vec<String> = advice.notes.clone();
    notes.push(covered_by(result));
    for (index, note) in notes.iter().enumerate() {
        if index == 0 {
            let _ = writeln!(
                out,
                "{g}{equals} {}: {}",
                style.blue(&style.bold("note")),
                style.code(note)
            );
        } else {
            let _ = writeln!(out, "{g}        {}", style.dim(&style.code(note)));
        }
    }
    help_and_tests(out, view, entry, &advice.help, &g, style);
    let ids = id_list(view, entry);
    let _ = writeln!(
        out,
        "\n{g}{} {} {}",
        style.dim("done when "),
        view.command("verify", &ids),
        style.dim("passes")
    );
    let _ = writeln!(
        out,
        "{g}{} {}",
        style.dim("reproduce "),
        view.command("run", &ids)
    );
    let _ = writeln!(
        out,
        "{g}{} {}",
        style.dim("or ignore "),
        view.command("ignore", &ids)
    );
}

/// `= help: the fix`, then each test to add. When the card has a test for more than one of its
/// mutants, each test comes after its own mutant's fix and id, as the Java edition gives them: the
/// lead's fix describes only the lead's test.
fn help_and_tests(out: &mut String, view: &View, entry: &Entry, help: &str, g: &str, style: Style) {
    let each = view.each_mutant(entry);
    let own = each.iter().filter(|(_, test)| test.is_some()).count() > 1;
    let shown = if own {
        each_help(&each).to_string()
    } else {
        style.code(help)
    };
    let _ = writeln!(
        out,
        "{g}{} {}: {shown}",
        style.dim("="),
        style.green(&style.bold("help"))
    );
    let mut any = false;
    for (result, test) in &each {
        if own {
            let fix = style.code(&view.advice(result).help);
            let id = style.dim(&format!(" · {}", view.id(result)));
            let _ = writeln!(out, "\n{g}        {fix}{id}");
        }
        if let Some(test) = test {
            if !own {
                let _ = writeln!(out);
            }
            any = true;
            let _ = writeln!(out, "{g}        // in {}", test.place);
            for line in &test.lines {
                let _ = writeln!(out, "{g}        {line}");
            }
        }
    }
    if any {
        let _ = writeln!(out, "{g}        {}", style.dim(CHECK_TODAY));
    }
}

/// What `help:` says when each test has its own fix: one test each, if every mutant has one.
fn each_help(each: &[(&MutantResult, Option<SuggestedTest>)]) -> &'static str {
    if each.iter().all(|(_, test)| test.is_some()) {
        "one test for each mutant"
    } else {
        "for each mutant"
    }
}

/// Under a suggested test, as the Java edition says it: the value it asserts is what the code
/// does, which may not be what it should.
const CHECK_TODAY: &str =
    "check today's value is the one you intend: the test pins what the code does now";

/// How many functions the Not covered section lists, as the Java edition caps it.
const NOT_COVERED_SHOWN: usize = 50;

/// `Not covered (no test reaches these lines)`, then each function with mutants no test reaches:
/// `○ src/lib.rs:18-20 tier() (4 mutants)`, in the order found, as the Java edition shows them.
/// The functions with mutants no test reaches, in the order found: where (`src/lib.rs:18-20
/// tier()`) and how many mutants, as the Java edition counts them.
/// Each function with mutants no test reaches, `src/lib.rs:18-20 tier()`, with each of those
/// mutants' line and id, in the order found.
fn not_covered_places(view: &View) -> Vec<(String, Vec<(usize, String)>)> {
    let mut by_function: Vec<(usize, Vec<(usize, String)>)> = Vec::new();
    for result in &view.report.results {
        if result.status == Status::NoCoverage {
            let mutant = view.mutant(result);
            let found = (mutant.line, mutant.id.clone());
            match by_function.iter_mut().find(|(f, _)| *f == mutant.function) {
                Some((_, mutants)) => mutants.push(found),
                None => by_function.push((mutant.function, vec![found])),
            }
        }
    }
    by_function
        .into_iter()
        .map(|(function, mutants)| {
            let (from, to) = (
                mutants.iter().map(|(line, _)| *line).min().unwrap_or(0),
                mutants.iter().map(|(line, _)| *line).max().unwrap_or(0),
            );
            let f = &view.report.found.functions[function];
            let place = if from == to {
                from.to_string()
            } else {
                format!("{from}-{to}")
            };
            (
                format!("{}:{place} {}()", f.file.display(), f.name),
                mutants,
            )
        })
        .collect()
}

/// `Not covered (no test reaches these lines)`, then each function with mutants no test reaches:
/// `○ src/lib.rs:18-20 tier() (4 mutants)`, in the order found, as the Java edition shows them.
fn not_covered(out: &mut String, view: &View, style: Style) {
    let places = not_covered_places(view);
    if places.is_empty() {
        return;
    }
    let _ = writeln!(
        out,
        "\n{}{}",
        style.amber(&style.bold(" Not covered ")),
        style.dim(&format!(
            "(no test reaches these lines{})",
            or_only_flaky(view.report)
        ))
    );
    for (place, mutants) in places.iter().take(NOT_COVERED_SHOWN) {
        let count = mutants.len();
        let _ = writeln!(
            out,
            "   {} {place} {}",
            style.amber("○"),
            style.dim(&format!(
                "({count} mutant{})",
                if count == 1 { "" } else { "s" }
            ))
        );
    }
    if places.len() > NOT_COVERED_SHOWN {
        let _ = writeln!(
            out,
            "{}",
            style.dim(&format!(
                "   … and {} more functions",
                places.len() - NOT_COVERED_SHOWN
            ))
        );
    }
}

fn summary(out: &mut String, report: &Report, style: Style) {
    let rows = rows(report, style);
    let width = rows.iter().map(|(label, _)| label.len()).max().unwrap_or(0);
    out.push('\n');
    for (label, value) in rows {
        let _ = writeln!(out, " {}  {value}", style.dim(&format!("{label:>width$}")));
    }
    let _ = writeln!(out, "\n {}", verdict(report, style));
    if report.count(Status::Survived) > 0 {
        let dir = &report.project.given;
        let _ = writeln!(
            out,
            "{}",
            style.dim(&format!(
                " Next: mutastic verify <id> {dir} · mutastic run --only=<id> {dir} · mutastic explain <id> {dir}"
            ))
        );
    }
}

/// The summary's rows, label and value, in the order shown.
fn rows(report: &Report, style: Style) -> Vec<(&'static str, String)> {
    let mut rows = vec![
        ("Files", files_row(report, style)),
        ("Mutants", mutants_row(report, style)),
    ];
    for (index, line) in not_mutated(report).into_iter().enumerate() {
        rows.push((if index == 0 { "Left out" } else { "" }, style.dim(&line)));
    }
    if let Some(scope) = &report.scope {
        rows.push((
            "Changed",
            format!(
                "{} line{} in {} file{} since {}",
                scope.lines,
                if scope.lines == 1 { "" } else { "s" },
                scope.files,
                if scope.files == 1 { "" } else { "s" },
                scope.since
            ),
        ));
    }
    if let Some(worst) = worst_at(report) {
        rows.push(("Worst at", worst));
    }
    if !report.flaky.is_empty() {
        let count = report.flaky.len();
        rows.push((
            "Flaky",
            format!("{count} test{} left out", if count == 1 { "" } else { "s" }),
        ));
    }
    let by_crash = report
        .results
        .iter()
        .filter(|r| {
            r.killed_with
                .as_deref()
                .is_some_and(|w| w.starts_with("crash"))
        })
        .count();
    if by_crash > 0 {
        rows.push((
            "Crashes",
            style.dim(&format!(
                "{by_crash} caught only because the test process crashed, not by a test's check"
            )),
        ));
    }
    let rounds = report.rounds;
    rows.push((
        "Build",
        format!(
            "one build of every mutant, {rounds} round{} (check and build)",
            if rounds == 1 { "" } else { "s" }
        ),
    ));
    rows.push(("Test runs", test_runs_row(report, style)));
    if let Some(started) = &report.started_at {
        rows.push(("Start at", started.clone()));
    }
    rows.push((
        "Duration",
        format!(
            "{} {}",
            duration(report.took),
            style.dim(&format!(
                "(setup {}, coverage {}, mutants {})",
                duration(report.setup),
                duration(report.coverage),
                duration(report.mutants)
            ))
        ),
    ));
    rows.push((
        "Tests",
        format!(
            "{} on rustc {} · {} worker{}",
            report.tests,
            report.rustc,
            report.workers,
            if report.workers == 1 { "" } else { "s" }
        ),
    ));
    if !report.reports.is_empty() {
        rows.push(("Reports", report.reports.join(&style.dim(" · "))));
    }
    rows.push(("Score", score_row(report, style)));
    rows
}

/// `0 clean | 1 with misses (1)`: the source files with mutants, by whether any was missed or not
/// covered.
fn files_row(report: &Report, style: Style) -> String {
    let mut by_file: BTreeMap<&std::path::Path, bool> = BTreeMap::new();
    for result in &report.results {
        let with_misses = matches!(result.status, Status::Survived | Status::NoCoverage);
        *by_file
            .entry(report.found.mutants[result.mutant].file.as_path())
            .or_default() |= with_misses;
    }
    let with_misses = by_file.values().filter(|&&misses| misses).count();
    let mut parts =
        vec![style.green(&style.bold(&format!("{} clean", by_file.len() - with_misses)))];
    if with_misses > 0 {
        parts.push(style.red(&style.bold(&format!("{with_misses} {}", style.words.with_misses()))));
    }
    format!(
        "{}{}",
        parts.join(&style.dim(" | ")),
        style.dim(&format!(" ({})", by_file.len()))
    )
}

fn mutants_row(report: &Report, style: Style) -> String {
    let caught = report.count(Status::Killed);
    let missed = report.count(Status::Survived);
    let timed_out = report.count(Status::Timeout);
    let not_covered = report.count(Status::NoCoverage);
    let skipped = report.count(Status::Ignored);
    let crashed = report.count(Status::RuntimeError);
    let words = style.words;
    let mut counts = vec![style.green(&format!(
        "✓ {}",
        style.bold(&format!("{caught} {}", words.detected()))
    ))];
    if missed > 0 {
        counts.push(style.red(&format!(
            "✗ {}",
            style.bold(&format!("{missed} {}", words.undetected()))
        )));
    }
    if timed_out > 0 {
        counts.push(style.amber(&format!("⏱ {timed_out} timed out")));
    }
    if not_covered > 0 {
        counts.push(style.amber(&format!("○ {not_covered} {}", words.not_covered())));
    }
    let scored = report.results.len() - report.unscored();
    let mut row = format!(
        "{}{}",
        counts.join(&style.dim(" · ")),
        style.dim(&format!(" ({scored})"))
    );
    if skipped > 0 {
        row.push_str(&style.dim(&format!(" · {}", skipped_by_kind(report))));
    }
    if crashed > 0 {
        row.push_str(&style.dim(&format!(
            " · {crashed} crashed where a crash proves nothing"
        )));
    }
    row
}

/// `2 skipped (1 equivalent, 1 ignored as unreachable)`: the kind is each reason's first word,
/// and an ignore's the sort it was given, as in the Java edition.
fn skipped_by_kind(report: &Report) -> String {
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for result in &report.results {
        if result.status == Status::Ignored {
            *by_kind
                .entry(summary_kind(result.reason.as_deref().unwrap_or_default()))
                .or_default() += 1;
        }
    }
    let kinds: Vec<String> = by_kind
        .iter()
        .map(|(kind, count)| format!("{count} {kind}"))
        .collect();
    format!(
        "{} skipped ({})",
        report.count(Status::Ignored),
        kinds.join(", ")
    )
}

/// `equivalent` for `equivalent: …`; `ignored as unreachable` for `user: unreachable: … (ignored)`;
/// `ignored` for an ignore that doesn't say its sort.
fn summary_kind(reason: &str) -> String {
    let (kind, rest) = reason.split_once(':').unwrap_or(("other", ""));
    if !reason.ends_with("(ignored)") {
        return kind.to_string();
    }
    let given = rest.trim_start();
    ignores::KINDS
        .iter()
        .find(|sort| given.starts_with(&format!("{sort}:")))
        .map_or_else(
            || "ignored".to_string(),
            |sort| format!("ignored as {sort}"),
        )
}

/// `unviable` for `unviable: doesn't compile (…)`.
fn skip_kind(result: &MutantResult) -> &str {
    result
        .reason
        .as_deref()
        .and_then(|reason| reason.split_once(':'))
        .map_or("other", |(kind, _)| kind)
}

/// The three functions with the most missed mutants: where to start.
fn worst_at(report: &Report) -> Option<String> {
    let mut by_function: Vec<(usize, usize)> = Vec::new();
    for result in &report.results {
        if result.status == Status::Survived {
            let function = report.found.mutants[result.mutant].function;
            match by_function.iter_mut().find(|(f, _)| *f == function) {
                Some((_, count)) => *count += 1,
                None => by_function.push((function, 1)),
            }
        }
    }
    // Stable: functions with as many misses stay in the order found.
    by_function.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
    let crate_prefix = format!("{}::", report.project.crate_name);
    let worst: Vec<String> = by_function
        .iter()
        .take(3)
        .map(|&(function, count)| {
            let path = &report.found.functions[function].path;
            format!(
                "{}() {count} ✗",
                path.strip_prefix(&crate_prefix).unwrap_or(path)
            )
        })
        .collect();
    (!worst.is_empty()).then(|| worst.join(" · "))
}

/// The tests run against mutants, and how many there would have been running every test on every
/// mutant.
fn test_runs_row(report: &Report, style: Style) -> String {
    let ran = report
        .results
        .iter()
        .filter(|r| !matches!(r.status, Status::Ignored | Status::NoCoverage))
        .count();
    format!(
        "{}{}",
        report.test_runs,
        style.dim(&format!(
            " ({} without coverage selection)",
            ran * report.tests
        ))
    )
}

fn score_row(report: &Report, style: Style) -> String {
    let threshold = report.threshold;
    let Some(score) = report.score() else {
        return format!(
            "{}{}",
            style.bold("none"),
            style.dim(&format!(" (no mutants to score)  threshold {threshold}"))
        );
    };
    let score = percent(Some(score));
    let covered = if report.count(Status::NoCoverage) > 0 && report.covered_score().is_some() {
        style.dim(&format!(
            "  · {} where tests reach",
            percent(report.covered_score())
        ))
    } else {
        String::new()
    };
    format!(
        "{}{}{}{covered}",
        if report.passed() {
            style.green(&style.bold(&score))
        } else {
            style.red(&style.bold(&score))
        },
        delta(report, style),
        style.dim(&format!("  threshold {threshold}"))
    )
}

/// `  ▲ 2.0 vs last run (same code)`, when there was a last run to compare with, as the Java
/// edition writes it.
fn delta(report: &Report, style: Style) -> String {
    let (Some(now), Some(found)) = (report.score(), &report.comparison) else {
        return String::new();
    };
    let Some(before) = found.previous_score else {
        return String::new();
    };
    let change = ((now - before) * 10.0).round() / 10.0;
    let sameness = style.dim(&format!(" ({})", found.sameness()));
    if change == 0.0 {
        return format!("{}{sameness}", style.dim("  = vs last run"));
    }
    let text = format!(
        "{} {:.1} vs last run",
        if change > 0.0 { "▲" } else { "▼" },
        change.abs()
    );
    let shown = if change > 0.0 {
        style.green(&text)
    } else {
        style.red(&text)
    };
    format!("  {shown}{sameness}")
}

/// The last line: PASS or FAIL, and why.
fn verdict(report: &Report, style: Style) -> String {
    let missed = report.count(Status::Survived);
    if report.score().is_none() {
        format!(
            "{} {}",
            style.label("PASS", GREEN),
            style.dim("Nothing to measure: no mutants to score.")
        )
    } else if report.passed() {
        format!(
            "{} {}",
            style.label("PASS", GREEN),
            style.green(&if missed == 0 {
                style.words.all_detected().to_string()
            } else {
                format!(
                    "{missed} {}, above the threshold.",
                    style.words.undetected()
                )
            })
        )
    } else {
        format!(
            "{} {}",
            style.label("FAIL", RED),
            style.red(&format!(
                "Score below {}: {missed} {}, {} {}.",
                report.threshold,
                style.words.undetected(),
                report.count(Status::NoCoverage),
                style.words.not_covered()
            ))
        )
    }
}

/// "not mutated: unsafe (3), …": the code left alone around unsafe, and how to include it; then
/// "not built here: `cfg(windows)` (2)", the code this build doesn't have.
fn not_mutated(report: &Report) -> Vec<String> {
    let mut lines = Vec::new();
    let count = report.found.not_mutated_unsafe;
    if count > 0 {
        lines.push(format!(
            "not mutated: unsafe ({count}), in or around unsafe blocks; --unsafe=enclosing or --unsafe=all includes them"
        ));
    }
    if !report.found.not_built.is_empty() {
        let by_cfg: Vec<String> = report
            .found
            .not_built
            .iter()
            .map(|(cfg, count)| format!("`{cfg}` ({count})"))
            .collect();
        lines.push(format!("not built here: {}", by_cfg.join(", ")));
    }
    lines
}

fn agent(report: &Report, style: Style) -> String {
    let words = style.words;
    let view = View { report };
    let mut out = String::new();
    let missed = report.count(Status::Survived);
    let not_covered = report.count(Status::NoCoverage);
    let skipped = report.count(Status::Ignored);
    let dir = &report.project.given;
    let _ = writeln!(
        out,
        "mutastic: {} · score {} (threshold {}{}) · {missed} {}, {not_covered} {}, {} {} of {} mutants{}",
        if report.passed() { "PASS" } else { "FAIL" },
        report.score().map_or("none (no mutants to score)".into(), |s| format!("{s:.1}%")),
        report.threshold,
        if not_covered == 0 || report.covered_score().is_none() {
            String::new()
        } else {
            format!("; {} where tests reach", percent(report.covered_score()))
        },
        words.undetected(),
        words.not_covered(),
        report.count(Status::Killed) + report.count(Status::Timeout),
        words.detected(),
        report.results.len() - skipped,
        if skipped == 0 {
            String::new()
        } else {
            format!(" ({skipped} more skipped)")
        }
    );
    for flaky in &report.flaky {
        let _ = writeln!(
            out,
            "flaky: {} failed, then passed when run again; left out of this run, so its catches don't count. {} Never remove, disable or loosen it to get a run through",
            flaky.test,
            flaky_advice(flaky)
        );
    }
    if let Some(found) = &report.comparison {
        let _ = writeln!(
            out,
            "vs last run: {} → {} ({})",
            percent(found.previous_score),
            percent(report.score()),
            found.sameness()
        );
    }
    for (test, mutants) in likely_flaky(report) {
        let _ = writeln!(
            out,
            "likely flaky: {test}; with the code unchanged, whether it catches {} changed since the last run. Its catches can't be trusted until it's fixed",
            mutants.join(", ")
        );
    }
    let _ = writeln!(
        out,
        "rules: {RULE}; a fix counts when `mutastic verify <id> {dir}` passes"
    );
    let _ = writeln!(
        out,
        "decline: {DECLINE} (`{}`)",
        view.command("propose", "<id>")
    );
    for line in not_mutated(report) {
        let _ = writeln!(out, "{line}");
    }
    let entries = ranked(&view, view.entries());
    for (index, entry) in entries.iter().take(10).enumerate() {
        let _ = write!(out, "\n{}. {}", index + 1, agent_entry(&view, entry, words));
    }
    if entries.len() > 10 {
        let _ = writeln!(
            out,
            "\n…and {} more: run with --format=jsonl for them all",
            entries.len() - 10
        );
    }
    // As the Java edition's agent text: what no test reaches, after the misses.
    let places = not_covered_places(&view);
    if !places.is_empty() {
        let _ = writeln!(
            out,
            "\nnot covered (no test reaches these lines{}{}):",
            or_only_flaky(report),
            if entries.is_empty() {
                ""
            } else {
                if words == Vocabulary::Plain {
                    "; fix the missed mutants first"
                } else {
                    "; fix the survived mutants first"
                }
            }
        );
        for (place, mutants) in places {
            let count = mutants.len();
            let _ = writeln!(
                out,
                "- {place} ({count} mutant{})",
                if count == 1 { "" } else { "s" }
            );
            // An agent decides a line no test reaches can't be reached as it decides a missed
            // mutant's case can't happen: so each id, by line, with how to propose it.
            let mut lines: Vec<usize> = mutants.iter().map(|(line, _)| *line).collect();
            lines.sort_unstable();
            lines.dedup();
            for line in &lines {
                let ids: Vec<&str> = mutants
                    .iter()
                    .filter(|(at, _)| at == line)
                    .map(|(_, id)| id.as_str())
                    .collect();
                let _ = writeln!(
                    out,
                    "  or propose{}: {}",
                    if lines.len() == 1 {
                        String::new()
                    } else {
                        format!(" (line {line})")
                    },
                    view.command("propose", &ids.join(","))
                );
            }
        }
    }
    out
}

fn agent_entry(view: &View, entry: &Entry, words: Vocabulary) -> String {
    let result = entry.main;
    let mutant = view.mutant(result);
    let advice = view.advice(result);
    let ids = id_list(view, entry);
    let shown = view.changed_lines(mutant);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} {}: {}",
        words.undetected(),
        mutant.id,
        advice.headline
    );
    let _ = writeln!(out, "   at {}:{}", view.file(mutant), mutant.line);
    for line in &shown.before {
        let _ = writeln!(out, "   - {line}");
    }
    for line in &shown.after {
        let _ = writeln!(out, "   + {line}");
    }
    for other in &entry.also {
        let _ = writeln!(
            out,
            "   also {} {}: {}",
            words.undetected(),
            view.id(other),
            view.advice(other).headline
        );
        let other_shown = view.changed_lines(view.mutant(other));
        if other_shown.after.is_empty() {
            let _ = writeln!(out, "     change: {}", view.advice(other).change);
        }
        for line in &other_shown.after {
            let _ = writeln!(out, "     + {line}");
        }
    }
    let _ = writeln!(
        out,
        "   covered by: {}; all still pass with the change",
        result.covered_by.join(", ")
    );
    for note in &advice.notes {
        let _ = writeln!(out, "   evidence: {note}");
    }
    let each = view.each_mutant(entry);
    let own = each.iter().filter(|(_, test)| test.is_some()).count() > 1;
    let fix = if own {
        each_help(&each)
    } else {
        advice.help.as_str()
    };
    let _ = writeln!(out, "   fix: {fix}");
    for (result, test) in &each {
        if own {
            let help = &view.advice(result).help;
            let _ = writeln!(out, "   fix for {}: {help}", view.id(result));
        }
        if let Some(test) = test {
            let _ = writeln!(out, "   test to add in {}:", test.place);
            for line in &test.lines {
                let _ = writeln!(out, "     {line}");
            }
        }
    }
    if each.iter().any(|(_, test)| test.is_some()) {
        let _ = writeln!(out, "     // {CHECK_TODAY}");
    }
    let _ = writeln!(out, "   done when: {} passes", view.command("verify", &ids));
    let _ = writeln!(out, "   reproduce: {}", view.command("run", &ids));
    let _ = writeln!(out, "   or propose: {}", view.command("propose", &ids));
    out
}

/// The `mutastic.events/v1` stream.
fn jsonl(report: &Report) -> String {
    let view = View { report };
    let mut lines = vec![json!({
        "schema": SCHEMA,
        "edition": EDITION,
        "reason": "run-started",
        "project": report.project.package,
        "rule": RULE,
        "decline": DECLINE,
        // What decides a mutant's meaning, so runs can be compared: the build's settings, which
        // mutastic forces for overflow checks and debug assertions, and how unsafe code was treated.
        "profile": {
            "name": "mutastic",
            "optLevel": report.profile["opt_level"],
            "overflowChecks": report.profile["overflow_checks"],
            "debugAssertions": report.profile["debug_assertions"],
        },
        "unsafe": report.unsafe_mode.name(),
    })];
    if let Some(share) = &report.share {
        lines[0]["share"] = share.json();
    }
    for result in &report.results {
        lines.push(mutant_event(&view, result));
    }
    let mut finished = json!({
        "schema": SCHEMA,
        "edition": EDITION,
        "reason": "run-finished",
        "toolchain": { "name": "rustc", "version": report.rustc },
        "mutants": report.results.len(),
    });
    for status in [
        Status::Killed,
        Status::Survived,
        Status::Timeout,
        Status::NoCoverage,
        Status::Ignored,
        Status::RuntimeError,
    ] {
        finished[status.event_name()] = json!(report.count(status));
    }
    finished["notMutated"] = json!({
        "unsafe": report.found.not_mutated_unsafe,
        "cfg": report.found.not_built.values().sum::<usize>(),
    });
    finished["score"] = report.score().map_or(Value::Null, |s| json!(round1(s)));
    finished["threshold"] = if report.threshold.fract() == 0.0 {
        json!(report.threshold as i64)
    } else {
        json!(report.threshold)
    };
    finished["passed"] = json!(report.passed());
    finished["coveredScore"] = report
        .covered_score()
        .map_or(Value::Null, |s| json!(round1(s)));
    finished["tests"] = json!(report.tests);
    finished["workers"] = json!(report.workers);
    // A Rust worker is a thread, never stopped and started again: as many start as run.
    finished["workersStarted"] = json!(report.workers);
    finished["coverageMs"] = json!(report.coverage.as_millis() as u64);
    finished["flaky"] = json!(report.flaky.iter().map(|f| &f.test).collect::<Vec<_>>());
    // A Rust test fails with a panic, not a chain of exceptions, so `failure` is empty and
    // nothing came from outside the code.
    finished["flakyEvidence"] = json!(report
        .flaky
        .iter()
        .map(|f| json!({
            "test": f.test,
            "failure": [],
            "reaches": f.reaches,
            "reachesInTest": f.reaches_in_test,
            "outside": false,
        }))
        .collect::<Vec<_>>());
    if let Some(found) = &report.comparison {
        finished["previousScore"] = found
            .previous_score
            .map_or(Value::Null, |s| json!(round1(s)));
        finished["sameCodeAsLastRun"] = json!(found.same_code);
        finished["sameMutasticAsLastRun"] = json!(found.same_tool);
        finished["changedSinceLastRun"] = json!(found
            .flips
            .iter()
            .map(|flip| {
                let mut entry = json!({
                    "id": flip.id,
                    "before": flip.before.schema_name(),
                    "after": flip.after.schema_name(),
                });
                if let Some(test) = &flip.caught_by {
                    entry["caughtBy"] = json!(test);
                }
                entry
            })
            .collect::<Vec<_>>());
    }
    finished["likelyFlaky"] = json!(likely_flaky(report)
        .into_iter()
        .map(|(test, mutants)| json!({
            "test": test,
            "mutants": mutants,
            "reasons": ["changed-since-last-run"],
        }))
        .collect::<Vec<_>>());
    // Nothing of this is built yet, so it's empty, as the schema asks.
    finished["unrelatedCatches"] = json!([]);
    if let Some(scope) = &report.scope {
        finished["scope"] = json!({
            "since": scope.since,
            "fileCount": scope.files,
            "lineCount": scope.lines,
        });
    }
    finished["reports"] = json!(report.reports);
    finished["staleIgnores"] = json!(report.stale_ignores);
    finished["build"] = json!({
        "rounds": report.rounds,
        "unviable": report
            .results
            .iter()
            .filter(|r| r.status == Status::Ignored && skip_kind(r) == "unviable")
            .count(),
        // How long reading, finding and building took, before the coverage pass: what a run put
        // together from shares shows as its setup.
        "setupMs": report.setup.as_millis() as u64,
    });
    finished["durationMs"] = json!(report.took.as_millis() as u64);
    lines.push(finished);
    lines
        .iter()
        .map(|line| format!("{line}\n"))
        .collect::<String>()
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn mutant_event(view: &View, result: &MutantResult) -> Value {
    let mutant = view.mutant(result);
    let function = &view.report.found.functions[mutant.function];
    let advice = view.advice(result);
    let mut mutation = json!({});
    match &mutant.change {
        Change::Boundary { before, after }
        | Change::Reversed { before, after }
        | Change::Connector { before, after }
        | Change::Math { before, after } => {
            mutation["original"] = json!(before);
            mutation["replacement"] = json!(after);
        }
        Change::Returns { value } => mutation["replacement"] = json!(value),
        Change::Swapped { before, after }
        | Change::Replaced { before, after }
        | Change::Compared { before, after, .. } => {
            mutation["original"] = json!(before);
            mutation["replacement"] = json!(after);
        }
        Change::Body { value } => mutation["replacement"] = json!(value),
        Change::ArmRemoved { pattern } => {
            mutation["original"] = json!(pattern);
            mutation["replacement"] = json!(mutant.replacement);
        }
        Change::Parameter { name, value } => {
            mutation["original"] = json!(name);
            mutation["replacement"] = json!(value);
        }
        Change::Removed { statement } => {
            mutation["original"] = json!(statement);
            mutation["replacement"] = json!(mutant.replacement);
        }
        Change::Always { truth, .. } => mutation["replacement"] = json!(truth.to_string()),
    }
    mutation["description"] = json!(advice.change);
    // `applied` is the change as the engine makes it, and at what level: here, to the source, the
    // whole site before and after, which no other field has for every change.
    let text = view.text(mutant);
    mutation["applied"] = json!({
        "level": "source",
        "text": format!("{} -> {}", sites_plain(&text[mutant.site.clone()]), mutant.replacement),
    });
    let mut event = json!({
        "schema": SCHEMA,
        "edition": EDITION,
        "reason": "mutant-finished",
        "id": mutant.id,
        "fingerprint": mutant.fingerprint,
        "status": result.status.schema_name(),
        "operator": mutant.operator,
        "location": {
            "file": view.file(mutant),
            "line": mutant.line,
            "column": mutant.column,
            // The schema's names: the item that holds the function (`pricing::Pricing`), and
            // the function.
            "class": function.path.rsplit_once("::").map_or(view.report.project.crate_name.as_str(), |(holder, _)| holder),
            "method": function.name,
        },
        "mutation": mutation,
    });
    if let (Some(docs), Some(fields)) = (
        crate::operator_docs::url(mutant.operator),
        event.as_object_mut(),
    ) {
        // Beside the operator it describes, as the Java edition writes it.
        let at = fields
            .keys()
            .position(|key| key == "operator")
            .map_or(fields.len(), |operator| operator + 1);
        fields.shift_insert(at, "docs".to_string(), json!(docs));
    }
    if let Some(reason) = &result.reason {
        event["statusReason"] = json!(reason);
        if result.status == Status::Ignored {
            event["skipKind"] = json!(skip_kind(result));
        }
    }
    if let Some(near) = mutant.near_unsafe {
        event["nearUnsafe"] = json!(near);
    }
    event["coveredBy"] = json!(result.covered_by);
    if let Some(test) = &result.killed_by {
        event["caughtBy"] = json!(test);
    }
    if let Some(failure) = &result.killed_with {
        event["caughtWith"] = json!(failure);
    }
    if !result.inputs.is_empty() {
        event["inputs"] = json!(result
            .inputs
            .iter()
            .map(|(name, values)| (
                name.clone(),
                json!(values
                    .iter()
                    .map(|v| crate::shares::whole(*v))
                    .collect::<Vec<_>>())
            ))
            .collect::<serde_json::Map<_, _>>());
    }
    event["testsRun"] = json!(result.tests_run);
    event["durationMs"] = json!(result.took.as_millis() as u64);
    if result.status == Status::Survived {
        event["doneWhen"] = json!(format!("{} passes", view.command("verify", &mutant.id)));
        event["reproduce"] = json!(view.command("run", &mutant.id));
        event["ignore"] = json!(view.command("ignore", &mutant.id));
        event["propose"] = json!(view.command("propose", &mutant.id));
        if let Some(test) = view.suggested_test(result, false) {
            event["suggestedTest"] = json!({
                "testClass": test.module,
                "name": test.name,
                "code": test.lines.join("\n"),
                "call": test.call,
                "today": test.today,
                "withMutant": test.mutated,
            });
            if let Some(found) = &result.suggestion {
                event["suggestion"] = suggestion(view, mutant, found, &test.call);
            }
        }
        let entry = Entry {
            main: result,
            also: Vec::new(),
        };
        // A machine format: the standard words, whatever the vocabulary.
        event["rendered"] = json!(agent_entry(view, &entry, Vocabulary::Plain));
    }
    event
}

/// The call a suggested test was found by, whole, as the Java edition writes it: what a share's
/// events need for the card to be written again from them. The receiver's numbers come first in
/// what the probe found, then the function's own.
fn suggestion(view: &View, mutant: &Mutant, found: &suggest::Suggestion, call: &str) -> Value {
    let callable = view.report.found.functions[mutant.function]
        .callable
        .as_ref();
    let receiver = callable.and_then(|callable| callable.receiver.as_ref());
    let taken = receiver.map_or(0, |receiver| receiver.parameters.len());
    let (made, own) = found.values.split_at(taken.min(found.values.len()));
    let numbers = |values: &[i128]| -> Vec<Value> {
        values.iter().map(|v| crate::shares::whole(*v)).collect()
    };
    let mut value = json!({
        "call": call,
        "parameters": callable.map_or_else(Vec::new, |callable| callable
            .parameters
            .iter()
            .map(|(name, _)| name.clone())
            .collect()),
        "arguments": numbers(own),
        "receiver": receiver.map_or_else(Vec::new, |receiver| vec![json!({
            "type": receiver.type_name,
            "arguments": numbers(made),
        })]),
        "today": found.today,
        "withMutant": found.mutated,
    });
    if let Some(test) = &found.built_by {
        value["builtBy"] = json!(test);
    }
    value
}

/// Source text on one line, whitespace collapsed.
fn sites_plain(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// How the last full run went for a mutant, from its `mutation.json`: its status, and the test
/// that caught it if one did.
pub struct LastRun {
    pub status: String,
    pub caught_by: Option<String>,
}

/// The last full run's result for `id`, from the `mutation.json` it left in `target/mutastic`.
pub fn last_run(report_file: &std::path::Path, id: &str) -> Option<LastRun> {
    let text = std::fs::read_to_string(report_file).ok()?;
    let document: Value = serde_json::from_str(&text).ok()?;
    document["files"]
        .as_object()?
        .values()
        .flat_map(|file| file["mutants"].as_array().cloned().unwrap_or_default())
        .find(|mutant| mutant["id"] == id)
        .map(|mutant| LastRun {
            status: mutant["status"].as_str().unwrap_or_default().to_string(),
            caught_by: mutant["killedBy"][0].as_str().map(str::to_string),
        })
}

/// `explain`: one mutant in full whatever became of it, as the Java edition explains it: a miss
/// gets its card; anything else a card that says what happened instead, without help or a test to
/// add; then what the change is, its operator, its id, its site and how the last full run went.
pub fn explained(report: &Report, last: Option<LastRun>, style: Style) -> String {
    let view = View { report };
    let mut out = String::new();
    let Some(result) = report.results.first() else {
        return out;
    };
    if result.status == Status::Survived {
        card(
            &mut out,
            &view,
            &Entry {
                main: result,
                also: Vec::new(),
            },
            style,
        );
    } else {
        outcome_card(&mut out, &view, result, style);
    }
    about(&mut out, &view, result, last, style);
    out
}

fn outcome_card(out: &mut String, view: &View, result: &MutantResult, style: Style) {
    let mutant = view.mutant(result);
    let advice = view.advice(result);
    let (label, outcome) = match result.status {
        Status::Killed => (
            style.label(
                &format!("✓ {}", style.words.detected().to_uppercase()),
                GREEN,
            ),
            format!(
                "caught by {}{}",
                result.killed_by.as_deref().unwrap_or("a test"),
                match result.killed_with.as_deref() {
                    Some(how) if how.starts_with("crash") => {
                        format!(", by a crash ({how})")
                    }
                    _ => String::new(),
                }
            ),
        ),
        Status::Timeout => (
            style.label(
                &format!("✓ {}", style.words.detected().to_uppercase()),
                GREEN,
            ),
            "its tests stopped finishing in time, which counts as caught".to_string(),
        ),
        Status::NoCoverage => (
            style.label("○ NOT COVERED", CYAN),
            "no test reaches this line".to_string(),
        ),
        Status::RuntimeError => (
            style.label("SKIPPED", CYAN),
            format!(
                "crashed where a crash proves nothing: {}",
                result.reason.as_deref().unwrap_or_default()
            ),
        ),
        Status::Ignored | Status::Survived => (
            style.label("SKIPPED", CYAN),
            result.reason.clone().unwrap_or_default(),
        ),
    };
    let _ = writeln!(out, "\n {label} {}", style.bold(&outcome));
    let docs = crate::operator_docs::url(mutant.operator);
    let g = frame(out, view, mutant, &advice, docs.as_deref(), style);
    before_and_after(out, &g, &view.changed_lines(mutant), style);
    let mut notes = advice.notes.clone();
    if !result.covered_by.is_empty() {
        // Whether they still pass is a miss's to say: here, only which tests reach it.
        notes.push(format!("covered by {}", result.covered_by.join(", ")));
    }
    for (index, note) in notes.iter().enumerate() {
        if index == 0 {
            let _ = writeln!(
                out,
                "{g}{} {}: {}",
                style.dim("="),
                style.blue(&style.bold("note")),
                style.code(note)
            );
        } else {
            let _ = writeln!(out, "{g}        {}", style.dim(&style.code(note)));
        }
    }
    let _ = writeln!(
        out,
        "\n{g}{} {}",
        style.dim("reproduce "),
        view.command("run", &mutant.id)
    );
}

/// `About this mutant`: the change, the operator, the id, the site, and the last full run.
fn about(
    out: &mut String,
    view: &View,
    result: &MutantResult,
    last: Option<LastRun>,
    style: Style,
) {
    let mutant = view.mutant(result);
    let function = &view.report.found.functions[mutant.function];
    let title = " About this mutant ";
    let _ = writeln!(
        out,
        "\n{}{}",
        style.bold(title),
        style.dim(&"─".repeat(66 - title.len()))
    );
    let text = view.text(mutant);
    let row = |out: &mut String, label: &str, first: String, rest: Vec<String>| {
        let _ = writeln!(
            out,
            "    {} {}  {first}",
            style.dim("="),
            style.blue(&style.bold(&format!("{label:<8}")))
        );
        for line in rest {
            let _ = writeln!(out, "{}{}", " ".repeat(16), style.dim(&line));
        }
    };
    row(
        out,
        "change",
        style.bold(&format!(
            "{} → {}",
            sites_plain(&text[mutant.site.clone()]),
            mutant.replacement
        )),
        vec!["the code the switch puts in place of the original when this mutant is live".into()],
    );
    row(
        out,
        "operator",
        format!(
            "{}: {}",
            mutant.operator,
            crate::callrules::find(mutant.operator).map_or_else(
                || view.advice(result).change,
                |rule| rule.explanation.clone()
            )
        ),
        // Where the docs say more, as text to copy in any terminal; a rules file's operator has
        // no section there.
        crate::operator_docs::url(mutant.operator)
            .map(|url| format!("docs: {url}"))
            .into_iter()
            .collect(),
    );
    row(
        out,
        "id",
        format!(
            "{}: `m` and the first 8 hex digits of its fingerprint, {}, a SHA-256 of the crate, function, operator, site and change",
            mutant.id, mutant.fingerprint
        ),
        vec!["no line number goes in, so the id stays the same when code above it moves".into()],
    );
    row(
        out,
        "site",
        format!(
            "`{}` in {}, line {}, column {}",
            sites_plain(&text[mutant.site.clone()]),
            function.path,
            mutant.line,
            mutant.column
        ),
        Vec::new(),
    );
    let last = match last {
        None => "none yet: `run` or `next` records one".to_string(),
        Some(run) => match run.status.as_str() {
            "Killed" => format!(
                "caught{}",
                run.caught_by
                    .map(|t| format!(" by {t}"))
                    .unwrap_or_default()
            ),
            "Survived" => "missed: every covering test still passed".into(),
            "Timeout" => "caught: its tests ran out of time".into(),
            "NoCoverage" => "not covered: no test reached its line".into(),
            "RuntimeError" => "crashed where a crash proves nothing".into(),
            _ => "skipped".into(),
        },
    };
    row(out, "last run", last, Vec::new());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lines a card shows for `claimed` replaced by `with` in `text`.
    fn shown(text: &str, claimed: &str, with: &str, forced: bool) -> Shown {
        let at = text.find(claimed).unwrap();
        let range = at..at + claimed.len();
        let extent = statement_extent(text, &range);
        let starts = sites::line_starts(text);
        let first = starts[sites::line_of(&starts, extent.start) - 1];
        let last = text[extent.end..]
            .find('\n')
            .map_or(text.len(), |end| extent.end + end);
        let before: Vec<&str> = text[first..last].lines().collect();
        let mut changed = format!(
            "{}{with}{}",
            &text[first..range.start],
            &text[range.end..last]
        );
        if forced {
            changed = simplified(&changed);
        }
        let after: Vec<&str> = changed.lines().collect();
        Shown::new(&before, &after)
    }

    #[test]
    fn an_ignore_is_counted_by_the_sort_it_was_given() {
        assert_eq!(
            summary_kind("equivalent: a `?` on a formatter"),
            "equivalent"
        );
        assert_eq!(
            summary_kind("user: unreachable: totals are tens (ignored)"),
            "ignored as unreachable"
        );
        assert_eq!(summary_kind("user: totals are tens (ignored)"), "ignored");
        assert_eq!(
            summary_kind("user: protobuf output (rule `skip: shop::generated::**`)"),
            "user"
        );
    }

    #[test]
    fn a_statement_over_several_lines_shows_whole() {
        let text = "fn f(next: &[u8]) -> bool {\n    let thrown = (!next.is_empty() && next[0] == 1)\n        || next.len() > 3;\n    thrown\n}\n";
        let card = shown(text, "next.is_empty()", "true", true);
        assert_eq!(
            card.before,
            [
                "let thrown = (!next.is_empty() && next[0] == 1)",
                "    || next.len() > 3;"
            ]
        );
        assert_eq!(
            card.after,
            [
                "let thrown = (false && next[0] == 1)",
                "    || next.len() > 3;"
            ]
        );
    }

    #[test]
    fn a_condition_shows_up_to_its_end_and_not_its_body() {
        let text =
            "fn f(a: u8, b: u8) {\n    if a > 1\n        && b > 2\n    {\n        g();\n    }\n}\n";
        let card = shown(text, ">", ">=", false);
        assert_eq!(card.before, ["if a > 1", "    && b > 2"]);
        assert_eq!(card.after, ["if a >= 1", "    && b > 2"]);
    }

    #[test]
    fn a_forced_value_reads_as_itself_and_a_long_statement_is_cut() {
        assert_eq!(
            simplified("if !true && !(false) || !trueish {"),
            "if false && true || !trueish {"
        );
        let lines: Vec<String> = (1..=9).map(|n| format!("    {n},")).collect();
        let before: Vec<&str> = lines.iter().map(String::as_str).collect();
        let mut after = before.clone();
        after[8] = "    10,";
        let card = Shown::new(&before, &after);
        assert_eq!(card.after.first().map(String::as_str), Some("…"));
        assert_eq!(card.after.last().map(String::as_str), Some("10,"));
        assert_eq!(card.after.len(), Shown::MOST_LINES + 1);
    }
}
