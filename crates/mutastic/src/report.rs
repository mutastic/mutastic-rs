//! The formats: `full` cards for people, `concise` lines for logs, `agent` text for coding agents,
//! and `jsonl`, the `mutastic.events/v1` stream for tools. The words are the Java edition's.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::ops::Range;

use serde_json::{json, Value};

use crate::engine::{MutantResult, Report, Status};
use crate::sites::{self, Change, Crate, Mutant};
use crate::suggest::{self, SuggestedTest};
use crate::words::{self, Advice};

pub mod files;

pub const SCHEMA: &str = "mutastic.events/v1";
/// The events format's name for this edition, on every event.
pub const EDITION: &str = "rust";
pub const RULE: &str = "add or change tests only, never production code";
pub const DECLINE: &str = "if a mutant's case can't happen in practice, or the change is one no caller can see, don't contrive a test: say why";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Full,
    Concise,
    Agent,
    Jsonl,
}

impl Format {
    pub fn parse(name: &str) -> Option<Format> {
        match name {
            "full" => Some(Format::Full),
            "concise" => Some(Format::Concise),
            "agent" => Some(Format::Agent),
            "jsonl" => Some(Format::Jsonl),
            _ => None,
        }
    }
}

/// ANSI colour, only where it's wanted: the Java edition's colours, code for code. Every colour has
/// a glyph or a word beside it, so the output reads the same without it.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub color: bool,
}

/// Label backgrounds.
const RED: &str = "41";
const GREEN: &str = "42";
const CYAN: &str = "46";

impl Style {
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
        words::advise(mutant, name, &result.inputs)
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

    /// The tests suggested for an entry's mutants, each test once: two mutants of one line can
    /// have the same call, and pasting it twice wouldn't compile. On a card with others, each
    /// test's comment names the mutant it was found for.
    fn suggested(&self, entry: &Entry) -> Vec<SuggestedTest> {
        let shared = !entry.also.is_empty();
        let mut tests: Vec<SuggestedTest> = Vec::new();
        for result in std::iter::once(entry.main).chain(entry.also.iter().copied()) {
            if let Some(test) = self.suggested_test(result, shared) {
                if !tests.iter().any(|kept| kept.name == test.name) {
                    tests.push(test);
                }
            }
        }
        tests
    }

    fn suggested_test(&self, result: &MutantResult, named: bool) -> Option<SuggestedTest> {
        let suggestion = result.suggestion.as_ref()?;
        let mutant = self.mutant(result);
        let callable = self.report.found.functions[mutant.function]
            .callable
            .as_ref()?;
        let at = suggest::named(
            callable,
            &mutant.operands,
            self.text(mutant),
            &suggestion.values,
        );
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
        let mut by_line: BTreeMap<(String, usize), Vec<&'a MutantResult>> = BTreeMap::new();
        for result in &self.report.results {
            if result.status == Status::Survived {
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
        let dir = &self.report.project.given;
        match verb {
            "verify" => format!("mutastic verify {ids} {dir}"),
            "explain" => format!("mutastic explain {ids} {dir}"),
            "ignore" | "propose" => {
                let propose = if verb == "propose" { " --propose" } else { "" };
                match ids.split_once(',') {
                    // One decision each: a card with several mutants names them all.
                    Some(_) => format!(
                        "mutastic ignore <id>{propose} --reason=\"<why>\" {dir} for each of {}",
                        ids.replace(',', ", ")
                    ),
                    None => format!("mutastic ignore {ids}{propose} --reason=\"<why>\" {dir}"),
                }
            }
            _ => format!("mutastic run --only={ids} {dir}"),
        }
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
/// it), and in their order otherwise.
fn ranked<'a>(view: &View<'a>, mut entries: Vec<Entry<'a>>) -> Vec<Entry<'a>> {
    entries.sort_by_key(|entry| {
        let best = std::iter::once(entry.main)
            .chain(entry.also.iter().copied())
            .map(|r| prior(view.mutant(r)))
            .max()
            .unwrap_or(1);
        (
            std::cmp::Reverse(best),
            std::cmp::Reverse(entry.main.covered_by.len()),
        )
    });
    entries
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
        Format::Full => human(report, style, true),
        Format::Concise => human(report, style, false),
        Format::Agent => agent(report),
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
        counts.push(style.red(&format!("{} missed", missed.len())));
    }
    if !uncovered.is_empty() {
        counts.push(style.amber(&format!("{} not covered", uncovered.len())));
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

fn human(report: &Report, style: Style, cards: bool) -> String {
    let view = View { report };
    let mut out = String::new();
    let entries = view.entries();
    if !entries.is_empty() {
        let _ = writeln!(
            out,
            "\n{}{}",
            style.red(&style.bold(" Missed ")),
            style.dim(&"─".repeat(60))
        );
        if cards {
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
                    style.bold(&file),
                    style.dim(&format!(
                        "{missed} missed in {} function{}",
                        functions.len(),
                        if functions.len() == 1 { "" } else { "s" }
                    ))
                );
                for entry in entries {
                    card(&mut out, &view, entry, style);
                }
            }
        } else {
            for entry in &entries {
                let mutant = view.mutant(entry.main);
                let _ = writeln!(
                    out,
                    " {} {}:{} {} {}",
                    style.red("✗"),
                    view.file(mutant),
                    mutant.line,
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
    if report.flaky.is_empty() {
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
/// it; gives the gutter the rest of the card is indented by.
fn frame(out: &mut String, view: &View, mutant: &Mutant, advice: &Advice, style: Style) -> String {
    let text = view.text(mutant);
    let lines: Vec<&str> = text.lines().collect();
    let width = (mutant.line + 1).to_string().len();
    let g = " ".repeat(width + 2);
    let bar = style.dim("│");
    let _ = writeln!(
        out,
        "{g}{} {}:{}",
        style.dim("┌─"),
        view.file(mutant),
        mutant.line
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
            let _ = writeln!(
                out,
                "{g}{bar} {}{}",
                " ".repeat(mutant.column - 1),
                style.red(&format!("{} {}", "^".repeat(caret_width), advice.caret))
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

fn card(out: &mut String, view: &View, entry: &Entry, style: Style) {
    let result = entry.main;
    let mutant = view.mutant(result);
    let advice = view.advice(result);
    let shared = !entry.also.is_empty();
    let _ = writeln!(
        out,
        "\n {} {}{}",
        style.label("✗ MISSED", RED),
        style.code(&advice.headline),
        if shared {
            style.dim(&format!(" · {}", mutant.id))
        } else {
            String::new()
        }
    );
    let g = frame(out, view, mutant, &advice, style);
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
    let _ = writeln!(
        out,
        "{g}{equals} {}: {}",
        style.green(&style.bold("help")),
        style.code(&advice.help)
    );
    let tests = view.suggested(entry);
    if !tests.is_empty() {
        let _ = writeln!(out);
        for test in &tests {
            let _ = writeln!(out, "{g}        // in {}", test.place);
            for line in &test.lines {
                let _ = writeln!(out, "{g}        {line}");
            }
        }
        let _ = writeln!(out, "{g}        {}", style.dim(CHECK_TODAY));
    }
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
fn not_covered_places(view: &View) -> Vec<(String, usize)> {
    let mut by_function: Vec<(usize, Vec<usize>)> = Vec::new();
    for result in &view.report.results {
        if result.status == Status::NoCoverage {
            let mutant = view.mutant(result);
            match by_function.iter_mut().find(|(f, _)| *f == mutant.function) {
                Some((_, lines)) => lines.push(mutant.line),
                None => by_function.push((mutant.function, vec![mutant.line])),
            }
        }
    }
    by_function
        .into_iter()
        .map(|(function, lines)| {
            let (from, to) = (
                lines.iter().min().copied().unwrap_or(0),
                lines.iter().max().copied().unwrap_or(0),
            );
            let f = &view.report.found.functions[function];
            let place = if from == to {
                from.to_string()
            } else {
                format!("{from}-{to}")
            };
            (
                format!("{}:{place} {}()", f.file.display(), f.name),
                lines.len(),
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
    for (place, count) in places.iter().take(NOT_COVERED_SHOWN) {
        let _ = writeln!(
            out,
            "   {} {place} {}",
            style.amber("○"),
            style.dim(&format!(
                "({count} mutant{})",
                if *count == 1 { "" } else { "s" }
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
        parts.push(style.red(&style.bold(&format!("{with_misses} with misses"))));
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
    let mut counts = vec![style.green(&format!("✓ {}", style.bold(&format!("{caught} caught"))))];
    if missed > 0 {
        counts.push(style.red(&format!("✗ {}", style.bold(&format!("{missed} missed")))));
    }
    if timed_out > 0 {
        counts.push(style.amber(&format!("⏱ {timed_out} timed out")));
    }
    if not_covered > 0 {
        counts.push(style.amber(&format!("○ {not_covered} not covered")));
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

/// `2 skipped (1 equivalent, 1 unviable)`: the kind is each reason's first word, as in the Java
/// edition.
fn skipped_by_kind(report: &Report) -> String {
    let mut by_kind: BTreeMap<&str, usize> = BTreeMap::new();
    for result in &report.results {
        if result.status == Status::Ignored {
            *by_kind.entry(skip_kind(result)).or_default() += 1;
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
        "{}{}{covered}",
        if report.passed() {
            style.green(&style.bold(&score))
        } else {
            style.red(&style.bold(&score))
        },
        style.dim(&format!("  threshold {threshold}"))
    )
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
                "Every mutant was caught.".to_string()
            } else {
                format!("{missed} missed, above the threshold.")
            })
        )
    } else {
        format!(
            "{} {}",
            style.label("FAIL", RED),
            style.red(&format!(
                "Score below {}: {missed} missed, {} not covered.",
                report.threshold,
                report.count(Status::NoCoverage)
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

fn agent(report: &Report) -> String {
    let view = View { report };
    let mut out = String::new();
    let missed = report.count(Status::Survived);
    let not_covered = report.count(Status::NoCoverage);
    let skipped = report.count(Status::Ignored);
    let dir = &report.project.given;
    let _ = writeln!(
        out,
        "mutastic: {} · score {} (threshold {}{}) · {missed} missed, {not_covered} not covered, {} caught of {} mutants{}",
        if report.passed() { "PASS" } else { "FAIL" },
        report.score().map_or("none (no mutants to score)".into(), |s| format!("{s:.1}%")),
        report.threshold,
        if not_covered == 0 || report.covered_score().is_none() {
            String::new()
        } else {
            format!("; {} where tests reach", percent(report.covered_score()))
        },
        report.count(Status::Killed) + report.count(Status::Timeout),
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
    let _ = writeln!(
        out,
        "rules: {RULE}; a fix counts when `mutastic verify <id> {dir}` passes"
    );
    let _ = writeln!(out, "decline: {DECLINE}");
    for line in not_mutated(report) {
        let _ = writeln!(out, "{line}");
    }
    let entries = ranked(&view, view.entries());
    for (index, entry) in entries.iter().take(10).enumerate() {
        let _ = write!(out, "\n{}. {}", index + 1, agent_entry(&view, entry));
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
                "; fix the missed mutants first"
            }
        );
        for (place, count) in places {
            let _ = writeln!(
                out,
                "- {place} ({count} mutant{})",
                if count == 1 { "" } else { "s" }
            );
        }
    }
    out
}

fn agent_entry(view: &View, entry: &Entry) -> String {
    let result = entry.main;
    let mutant = view.mutant(result);
    let advice = view.advice(result);
    let ids = id_list(view, entry);
    let shown = view.changed_lines(mutant);
    let mut out = String::new();
    let _ = writeln!(out, "missed {}: {}", mutant.id, advice.headline);
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
            "   also missed {}: {}",
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
    let _ = writeln!(out, "   fix: {}", advice.help);
    let tests = view.suggested(entry);
    for test in &tests {
        let _ = writeln!(out, "   test to add in {}:", test.place);
        for line in &test.lines {
            let _ = writeln!(out, "     {line}");
        }
    }
    if !tests.is_empty() {
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
    // Nothing of these is built yet, so each is empty, as the schema asks.
    for none in ["likelyFlaky", "unrelatedCatches"] {
        finished[none] = json!([]);
    }
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
        event["rendered"] = json!(agent_entry(view, &entry));
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
    json!({
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
    })
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
            style.label("✓ CAUGHT", GREEN),
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
            style.label("✓ CAUGHT", GREEN),
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
    let g = frame(out, view, mutant, &advice, style);
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
        Vec::new(),
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
        None => "none yet: a full run records one".to_string(),
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
