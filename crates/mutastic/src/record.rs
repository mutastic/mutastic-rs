//! What a run found, kept for the next run to compare with, as the Java edition keeps it: each
//! mutant's status and catching test, the score, and hashes of the code and of the mutastic that
//! ran.
//!
//! The file is one entry per line, tab-separated, so it diffs well: `project <package directory>`,
//! `operators <ids>`, `code <hash>`, `tool <hash>`, `score <score>`, then `<id> <status> <caught
//! by or ->`.
//!
//! With the code and mutastic unchanged, a mutant is caught or missed the same way every time
//! unless a test behaves differently between runs. So a mutant caught last run and missed now, or
//! the other way round, points at the test that caught it: likely flaky. With either changed, a
//! flip is just the change at work.

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::engine::{Report, Status};

/// One mutant's result, as the next run compares with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub status: Status,
    /// The test that caught it; none for a timeout, or a mutant not caught.
    pub caught_by: Option<String>,
}

/// A run, as it's kept.
#[derive(Clone, Debug, PartialEq)]
pub struct RunRecord {
    /// Which package it's of, by its directory, so a run is only ever compared with an earlier run
    /// of the same package, in the same place.
    pub project: String,
    /// The ids of the operators the run used, sorted: a score is a share of what they make, so it
    /// says nothing about a run under others.
    pub operators: String,
    /// The hash of every Rust file and Cargo manifest in the workspace: the code and its tests.
    pub code: String,
    /// The hash of the mutastic that ran: a new mutastic can change results too.
    pub tool: String,
    pub score: Option<f64>,
    /// Each mutant's result, by id.
    pub results: BTreeMap<String, Entry>,
}

/// This run against the last one.
#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    /// The last run's score; none when it had no mutants to score.
    pub previous_score: Option<f64>,
    /// Whether the code and tests are byte for byte what the last run had.
    pub same_code: bool,
    /// Whether the same mutastic ran.
    pub same_tool: bool,
    /// Mutants caught in one run and not the other, by id.
    pub flips: Vec<Flip>,
}

/// A mutant caught in one run and not the other.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flip {
    pub id: String,
    pub before: Status,
    pub after: Status,
    /// The test that caught it, in whichever run it was caught; none for a timeout.
    pub caught_by: Option<String>,
}

impl Comparison {
    /// `same code`, `code changed` or `mutastic changed`: what explains a difference.
    pub fn sameness(&self) -> &'static str {
        if !self.same_code {
            "code changed"
        } else if !self.same_tool {
            "mutastic changed"
        } else {
            "same code"
        }
    }

    /// Each likely flaky test and the mutants whose results it changed: empty unless the code and
    /// mutastic are the same.
    pub fn likely_flaky(&self) -> BTreeMap<String, Vec<String>> {
        let mut tests: BTreeMap<String, Vec<String>> = BTreeMap::new();
        if self.same_code && self.same_tool {
            for flip in &self.flips {
                if let Some(test) = &flip.caught_by {
                    tests.entry(test.clone()).or_default().push(flip.id.clone());
                }
            }
        }
        tests
    }
}

/// Caught, by a test or a timeout: what a flip turns on.
fn detected(status: Status) -> bool {
    matches!(status, Status::Killed | Status::Timeout)
}

/// Out of the score, so not caught or missed either: skipped, or crashed where a crash proves
/// nothing.
fn unscored(status: Status) -> bool {
    matches!(status, Status::Ignored | Status::RuntimeError)
}

impl RunRecord {
    /// The record of `report`, run on code whose hash is `code` by a mutastic whose hash is `tool`.
    pub fn of(report: &Report, code: &str, tool: &str) -> RunRecord {
        let results = report
            .results
            .iter()
            .map(|result| {
                let entry = Entry {
                    status: result.status,
                    caught_by: result.killed_by.clone(),
                };
                (report.found.mutants[result.mutant].id.clone(), entry)
            })
            .collect();
        let mut operators: Vec<&str> = report.operators.clone();
        operators.sort_unstable();
        RunRecord {
            project: project_of(report),
            operators: operators.join(","),
            code: code.to_string(),
            tool: tool.to_string(),
            score: report.score(),
            results,
        }
    }

    /// Whether `previous` is of the same package, in the same place: else comparing the two says
    /// nothing.
    pub fn same_project_as(&self, previous: &RunRecord) -> bool {
        !self.project.is_empty() && self.project == previous.project
    }

    /// Whether a comparison with `previous` means anything: the same package, under the same
    /// operators.
    pub fn compares_with(&self, previous: &RunRecord) -> bool {
        self.same_project_as(previous)
            && !self.operators.is_empty()
            && self.operators == previous.operators
    }

    /// How this run differs from `previous`.
    pub fn against(&self, previous: &RunRecord) -> Comparison {
        let flips = self
            .results
            .iter()
            .filter_map(|(id, now)| {
                let before = previous.results.get(id)?;
                let flipped = detected(before.status) != detected(now.status)
                    && !unscored(before.status)
                    && !unscored(now.status);
                flipped.then(|| {
                    let caught = if detected(now.status) { now } else { before };
                    Flip {
                        id: id.clone(),
                        before: before.status,
                        after: now.status,
                        caught_by: caught.caught_by.clone(),
                    }
                })
            })
            .collect();
        Comparison {
            previous_score: previous.score,
            same_code: previous.code == self.code,
            same_tool: previous.tool == self.tool,
            flips,
        }
    }

    /// The record in `file`, whatever it's of: none if there's none, or it can't be read, as an
    /// unreadable record is no record and the next run writes a new one.
    pub fn read(file: &Path) -> Option<RunRecord> {
        let text = std::fs::read_to_string(file).ok()?;
        let mut fields = BTreeMap::new();
        let mut results = BTreeMap::new();
        for line in text.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            match parts.as_slice() {
                [name, value] if HEADERS.contains(name) => {
                    fields.insert(*name, value.to_string());
                }
                [id, status, caught_by] => {
                    let entry = Entry {
                        status: status_named(status)?,
                        caught_by: (*caught_by != "-").then(|| caught_by.to_string()),
                    };
                    results.insert(id.to_string(), entry);
                }
                _ => return None,
            }
        }
        let field = |name: &str| fields.get(name).cloned().unwrap_or_default();
        Some(RunRecord {
            project: field("project"),
            operators: field("operators"),
            code: fields.get("code")?.clone(),
            tool: field("tool"),
            score: match fields.get("score").map(String::as_str) {
                None | Some("-") => None,
                Some(score) => Some(score.parse().ok()?),
            },
            results,
        })
    }

    /// Writes it to `file`, through a file beside it renamed into place, so a run reading it
    /// meanwhile reads the last record or this one, never half of one.
    pub fn write(&self, file: &Path) -> std::io::Result<()> {
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let partial = file.with_extension(format!("tsv.{}", std::process::id()));
        std::fs::write(&partial, self.lines())?;
        std::fs::rename(&partial, file)
    }

    fn lines(&self) -> String {
        let score = self
            .score
            .map_or("-".to_string(), |score| score.to_string());
        let mut out = format!(
            "project\t{}\noperators\t{}\ncode\t{}\ntool\t{}\nscore\t{score}\n",
            self.project, self.operators, self.code, self.tool
        );
        for (id, entry) in &self.results {
            let caught_by = entry.caught_by.as_deref().unwrap_or("-");
            out.push_str(&format!(
                "{id}\t{}\t{caught_by}\n",
                entry.status.schema_name()
            ));
        }
        out
    }
}

/// The lines before the results, by their first field.
const HEADERS: &[&str] = &["project", "operators", "code", "tool", "score"];

/// A status from the name a record keeps it by, the report schema's: `Killed`.
fn status_named(name: &str) -> Option<Status> {
    [
        Status::Killed,
        Status::Survived,
        Status::Timeout,
        Status::NoCoverage,
        Status::Ignored,
        Status::RuntimeError,
    ]
    .into_iter()
    .find(|status| status.schema_name() == name)
}

/// `/code/shop`: which package a record is of, by its directory.
fn project_of(report: &Report) -> String {
    let project = &report.project;
    // Rebuilt from its parts, so a package at the workspace's root has no `/` at the end.
    project
        .workspace
        .join(&project.package_dir)
        .components()
        .collect::<std::path::PathBuf>()
        .display()
        .to_string()
}

/// The hash of the mutastic that's running, so a rebuilt mutastic counts as a different one even
/// at the same version; its version, where its file can't be read.
pub fn tool() -> String {
    let bytes = std::env::current_exe().and_then(std::fs::read);
    match bytes {
        Ok(bytes) => Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        Err(_) => env!("CARGO_PKG_VERSION").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(code: &str, results: &[(&str, Status, Option<&str>)]) -> RunRecord {
        RunRecord {
            project: "/code/shop".into(),
            operators: "boundary,comparison".into(),
            code: code.into(),
            tool: "t".into(),
            score: Some(50.0),
            results: results
                .iter()
                .map(|(id, status, caught_by)| {
                    let entry = Entry {
                        status: *status,
                        caught_by: caught_by.map(str::to_string),
                    };
                    (id.to_string(), entry)
                })
                .collect(),
        }
    }

    #[test]
    fn a_record_reads_back_as_it_was_written() {
        let kept = record(
            "c",
            &[
                ("m1", Status::Killed, Some("tests::caught")),
                ("m2", Status::Survived, None),
            ],
        );
        let file = std::env::temp_dir().join(format!("mutastic-record-{}.tsv", std::process::id()));
        kept.write(&file).unwrap();
        let read = RunRecord::read(&file);
        std::fs::remove_file(&file).ok();
        assert_eq!(read, Some(kept));
    }

    #[test]
    fn an_unreadable_record_is_none() {
        let file = std::env::temp_dir().join(format!("mutastic-bad-{}.tsv", std::process::id()));
        std::fs::write(&file, "code\tc\nm1\tNotAStatus\t-\n").unwrap();
        assert_eq!(RunRecord::read(&file), None);
        std::fs::write(&file, "score\t1\n").unwrap();
        assert_eq!(RunRecord::read(&file), None, "no code hash");
        std::fs::remove_file(&file).ok();
    }

    #[test]
    fn with_the_same_code_a_flip_names_its_catching_test() {
        let before = record(
            "c",
            &[
                ("m1", Status::Killed, Some("tests::sometimes")),
                ("m2", Status::Survived, None),
                ("m3", Status::Ignored, None),
            ],
        );
        let now = record(
            "c",
            &[
                ("m1", Status::Survived, None),
                ("m2", Status::Killed, Some("tests::sometimes")),
                ("m3", Status::Killed, Some("tests::other")),
            ],
        );
        let compared = now.against(&before);
        assert_eq!(compared.sameness(), "same code");
        assert_eq!(
            compared
                .flips
                .iter()
                .map(|f| f.id.as_str())
                .collect::<Vec<_>>(),
            ["m1", "m2"],
            "a mutant out of the score doesn't flip"
        );
        assert_eq!(
            compared.likely_flaky(),
            BTreeMap::from([(
                "tests::sometimes".to_string(),
                vec!["m1".into(), "m2".into()]
            )])
        );
    }

    #[test]
    fn with_the_code_or_mutastic_changed_nothing_is_named() {
        let before = record("c", &[("m1", Status::Killed, Some("tests::t"))]);
        let changed = record("d", &[("m1", Status::Survived, None)]);
        assert_eq!(changed.against(&before).sameness(), "code changed");
        assert!(changed.against(&before).likely_flaky().is_empty());
        let mut rebuilt = record("c", &[("m1", Status::Survived, None)]);
        rebuilt.tool = "u".into();
        assert_eq!(rebuilt.against(&before).sameness(), "mutastic changed");
        assert!(rebuilt.against(&before).likely_flaky().is_empty());
    }

    #[test]
    fn only_the_same_package_under_the_same_operators_compares() {
        let base = record("c", &[]);
        let mut other_operators = base.clone();
        other_operators.operators = "boundary".into();
        let mut elsewhere = base.clone();
        elsewhere.project = "/code/other".into();
        assert!(base.compares_with(&base));
        assert!(!base.compares_with(&other_operators));
        assert!(!base.compares_with(&elsewhere));
    }
}
