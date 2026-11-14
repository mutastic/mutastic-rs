//! The report files a full run writes, as the Java edition writes them: `mutation.json`, in the
//! mutation-testing report schema (version 2), and `mutation.html`, that report in mutastic's own
//! page, the one the Java edition embeds, so it needs no network; and `mutation.sarif`, SARIF 2.1.0 for code
//! scanning, when asked for (`sarif.rs`).

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Value};

use super::{Status, View};
use crate::engine::{MutantResult, Report};

/// The HTML report's page as mutastic's `report-ui/` builds it, one file that needs no network,
/// vendored at the mutastic commit `page/report-page.json` pins: the Java edition embeds the same page.
/// `scripts/pin-report-page.sh` moves the pin.
const PAGE: &str = include_str!("page/report.html");

/// The page's empty slot for the report, which it holds once.
const SLOT: &str = "<script type=\"application/json\" id=\"mutation-report\"></script>";

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
    let view = View {
        report,
        shown: false,
    };
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

/// The JSON in mutastic's report page. Every `<` in it is written as JSON's escape for it, which
/// reads back the same, so nothing in the data (a `</script>` or a `<!--` in a source file) can end
/// the page's script element or change how the page reads it.
pub fn html(report: &Report) -> String {
    let data = json(report).to_string().replace('<', "\\u003c");
    PAGE.replacen(
        SLOT,
        &format!("<script type=\"application/json\" id=\"mutation-report\">{data}</script>"),
        1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vendored page is the one `page/report-page.json` pins, byte for byte, and holds its slot
    /// once. `tests/report_page.rs` checks the pin against mutastic's own checksum at that commit.
    #[test]
    fn the_page_is_the_pinned_one() {
        use sha2::{Digest, Sha256};
        let pin: Value = serde_json::from_str(include_str!("page/report-page.json")).unwrap();
        let sum: String = Sha256::digest(PAGE.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(
            pin["sha256"].as_str().unwrap(),
            sum,
            "src/report/page/report.html isn't the pinned page: scripts/pin-report-page.sh <commit>"
        );
        assert_eq!(PAGE.matches(SLOT).count(), 1);
    }
}
