//! The report files a full run writes, as the Java edition writes them: `mutation.json`, in the
//! mutation-testing report schema (version 2), and `mutation.html`, that report in the schema's
//! own viewer, embedded so the page needs no network.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Value};

use super::{Status, View};
use crate::engine::{MutantResult, Report};

/// The viewer the HTML report embeds: mutation-testing-elements 3.9.0, unchanged, as the Java
/// edition embeds it (see `THIRD-PARTY.md`).
const VIEWER: &str = include_str!("viewer/mutation-test-elements.js");

/// A report a run can write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Json,
    Html,
}

impl Kind {
    pub fn parse(name: &str) -> Option<Kind> {
        match name {
            "json" => Some(Kind::Json),
            "html" => Some(Kind::Html),
            _ => None,
        }
    }

    fn default_name(self) -> &'static str {
        match self {
            Kind::Json => "mutation.json",
            Kind::Html => "mutation.html",
        }
    }
}

/// The reports a run writes when `--report` doesn't say: both.
pub const DEFAULT: &[(Kind, Option<&str>)] = &[(Kind::Json, None), (Kind::Html, None)];

/// Writes each report, to its path (against the directory mutastic was pointed at) or to
/// `target/mutastic`, and gives the paths written, as the summary and `run-finished` show them:
/// from that directory when they're inside it, whole otherwise.
pub fn write(report: &Report, reports: &[(Kind, Option<String>)]) -> Result<Vec<String>, String> {
    let given = Path::new(&report.project.given);
    let mut written = Vec::new();
    for (kind, path) in reports {
        let file = match path {
            Some(path) => given.join(path),
            None => report.project.mutastic_dir().join(kind.default_name()),
        };
        let text = match kind {
            Kind::Json => format!("{:#}\n", json(report)),
            Kind::Html => html(report),
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
fn shown(given: &Path, file: &Path) -> String {
    let real = |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    real(file)
        .strip_prefix(real(given))
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| file.to_path_buf())
        .display()
        .to_string()
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
    })
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
        "mutatorName": mutant.operator.strip_prefix("mutastic/").unwrap_or(mutant.operator),
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
    entry
}

/// `{"line": 26, "column": 18}` for a byte offset: 1-based, the column in characters.
fn position(text: &str, offset: usize) -> Value {
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
        Status::Survived => crate::words::plain(headline),
        Status::Timeout => "the tests didn't finish in time".into(),
        Status::NoCoverage => "no test reaches this line".into(),
    }
}

/// The tests, by the module that holds them: `pricing::tests` → its tests.
fn test_files(report: &Report) -> Value {
    let mut by_module: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for result in &report.results {
        for test in &result.covered_by {
            let module = test.rsplit_once("::").map_or("", |(module, _)| module);
            by_module
                .entry(module.to_string())
                .or_default()
                .push(test.clone());
        }
    }
    by_module
        .into_iter()
        .map(|(module, mut tests)| {
            tests.sort();
            tests.dedup();
            let tests: Vec<Value> = tests
                .iter()
                .map(|id| json!({ "id": id, "name": id.rsplit("::").next().unwrap_or(id) }))
                .collect();
            (module, json!({ "tests": tests }))
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
