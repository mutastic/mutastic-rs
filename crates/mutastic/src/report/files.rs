//! The report files a full run writes, as the Java edition writes them: `mutation.json`, in the
//! mutation-testing report schema (version 2), and `mutation.html`, that report in the schema's
//! own viewer, embedded so the page needs no network; and `mutation.sarif`, SARIF 2.1.0 for code
//! scanning, when asked for (`sarif.rs`).

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Value};

use super::{Status, View};
use crate::commands::DOCS;
use crate::engine::{MutantResult, Report};

/// The viewer the HTML report embeds: mutation-testing-elements 3.9.0, unchanged, as the Java
/// edition embeds it (see `THIRD-PARTY.md`).
const VIEWER: &str = include_str!("viewer/mutation-test-elements.js");

/// A report a run can write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Json,
    Html,
    Sarif,
}

impl Kind {
    pub fn parse(name: &str) -> Option<Kind> {
        match name {
            "json" => Some(Kind::Json),
            "html" => Some(Kind::Html),
            "sarif" => Some(Kind::Sarif),
            _ => None,
        }
    }

    fn default_name(self) -> &'static str {
        match self {
            Kind::Json => "mutation.json",
            Kind::Html => "mutation.html",
            Kind::Sarif => "mutation.sarif",
        }
    }
}

/// The reports a run writes when `--report` doesn't say: both.
pub const DEFAULT: &[(Kind, Option<&str>)] = &[(Kind::Json, None), (Kind::Html, None)];

/// Writes each report, to its path (against the directory mutastic was pointed at) or to `folder`,
/// `target/mutastic` or a share's own, and gives the paths written, as the summary and
/// `run-finished` show them: from that directory when they're inside it, whole otherwise.
pub fn write(
    report: &Report,
    reports: &[(Kind, Option<String>)],
    folder: &Path,
) -> Result<Vec<String>, String> {
    let given = Path::new(&report.project.given);
    let mut written = Vec::new();
    for (kind, path) in reports {
        let file = match path {
            Some(path) => given.join(path),
            None => folder.join(kind.default_name()),
        };
        let text = match kind {
            Kind::Json => format!("{:#}\n", json(report)),
            Kind::Html => html(report),
            Kind::Sarif => format!("{:#}\n", super::sarif::document(report)),
        };
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("couldn't write {}: {error}", file.display()))?;
        }
        std::fs::write(&file, text)
            .map_err(|error| format!("couldn't write {}: {error}", file.display()))?;
        written.push(shown(given, &file));
    }
    Ok(written)
}

/// `target/mutastic/mutation.json`, from `given`, or the whole path when it's outside.
pub fn shown(given: &Path, file: &Path) -> String {
    let real = |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    match real(file).strip_prefix(real(given)) {
        Ok(inside) => crate::workspace::slashed(inside),
        Err(_) => file.display().to_string(),
    }
}

/// The report-schema document. Statuses are the schema's (`Killed`, `Survived`, …); locations are
/// 1-based, with an exclusive end.
pub fn json(report: &Report) -> Value {
    let view = View { report };
    let threshold = report.threshold;
    json!({
        "$schema": "https://raw.githubusercontent.com/stryker-mutator/mutation-testing-elements/master/packages/report-schema/src/mutation-testing-report-schema.json",
        "schemaVersion": "2",
        // One threshold in mutastic: at or above it is good; the warning band is the 20 below.
        "thresholds": { "high": number(threshold), "low": number((threshold - 20.0).max(0.0)) },
        "projectRoot": report.project.workspace.display().to_string(),
        "files": files(&view),
        "testFiles": test_files(report),
        "framework": { "name": "mutastic", "version": env!("CARGO_PKG_VERSION") },
        "performance": {
            "setup": 0,
            "initialRun": report.coverage.as_millis() as u64,
            "mutation": report.took.saturating_sub(report.coverage).as_millis() as u64,
        },
        "mutastic": run(&view),
    })
}

/// What mutastic knows of the run beyond the report schema (`report/v1`, under `mutastic`): the
/// commands its output gives for a mutant, with `<id>` for the mutant's id, where its docs are,
/// and how the run ended, as `run-finished` says it.
fn run(view: &View) -> Value {
    let report = view.report;
    let command = |verb: &str| super::mutant_command(verb, "<id>", &report.project.given);
    let mut operators = serde_json::Map::new();
    for result in &report.results {
        let mutant = view.mutant(result);
        let page = crate::operator_docs::url(mutant.operator)
            .and_then(|url| url.strip_prefix(DOCS).map(str::to_string));
        if let Some(page) = page {
            operators.insert(mutator_name(mutant.operator).to_string(), json!(page));
        }
    }
    operators.sort_keys();
    json!({
        "edition": super::EDITION,
        "commands": {
            "verify": command("verify"),
            "run": command("run"),
            "ignore": command("ignore"),
            "propose": command("propose"),
        },
        "docsBase": DOCS,
        "operatorDocs": operators,
        "score": report.score().map_or(Value::Null, |s| json!(super::round1(s))),
        "threshold": number(report.threshold),
        "passed": report.passed(),
        "toolchain": { "name": "rustc", "version": report.rustc },
        "startedAt": utc(report.started),
    })
}

/// `boundary` for `mutastic/boundary`: the report schema's `mutatorName`.
fn mutator_name(operator: &str) -> &str {
    operator.strip_prefix("mutastic/").unwrap_or(operator)
}

/// `2026-11-13T20:34:31Z`: a time in UTC, to the second.
fn utc(time: std::time::SystemTime) -> String {
    let seconds = time
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Days since 1970-01-01 to a date in the proleptic Gregorian calendar (Howard Hinnant's
    // `civil_from_days`), in 400-year eras that start on 1 March.
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// A whole number as one, as the Java edition writes its integer threshold.
fn number(value: f64) -> Value {
    if value.fract() == 0.0 {
        json!(value as i64)
    } else {
        json!(value)
    }
}

/// Each source file's entry, by its path: its text and its mutants.
fn files(view: &View) -> Value {
    let mut files: BTreeMap<String, (String, Vec<Value>)> = BTreeMap::new();
    for result in &view.report.results {
        let mutant = view.mutant(result);
        files
            .entry(view.file(mutant))
            .or_insert_with(|| (view.text(mutant).to_string(), Vec::new()))
            .1
            .push(mutant_entry(view, result));
    }
    files
        .into_iter()
        .map(|(path, (source, mutants))| {
            (
                path,
                json!({ "language": "rust", "source": source, "mutants": mutants }),
            )
        })
        .collect::<serde_json::Map<_, _>>()
        .into()
}

fn mutant_entry(view: &View, result: &MutantResult) -> Value {
    let mutant = view.mutant(result);
    let advice = view.advice(result);
    let text = view.text(mutant);
    let (range, replacement) = mutant.claim();
    let mut entry = json!({
        "id": mutant.id,
        "mutatorName": mutator_name(mutant.operator),
        "replacement": replacement,
        "description": advice.change,
        "location": {
            "start": position(text, range.start),
            "end": position(text, range.end),
        },
        "status": result.status.schema_name(),
    });
    let reason = status_reason(result, &advice.headline);
    if !reason.is_empty() {
        entry["statusReason"] = json!(reason);
    }
    entry["coveredBy"] = json!(result.covered_by);
    if let Some(test) = &result.killed_by {
        entry["killedBy"] = json!([test]);
    }
    entry["testsCompleted"] = json!(result.tests_run);
    entry["duration"] = json!(result.took.as_millis() as u64);
    let more = mutant_more(view, result, &advice);
    if !more.is_empty() {
        entry["mutastic"] = Value::Object(more);
    }
    entry
}

/// What mutastic knows of a mutant beyond the report schema, as its card and its event say it:
/// where it is, and for a miss, the card's evidence and fix and the test to add. Only what there
/// is.
fn mutant_more(
    view: &View,
    result: &MutantResult,
    advice: &crate::words::Advice,
) -> serde_json::Map<String, Value> {
    let mutant = view.mutant(result);
    let missed = result.status == Status::Survived;
    let mut more = serde_json::Map::new();
    more.insert(
        "method".into(),
        json!(super::qualified(&view.report.found, mutant)),
    );
    if missed && !advice.notes.is_empty() {
        more.insert("evidence".into(), json!(advice.notes));
    }
    if missed {
        more.insert("fix".into(), json!(advice.help));
    }
    if let Some(inputs) = super::inputs(result) {
        more.insert("inputs".into(), inputs);
    }
    if let Some((mut test, found)) = missed.then(|| super::suggested(view, result)).flatten() {
        let found = found.unwrap_or_default();
        // The value of each of the function's own parameters, by name, as the call passes it.
        let arguments: serde_json::Map<String, Value> = found["parameters"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .zip(found["arguments"].as_array().into_iter().flatten())
            .map(|(name, value)| (name.to_string(), value.clone()))
            .collect();
        if !arguments.is_empty() {
            test["arguments"] = Value::Object(arguments);
        }
        more.insert("suggestedTest".into(), test);
    }
    if let Some(near) = mutant.near_unsafe {
        more.insert("nearUnsafe".into(), json!(near));
    }
    more
}

/// `{"line": 26, "column": 18}` for a byte offset: 1-based, the column in characters.
pub(super) fn position(text: &str, offset: usize) -> Value {
    let offset = offset.min(text.len());
    let starts = crate::sites::line_starts(text);
    let line = crate::sites::line_of(&starts, offset);
    let column = text[starts[line - 1]..offset].chars().count() + 1;
    json!({ "line": line, "column": column })
}

/// Why the mutant has its status, or nothing.
fn status_reason(result: &MutantResult, headline: &str) -> String {
    match result.status {
        Status::Ignored | Status::RuntimeError => result.reason.clone().unwrap_or_default(),
        Status::Killed => result.killed_with.clone().unwrap_or_default(),
        Status::Survived => headline.to_string(),
        Status::Timeout => "the tests didn't finish in time".into(),
        Status::NoCoverage => "no test reaches this line".into(),
    }
}

/// The tests, by the file that holds them, relative to `projectRoot`, as the Java edition keys
/// them: each file's text, and its tests, with where each is declared when that's known. A test no
/// target of the package holds is under its module's name, `tests`, with no text.
fn test_files(report: &Report) -> Value {
    let tests: std::collections::BTreeSet<&String> = report
        .results
        .iter()
        .flat_map(|result| &result.covered_by)
        .collect();
    let mut by_file: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for id in tests {
        let mut test = json!({ "id": id, "name": id.rsplit("::").next().unwrap_or(id) });
        let file = match crate::test_files::place(&report.project, id) {
            Some(place) => {
                if let Some((line, column)) = place.at {
                    test["location"] = json!({ "start": { "line": line, "column": column } });
                }
                crate::workspace::slashed(&place.file)
            }
            None => id
                .rsplit_once("::")
                .map_or("", |(module, _)| module)
                .to_string(),
        };
        by_file.entry(file).or_default().push(test);
    }
    by_file
        .into_iter()
        .map(|(file, tests)| {
            let mut entry = json!({ "tests": tests });
            let path = Path::new(&file);
            if path.extension().is_some_and(|extension| extension == "rs") {
                if let Ok(source) = std::fs::read_to_string(report.project.workspace.join(path)) {
                    entry["source"] = json!(source);
                }
            }
            (file, entry)
        })
        .collect::<serde_json::Map<_, _>>()
        .into()
}

/// The JSON in the schema's viewer, with the viewer itself, so the page needs no network.
pub fn html(report: &Report) -> String {
    // `</` can't appear inside a script element: `<\/` means the same in JavaScript.
    let data = json(report).to_string().replace("</", "<\\/");
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>mutastic · {}</title>\n<script>\n{}\n</script>\n</head>\n<body>\n\
         <mutation-test-report-app titlePostfix=\"mutastic\"></mutation-test-report-app>\n\
         <script>\ndocument.querySelector('mutation-test-report-app').report = {data};\n</script>\n\
         </body>\n</html>\n",
        escape(&report.project.package),
        VIEWER.replace("</script", "<\\/script")
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_is_written_in_utc() {
        let at = |seconds| utc(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds));
        assert_eq!(at(0), "1970-01-01T00:00:00Z");
        assert_eq!(at(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(at(1_794_601_271), "2026-11-13T20:21:11Z");
        assert_eq!(at(4_102_444_799), "2099-12-31T23:59:59Z");
    }
}
