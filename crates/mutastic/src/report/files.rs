//! The report files a full run writes, as the Java edition writes them: `mutation.json`, in the
//! mutation-testing report schema (version 2), and `mutation.html`, that report in mutastic's own
//! page, the one the Java edition embeds, so it needs no network; and `mutation.sarif`, SARIF 2.1.0 for code
//! scanning, when asked for (`sarif.rs`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
/// `target/mutastic` or a share's own, and gives the files written, each whole.
pub fn write(
    report: &Report,
    reports: &[(Kind, Option<String>)],
    folder: &Path,
) -> Result<Vec<PathBuf>, String> {
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
        written.push(whole(&file));
    }
    Ok(written)
}

/// `file` whole: against the directory mutastic runs in when it's relative.
pub fn whole(file: &Path) -> PathBuf {
    if file.is_absolute() {
        return file.to_path_buf();
    }
    std::env::current_dir().map_or_else(|_| file.to_path_buf(), |here| here.join(file))
}

/// `file` as the person would type it from where mutastic runs: relative when it's below there,
/// as `fixtures/pricing/target/mutastic/mutation.html`, whole otherwise. A path from the project
/// reads wrong from anywhere else, and a terminal guessing a link from one opened `/target/…`.
pub fn from_here(file: &Path) -> String {
    let file = whole(file);
    match std::env::current_dir() {
        Ok(here) if real(&file) != real(&here) => {
            let shown = shown(&here, &file);
            if Path::new(&shown).is_absolute() {
                file.display().to_string()
            } else {
                shown
            }
        }
        _ => file.display().to_string(),
    }
}

/// A message's logs, `Its output is in /…/target/mutastic/tests.log.`, each as typed from where
/// mutastic runs and, with links on, linked to the file itself. A relative one is linked only when
/// it's there from here.
pub fn logs(said: &str, style: super::Style) -> String {
    const LEAD: &str = "output is in ";
    let mut out = String::with_capacity(said.len());
    let mut rest = said;
    while let Some(at) = rest.find(LEAD) {
        let (before, after) = rest.split_at(at + LEAD.len());
        out.push_str(before);
        let end = after.find(char::is_whitespace).unwrap_or(after.len());
        let (word, tail) = after.split_at(end);
        let (named, stop) = word
            .strip_suffix('.')
            .map_or((word, ""), |named| (named, "."));
        out.push_str(&log(named, style).unwrap_or_else(|| named.to_string()));
        out.push_str(stop);
        rest = tail;
    }
    out.push_str(rest);
    out
}

/// `named` shown from here and linked, when it names a file to link.
fn log(named: &str, style: super::Style) -> Option<String> {
    let path = Path::new(named);
    if named.is_empty() || (!path.is_absolute() && !whole(path).is_file()) {
        return None;
    }
    let file = whole(path);
    Some(style.link(&file, &from_here(&file)))
}

fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// `target/mutastic/mutation.json`, from `given`, or the whole path when it's outside.
pub fn shown(given: &Path, file: &Path) -> String {
    // As given first: on Windows a path that isn't there yet can't be made real, and the real
    // directory it's in is a `\\?\` path it would never match.
    if let Ok(inside) = file.strip_prefix(given) {
        return crate::workspace::slashed(inside);
    }
    let real = |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    match real(file).strip_prefix(real(given)) {
        Ok(inside) => crate::workspace::slashed(inside),
        Err(_) => file.display().to_string(),
    }
}

/// The report-schema document. Statuses are the schema's (`Killed`, `Survived`, …); locations are
/// 1-based, with an exclusive end.
pub fn json(report: &Report) -> Value {
    let view = View::of(report, false);
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
    for result in view.results {
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

    /// A log a message names is shown from here and, with links on, linked whole; one named
    /// relative that isn't there from here is left as said. The Java edition's `HereTest` is the
    /// same.
    #[test]
    fn a_log_in_a_message_is_shown_from_here_and_linked_whole() {
        let here = std::env::current_dir().unwrap();
        let log = here
            .join("pricing")
            .join("target")
            .join("mutastic")
            .join("tests.log");
        let said = format!(
            "t fails.\nIts output is in {}. To run it alone:",
            log.display()
        );
        let links = super::super::Style {
            links: true,
            ..super::super::Style::default()
        };

        assert_eq!(
            logs(&said, super::super::Style::default()),
            "t fails.\nIts output is in pricing/target/mutastic/tests.log. To run it alone:"
        );
        assert!(logs(&said, links).contains(&format!(
            "Its output is in \x1b]8;;{}\x1b\\pricing/target/mutastic/tests.log\x1b]8;;\x1b\\. To",
            super::super::file_uri(&log)
        )));
        let relative = "Their output is in target/nowhere/tests.log.";
        assert_eq!(logs(relative, links), relative);
    }

    /// A file's URI holds what a path can, percent-encoding the rest, as Java's `toUri` does.
    #[test]
    fn a_file_uri_encodes_what_a_uri_cant_hold() {
        assert_eq!(
            super::super::file_uri(Path::new("/work/my project/#1/mutation.html")),
            "file:///work/my%20project/%231/mutation.html"
        );
    }
}
