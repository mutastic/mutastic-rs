//! What a run tells GitHub Actions when it runs there, with no token or permission, as the Java
//! edition's `GitHubActions` does: a warning annotation on the line of each of the first misses,
//! ranked as the agent format ranks them, which the pull request's diff shows; and a Markdown
//! summary for the job's page.
//!
//! Paths are relative to the git repository the workspace is in, as GitHub resolves an
//! annotation's file against the repository's root, or to the workspace where there's no
//! repository.

use std::path::{Path, PathBuf};

use super::sarif::{from_base, repository};
use super::{ranked, Entry, Status, View};
use crate::engine::{MutantResult, Report};
use crate::untrusted;

/// How many misses get an annotation: as many as the agent format shows.
pub const LIMIT: usize = 10;

/// Whether a run is on GitHub Actions and tells it so: `GITHUB_ACTIONS` is `true`, and
/// `MUTASTIC_GITHUB` isn't `off`. `variable` reads one.
pub fn on(variable: impl Fn(&str) -> Option<String>) -> bool {
    variable("GITHUB_ACTIONS").as_deref() == Some("true")
        && variable("MUTASTIC_GITHUB").as_deref() != Some("off")
}

/// The job summary's file, `GITHUB_STEP_SUMMARY`, when it names one.
pub fn summary_file(variable: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    variable("GITHUB_STEP_SUMMARY")
        .filter(|file| !file.trim().is_empty())
        .map(PathBuf::from)
}

/// `::warning file=…,line=…,title=…::…` for each of the first misses: a workflow command the
/// runner turns into an annotation.
pub fn annotations(report: &Report) -> Vec<String> {
    let view = View::of(report, true);
    let base = repository(&report.project.workspace);
    first_misses(&view)
        .iter()
        .map(|entry| {
            let mutant = view.mutant(entry.main);
            format!(
                "::warning file={},line={},title={}::{}",
                property(&from_root(report, &mutant.file, base.as_deref())),
                mutant.line,
                property(&format!("mutastic: missed {}", mutant.id)),
                data(&view.advice(entry.main).headline)
            )
        })
        .collect()
}

/// The job summary: the verdict and score, the counts, and the first misses with where they are.
/// With `--fail-on-new`, the verdict is on the new misses, not the score.
pub fn summary(report: &Report) -> String {
    let mut lines = vec![
        format!(
            "### mutastic: {} · score {} ({})",
            verdict(report),
            report
                .score()
                .map_or("none".to_string(), |score| format!("{score:.1}%")),
            if report.fail_on_new.is_some() {
                "threshold not applied".to_string()
            } else {
                format!("threshold {}", report.threshold)
            }
        ),
        String::new(),
        "| Caught | Missed | Not covered | Timed out |".to_string(),
        "|---|---|---|---|".to_string(),
        format!(
            "| {} | {} | {} | {} |",
            report.count(Status::Killed),
            report.count(Status::Survived),
            report.count(Status::NoCoverage),
            report.count(Status::Timeout)
        ),
    ];
    lines.extend(misses_table(report));
    lines.join("\n") + "\n"
}

/// `PASS` or `FAIL`; with `--fail-on-new`, the new misses that decided it, or the test that fails
/// on its own.
fn verdict(report: &Report) -> String {
    match &report.fail_on_new {
        Some(_) if !report.failing_alone().is_empty() => {
            "FAIL (a test that caught mutants fails on its own, with no mutant in)".to_string()
        }
        Some(judged) => judged.verdict(report),
        None if report.passed() => "PASS".to_string(),
        None => "FAIL".to_string(),
    }
}

/// The first misses as a table, and how many more there are; nothing when none was missed.
fn misses_table(report: &Report) -> Vec<String> {
    let view = View::of(report, true);
    let first = first_misses(&view);
    if first.is_empty() {
        return Vec::new();
    }
    let base = repository(&report.project.workspace);
    let mut lines = vec![
        String::new(),
        "**Missed, most worth a test first**".to_string(),
        String::new(),
        "| Mutant | Where | No test checks |".to_string(),
        "|---|---|---|".to_string(),
    ];
    lines.extend(
        first
            .iter()
            .map(|entry| row(report, &view, entry.main, base.as_deref())),
    );
    let more = report.count(Status::Survived) - first.len();
    if more > 0 {
        lines.push(String::new());
        lines.push(format!(
            "and {more} more missed. `mutastic explain <id>` shows any one."
        ));
    }
    lines
}

/// A miss's row: its id, where it is, and what no test checks.
fn row(report: &Report, view: &View, result: &MutantResult, base: Option<&Path>) -> String {
    let mutant = view.mutant(result);
    format!(
        "| `{}` | `{}:{}` | {} |",
        mutant.id,
        untrusted::value(&from_root(report, &mutant.file, base)),
        mutant.line,
        view.advice(result).headline.replace('|', "\\|")
    )
}

/// The first [`LIMIT`] lines' misses, most worth a test first, each written about its lead.
fn first_misses<'a>(view: &View<'a>) -> Vec<Entry<'a>> {
    let mut first = ranked(view, view.entries());
    first.truncate(LIMIT);
    first
}

/// `src/lib.rs`, relative to the workspace, as it's relative to the repository's root.
fn from_root(report: &Report, file: &Path, base: Option<&Path>) -> String {
    from_base(&report.project.workspace, file, base)
}

/// A workflow command's message, its `%`, carriage returns and newlines escaped.
fn data(text: &str) -> String {
    text.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// A workflow command's property: as [`data`], and its `:` and `,` too.
fn property(text: &str) -> String {
    data(text).replace(':', "%3A").replace(',', "%2C")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        |name| {
            pairs
                .iter()
                .find(|(set, _)| *set == name)
                .map(|(_, value)| value.to_string())
        }
    }

    #[test]
    fn it_is_on_only_where_github_actions_says_true_and_mutastic_github_is_not_off() {
        assert!(on(with(&[("GITHUB_ACTIONS", "true")])));
        assert!(on(with(&[
            ("GITHUB_ACTIONS", "true"),
            ("MUTASTIC_GITHUB", "on")
        ])));
        assert!(!on(with(&[
            ("GITHUB_ACTIONS", "true"),
            ("MUTASTIC_GITHUB", "off")
        ])));
        assert!(!on(with(&[("GITHUB_ACTIONS", "1")])));
        assert!(!on(with(&[("CI", "true")])));
    }

    #[test]
    fn a_blank_summary_file_is_none() {
        assert_eq!(summary_file(with(&[("GITHUB_STEP_SUMMARY", " ")])), None);
        assert_eq!(summary_file(with(&[])), None);
        assert_eq!(
            summary_file(with(&[("GITHUB_STEP_SUMMARY", "/tmp/summary.md")])),
            Some(PathBuf::from("/tmp/summary.md"))
        );
    }

    #[test]
    fn a_workflow_command_escapes_what_would_end_it() {
        assert_eq!(data("50% of\r\nit"), "50%25 of%0D%0Ait");
        assert_eq!(
            property("mutastic: missed m1, 50%"),
            "mutastic%3A missed m1%2C 50%25"
        );
    }
}
