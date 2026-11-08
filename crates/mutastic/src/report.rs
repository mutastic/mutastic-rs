//! The formats: `full` cards for people, `concise` lines for logs, `agent` text for coding agents,
//! and `jsonl`, the `mutastic.events/v1` stream for tools. The words are the Java edition's.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::{json, Value};

use crate::engine::{MutantResult, Report, Status};
use crate::sites::{self, Change, Mutant};
use crate::words::{self, Advice};

pub const SCHEMA: &str = "mutastic.events/v1";
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

/// ANSI colour, only where it's wanted.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub color: bool,
}

impl Style {
    fn paint(self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }
    fn red(self, text: &str) -> String {
        self.paint("31", text)
    }
    fn green(self, text: &str) -> String {
        self.paint("32", text)
    }
    fn dim(self, text: &str) -> String {
        self.paint("2", text)
    }
    fn bold(self, text: &str) -> String {
        self.paint("1", text)
    }
    fn label(self, text: &str, background: &str) -> String {
        if self.color {
            format!("\x1b[1;97;{background}m {text} \x1b[0m")
        } else {
            format!("[{text}]")
        }
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
                results.sort_by_key(|r| (rank(self.mutant(r)), r.mutant));
                let main = results.remove(0);
                Entry {
                    main,
                    also: results,
                }
            })
            .collect()
    }

    /// The source line with the change made, without its indent.
    fn changed_line(&self, mutant: &Mutant) -> (String, String) {
        let text = self.text(mutant);
        let starts = sites::line_starts(text);
        let (range, with) = mutant.claim();
        let line_start = starts[sites::line_of(&starts, range.start) - 1];
        let line_end = text[line_start..]
            .find('\n')
            .map_or(text.len(), |end| line_start + end);
        let line = &text[line_start..line_end];
        let start = range.start.max(line_start) - line_start;
        let end = range.end.min(line_end).max(range.start.max(line_start)) - line_start;
        let changed = format!("{}{with}{}", &line[..start], &line[end..]);
        let changed = match changed.trim() {
            "" => "(nothing: the line is gone)".to_string(),
            kept => kept.to_string(),
        };
        (line.trim().to_string(), changed)
    }

    fn command(&self, verb: &str, ids: &str) -> String {
        let dir = &self.report.project.given;
        match verb {
            "verify" => format!("mutastic verify {ids} {dir}"),
            _ => format!("mutastic run --only={ids} {dir}"),
        }
    }
}

/// Conditions and removals before arithmetic, as the Java edition ranks them.
fn rank(mutant: &Mutant) -> u8 {
    match mutant.operator {
        sites::BOUNDARY => 0,
        sites::REMOVE_CONDITIONAL => 1,
        sites::OTHER_COMPARISON => 2,
        sites::LOGICAL_CONNECTOR => 3,
        sites::RETURN_VALUE => 4,
        _ => 5,
    }
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

fn duration(took: std::time::Duration) -> String {
    if took.as_secs_f64() >= 1.0 {
        format!("{:.1}s", took.as_secs_f64())
    } else {
        format!("{}ms", took.as_millis())
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

fn human(report: &Report, style: Style, cards: bool) -> String {
    let view = View { report };
    let mut out = String::new();
    let entries = view.entries();
    if !entries.is_empty() {
        let _ = writeln!(
            out,
            "\n {} {}",
            style.bold("Missed"),
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
                    style.dim("❯"),
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
                    words::plain(&view.advice(entry.main).headline),
                    style.dim(&id_list(&view, entry))
                );
            }
        }
    }
    not_covered(&mut out, &view, style);
    summary(&mut out, report, style);
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
    let headline = words::plain(&advice.headline);
    let _ = writeln!(
        out,
        "\n  {} {}  {}{}",
        style.red("✗"),
        style.label("MISSED", "41"),
        style.bold(&headline),
        if shared {
            style.dim(&format!(" · {}", mutant.id))
        } else {
            String::new()
        }
    );
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
                "{g}{bar} {}{} {}",
                " ".repeat(mutant.column - 1),
                style.red(&"^".repeat(caret_width)),
                style.red(&advice.caret)
            );
        }
    }
    let _ = writeln!(out, "{g}{bar}");
    let (before, after) = view.changed_line(mutant);
    let _ = writeln!(
        out,
        "{g}{}  {}",
        style.dim("= mutant"),
        style.red(&format!("- {before}"))
    );
    let _ = writeln!(out, "{g}          {}", style.green(&format!("+ {after}")));
    for other in &entry.also {
        let other_mutant = view.mutant(other);
        let _ = writeln!(
            out,
            "{g}{}    {} {} {}",
            style.dim("= also"),
            style.red("✗"),
            words::plain(&view.advice(other).headline),
            style.dim(&format!("· {}", other_mutant.id))
        );
        let _ = writeln!(
            out,
            "{g}          {}",
            style.green(&format!("+ {}", view.changed_line(other_mutant).1))
        );
    }
    let mut notes: Vec<String> = advice.notes.iter().map(|n| words::plain(n)).collect();
    notes.push(covered_by(result));
    for (index, note) in notes.iter().enumerate() {
        let label = if index == 0 { "= note:" } else { "       " };
        let _ = writeln!(out, "{g}{} {note}", style.dim(label));
    }
    let _ = writeln!(
        out,
        "{g}{} {}",
        style.dim("= help:"),
        words::plain(&advice.help)
    );
    let ids = id_list(view, entry);
    let _ = writeln!(
        out,
        "\n{g}{}  {} {}",
        style.dim("done when"),
        view.command("verify", &ids),
        style.dim("passes")
    );
    let _ = writeln!(
        out,
        "{g}{}  {}",
        style.dim("reproduce"),
        view.command("run", &ids)
    );
}

fn not_covered(out: &mut String, view: &View, style: Style) {
    let mut by_function: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for result in &view.report.results {
        if result.status == Status::NoCoverage {
            let mutant = view.mutant(result);
            by_function
                .entry(mutant.function)
                .or_default()
                .push(mutant.line);
        }
    }
    if by_function.is_empty() {
        return;
    }
    let _ = writeln!(
        out,
        "\n {} {}",
        style.bold("Not covered"),
        style.dim(&"─".repeat(55))
    );
    for (function, mut lines) in by_function {
        let count = lines.len();
        lines.sort();
        lines.dedup();
        let f = &view.report.found.functions[function];
        let _ = writeln!(
            out,
            " {} {} {}() {}",
            style.dim("○"),
            f.file.display(),
            f.name,
            style.dim(&format!(
                "line{} {} ({count} mutant{}): no test calls it",
                if lines.len() == 1 { "" } else { "s" },
                lines
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                if count == 1 { "" } else { "s" }
            ))
        );
    }
}

fn summary(out: &mut String, report: &Report, style: Style) {
    let caught = report.count(Status::Killed);
    let missed = report.count(Status::Survived);
    let timed_out = report.count(Status::Timeout);
    let not_covered = report.count(Status::NoCoverage);
    let skipped = report.count(Status::Ignored);
    let crashed = report.count(Status::RuntimeError);
    let mut counts = vec![style.green(&format!("✓ {caught} caught"))];
    if missed > 0 {
        counts.push(style.red(&format!("✗ {missed} missed")));
    }
    if timed_out > 0 {
        counts.push(format!("⏱ {timed_out} timed out"));
    }
    if not_covered > 0 {
        counts.push(format!("○ {not_covered} not covered"));
    }
    let scored = report.results.len() - report.unscored();
    let mut mutants = format!("{} ({scored})", counts.join(" · "));
    if skipped > 0 {
        mutants.push_str(&style.dim(&format!(" · {skipped} skipped as unviable")));
    }
    if crashed > 0 {
        mutants.push_str(&style.dim(&format!(
            " · {crashed} crashed where a crash proves nothing"
        )));
    }
    let _ = writeln!(out, "\n{}  {mutants}", style.dim("   Mutants"));
    if let Some(line) = not_mutated(report) {
        let _ = writeln!(out, "{}  {}", style.dim("  Left out"), style.dim(&line));
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
        let _ = writeln!(
            out,
            "{}  {}",
            style.dim("   Crashes"),
            style.dim(&format!(
                "{by_crash} caught only because the test process crashed, not by a test's check"
            ))
        );
    }
    let rounds = report.rounds.len();
    let _ = writeln!(
        out,
        "{}  one build of every mutant, {rounds} round{} (check and build)",
        style.dim("     Build"),
        if rounds == 1 { "" } else { "s" }
    );
    let _ = writeln!(out, "{}  {}", style.dim(" Test runs"), report.test_runs);
    let _ = writeln!(
        out,
        "{}  {} (setup {}, coverage {}, mutants {})",
        style.dim("  Duration"),
        duration(report.took),
        duration(report.setup),
        duration(report.coverage),
        duration(report.mutants)
    );
    let _ = writeln!(
        out,
        "{}  {} on rustc {} · {} worker{}",
        style.dim("     Tests"),
        report.tests,
        report.rustc,
        report.workers,
        if report.workers == 1 { "" } else { "s" }
    );
    let covered = if not_covered > 0 {
        style.dim(&format!(
            "  · {} where tests reach",
            percent(report.covered_score())
        ))
    } else {
        String::new()
    };
    let _ = writeln!(
        out,
        "{}  {}{covered}  {}",
        style.dim("     Score"),
        style.bold(&percent(report.score())),
        style.dim(&format!("threshold {}", report.threshold))
    );
    let verdict = if report.score().is_none() {
        format!(
            "{}  {}",
            style.label("PASS", "42"),
            style.dim("Nothing to measure: no mutants to score.")
        )
    } else if report.passed() {
        format!(
            "{}  {}",
            style.label("PASS", "42"),
            style.green(&if missed == 0 {
                "Every mutant was caught.".to_string()
            } else {
                format!("{missed} missed, above the threshold.")
            })
        )
    } else {
        format!(
            "{}  {}",
            style.label("FAIL", "41"),
            style.red(&format!(
                "Score below {}: {missed} missed, {not_covered} not covered.",
                report.threshold
            ))
        )
    };
    let _ = writeln!(out, "\n  {verdict}");
    if missed > 0 {
        let dir = &report.project.given;
        let _ = writeln!(
            out,
            "{}",
            style.dim(&format!(
                " Next: mutastic verify <id> {dir} · mutastic run --only=<id> {dir}"
            ))
        );
    }
}

/// "not mutated: unsafe (3), …": the code left alone around unsafe, and how to include it.
fn not_mutated(report: &Report) -> Option<String> {
    let count = report.found.not_mutated_unsafe;
    (count > 0).then(|| {
        format!(
            "not mutated: unsafe ({count}), in or around unsafe blocks; --unsafe=enclosing or --unsafe=all includes them"
        )
    })
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
    let _ = writeln!(
        out,
        "rules: {RULE}; a fix counts when `mutastic verify <id> {dir}` passes"
    );
    let _ = writeln!(out, "decline: {DECLINE}");
    if let Some(line) = not_mutated(report) {
        let _ = writeln!(out, "{line}");
    }
    let mut entries = view.entries();
    entries.sort_by_key(|entry| rank(view.mutant(entry.main)));
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
    out
}

fn agent_entry(view: &View, entry: &Entry) -> String {
    let result = entry.main;
    let mutant = view.mutant(result);
    let advice = view.advice(result);
    let ids = id_list(view, entry);
    let (before, after) = view.changed_line(mutant);
    let mut out = String::new();
    let _ = writeln!(out, "missed {}: {}", mutant.id, advice.headline);
    let _ = writeln!(out, "   at {}:{}", view.file(mutant), mutant.line);
    let _ = writeln!(out, "   - {before}");
    let _ = writeln!(out, "   + {after}");
    for other in &entry.also {
        let _ = writeln!(
            out,
            "   also missed {}: {}",
            view.id(other),
            view.advice(other).headline
        );
        let _ = writeln!(out, "     + {}", view.changed_line(view.mutant(other)).1);
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
    let _ = writeln!(out, "   done when: {} passes", view.command("verify", &ids));
    let _ = writeln!(out, "   reproduce: {}", view.command("run", &ids));
    out
}

/// The `mutastic.events/v1` stream.
fn jsonl(report: &Report) -> String {
    let view = View { report };
    let mut lines = vec![json!({
        "schema": SCHEMA,
        "reason": "run-started",
        "project": report.project.package,
        "rules": RULE,
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
    for result in &report.results {
        lines.push(mutant_event(&view, result));
    }
    let mut finished = json!({
        "schema": SCHEMA,
        "reason": "run-finished",
        "rustc": report.rustc,
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
        finished[status.schema_name()] = json!(report.count(status));
    }
    // The schema's names from 0.6.0, beside the old ones until 1.0.
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
    finished["notMutated"] = json!({ "unsafe": report.found.not_mutated_unsafe });
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
    // Nothing of these is built yet, so each is empty, as the schema asks.
    for none in [
        "flaky",
        "flakyEvidence",
        "likelyFlaky",
        "unrelatedCatches",
        "reports",
        "staleIgnores",
    ] {
        finished[none] = json!([]);
    }
    finished["build"] = json!({
        "rounds": report.rounds.len(),
        "unviable": report.count(Status::Ignored),
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
    // The schema's `bytecode` is the change as the engine makes it; in Rust that's the source.
    let text = view.text(mutant);
    mutation["bytecode"] = json!(format!(
        "{} -> {}",
        sites_plain(&text[mutant.site.clone()]),
        mutant.replacement
    ));
    let mut event = json!({
        "schema": SCHEMA,
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
            event["skipKind"] = json!(reason.split(':').next().unwrap_or_default());
        }
    }
    if let Some(near) = mutant.near_unsafe {
        event["nearUnsafe"] = json!(near);
    }
    event["coveredBy"] = json!(result.covered_by);
    if let Some(test) = &result.killed_by {
        event["killedBy"] = json!(test);
        event["caughtBy"] = json!(test);
    }
    if let Some(failure) = &result.killed_with {
        event["killedWith"] = json!(failure);
    }
    if !result.inputs.is_empty() {
        event["inputs"] = json!(result
            .inputs
            .iter()
            .map(|(name, values)| (
                name.clone(),
                json!(values.iter().map(|v| *v as i64).collect::<Vec<_>>())
            ))
            .collect::<serde_json::Map<_, _>>());
    }
    event["testsRun"] = json!(result.tests_run);
    event["durationMs"] = json!(result.took.as_millis() as u64);
    if result.status == Status::Survived {
        event["doneWhen"] = json!(format!("{} passes", view.command("verify", &mutant.id)));
        event["reproduce"] = json!(view.command("run", &mutant.id));
        let entry = Entry {
            main: result,
            also: Vec::new(),
        };
        event["rendered"] = json!(agent_entry(view, &entry));
    }
    event
}

/// Source text on one line, whitespace collapsed.
fn sites_plain(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
