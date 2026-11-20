//! The report files a full run writes, as the Java edition writes them: `mutation.json`, in the
//! mutation-testing report schema (version 2), and `mutation.html`, that report in mutastic's own
//! page, the one the Java edition embeds, so it needs no network; and `mutation.sarif`, SARIF 2.1.0 for code
//! scanning, when asked for (`sarif.rs`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::{Status, View};
use crate::commands::DOCS;
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

/// The folder of mutant pages beside an HTML report: `mutation-mutants/` beside `mutation.html`.
pub fn pages_folder(html: &Path) -> PathBuf {
    let stem = html
        .file_stem()
        .map_or_else(|| "mutation".into(), |stem| stem.to_string_lossy());
    html.with_file_name(format!("{stem}-mutants"))
}

/// Writes, beside the HTML report at `html`, a page for each mutant the run didn't catch,
/// `mutation-mutants/m855c8e3c.html`, that opens the report on it, and gives their folder. The
/// output's ids link to these: a link to `mutation.html#/m/<id>` itself loses its `#…` on the way
/// to the browser on macOS (and on Linux and Windows by reports), as the system's opener takes a
/// `file:` URL for a file and drops the rest. A page's own address has nothing to lose, and the
/// browser keeps the `#…` it then goes to. Pages a run before left are taken away first.
pub fn write_pages(report: &Report, html: &Path) -> Result<PathBuf, String> {
    let folder = pages_folder(html);
    let failed = |error: std::io::Error| format!("couldn't write {}: {error}", folder.display());
    std::fs::create_dir_all(&folder).map_err(failed)?;
    for entry in std::fs::read_dir(&folder).map_err(failed)? {
        let path = entry.map_err(failed)?.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_page)
        {
            std::fs::remove_file(&path).map_err(failed)?;
        }
    }
    let name = html
        .file_name()
        .map_or_else(String::new, |name| super::encoded(&name.to_string_lossy()));
    for id in paged(report) {
        std::fs::write(folder.join(format!("{id}.html")), page(&name, id)).map_err(failed)?;
    }
    Ok(folder)
}

/// The ids of the mutants the run didn't catch, each of which gets a page.
pub fn paged(report: &Report) -> BTreeSet<&str> {
    report
        .results
        .iter()
        .filter(|result| result.status != crate::engine::Status::Killed)
        .map(|result| report.found.mutants[result.mutant].id.as_str())
        .collect()
}

/// `m855c8e3c.html`: a mutant's page, which a run may take away.
fn is_page(name: &str) -> bool {
    name.strip_suffix(".html")
        .and_then(|id| id.strip_prefix('m'))
        .is_some_and(|hex| hex.len() == 8 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// A page that opens the report `html`, beside its folder, on mutant `id`, as the Java edition
/// writes it: at once with a script, by refresh without one, and by its link if neither runs.
fn page(html: &str, id: &str) -> String {
    let target = format!("../{html}#/m/{id}");
    format!(
        "<!doctype html>\n<meta charset=\"utf-8\">\n<title>{id} · mutastic report</title>\n\
         <meta http-equiv=\"refresh\" content=\"0; url={target}\">\n\
         <script>location.replace('{target}')</script>\n\
         <a href=\"{target}\">{id} in the report</a>\n"
    )
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

/// Whether `dir` is where mutastic runs, so a command offered for it needs no directory.
pub fn is_here(dir: &Path) -> bool {
    std::env::current_dir().is_ok_and(|here| real(&whole(dir)) == real(&here))
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
        "mutastic": run(&view, report),
    })
}

/// What mutastic knows of the run beyond the report schema (`report/v1`, under `mutastic`): the
/// commands its output gives for a mutant, with `<id>` for the mutant's id, where its docs are,
/// and how the run ended, as `run-finished` says it.
fn run(view: &View, report: &Report) -> Value {
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
    more.insert("method".into(), json!(super::qualified(view.found, mutant)));
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
    // The arid rule that skipped it, its docs and what runs it, as the Java edition gives them.
    if let Some(rule) = result
        .reason
        .as_deref()
        .filter(|_| result.status == Status::Ignored)
        .and_then(crate::arid::rule)
    {
        let function = &view.found.functions[mutant.function].path;
        more.insert("skipRule".into(), json!(rule.id));
        more.insert("skipDocs".into(), json!(rule.url()));
        more.insert("bringBack".into(), json!(rule.bring_back(function)));
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

    #[test]
    fn a_time_is_written_in_utc() {
        let at = |seconds| utc(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds));
        assert_eq!(at(0), "1970-01-01T00:00:00Z");
        assert_eq!(at(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(at(1_794_601_271), "2026-11-13T20:21:11Z");
        assert_eq!(at(4_102_444_799), "2099-12-31T23:59:59Z");
    }
}
