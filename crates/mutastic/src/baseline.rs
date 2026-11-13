//! `.mutastic/baseline.yaml`: the mutants a package missed when its baseline was taken, by id,
//! written by `mutastic baseline` and committed. A miss that isn't in it is new: `gate` holds only
//! new misses, `next` offers them first, and `run --fail-on-new` fails only on them. It shrinks
//! with `baseline --prune` as tests catch its mutants, and is never rewritten to take in new
//! misses, or it would hide them. The same file, in the same shape, as the Java edition's:
//!
//! ```yaml
//! baseline:
//!   - id: mb2f8ae57
//!     at: 'pricing::Pricing::discount, line 26'
//!     change: '`>=` becomes `>`'
//! ```
//!
//! Ids alone decide: a mutant's id is a hash of what it is, without its line, so it stays the same
//! while code around it moves. `at` and `change` are for whoever reads a diff of it. One entry an
//! id, in order of function and then id, so a line moving doesn't reorder it.

use std::collections::BTreeSet;
use std::path::Path;

use crate::engine::{MutantResult, Report, Status};

/// The file, in the package's directory.
pub const FILE: &str = ".mutastic/baseline.yaml";

const HEADER: &[&str] = &[
    "# The mutants this project missed when its baseline was taken, written by `mutastic baseline`.",
    "# Commit it. A miss that isn't here is new: the gate holds only new misses. As tests",
    "# catch these, `mutastic baseline --prune` drops them; it never takes in new misses.",
    "baseline:",
];

/// One mutant, with where it is and what it changes, for the reader.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub at: String,
    pub change: String,
}

/// The id an entry's first line names: `  - id: mb2f8ae57`.
fn id_on(line: &str) -> Option<&str> {
    let id = line.trim().strip_prefix("- id:")?.trim();
    let hex = id.strip_prefix('m')?;
    (hex.len() == 8
        && hex
            .bytes()
            .all(|digit| matches!(digit, b'0'..=b'9' | b'a'..=b'f')))
    .then_some(id)
}

/// The ids in a baseline's text.
pub fn ids_in(text: &str) -> BTreeSet<String> {
    text.lines().filter_map(id_on).map(str::to_string).collect()
}

/// The ids in the package's baseline; none when it has none.
pub fn ids(dir: &Path) -> BTreeSet<String> {
    std::fs::read_to_string(dir.join(FILE))
        .map(|text| ids_in(&text))
        .unwrap_or_default()
}

/// The baseline as it was at a commit, as git has it: what a gated task started from. None when
/// it had none.
pub fn ids_at(dir: &Path, commit: &str) -> BTreeSet<String> {
    crate::since::git(dir, &["show", &format!("{commit}:./{FILE}")])
        .map(|text| ids_in(&text))
        .unwrap_or_default()
}

pub fn exists(dir: &Path) -> bool {
    dir.join(FILE).is_file()
}

/// A mutant no test catches, missed or reached by none: what a baseline holds.
pub fn uncaught(result: &MutantResult) -> bool {
    matches!(result.status, Status::Survived | Status::NoCoverage)
}

/// The ids of a run's mutants no test catches.
pub fn uncaught_ids(report: &Report) -> BTreeSet<String> {
    report
        .results
        .iter()
        .filter(|result| uncaught(result))
        .map(|result| report.found.mutants[result.mutant].id.clone())
        .collect()
}

/// An entry for each mutant no test catches, in the file's order: by function, then id.
pub fn entries(report: &Report) -> Vec<Entry> {
    let mut entries: Vec<(String, Entry)> = report
        .results
        .iter()
        .filter(|result| uncaught(result))
        .map(|result| {
            let mutant = &report.found.mutants[result.mutant];
            let function = &report.found.functions[mutant.function];
            let entry = Entry {
                id: mutant.id.clone(),
                at: format!("{}, line {}", function.path, mutant.line),
                change: crate::words::advise(mutant, &function.name, &Default::default()).change,
            };
            (function.path.clone(), entry)
        })
        .collect();
    entries.sort_by(|(a, first), (b, second)| (a, &first.id).cmp(&(b, &second.id)));
    entries.into_iter().map(|(_, entry)| entry).collect()
}

/// Writes the entries, replacing the file.
pub fn write(dir: &Path, entries: &[Entry]) -> Result<(), String> {
    let mut lines: Vec<String> = HEADER.iter().map(|line| line.to_string()).collect();
    for entry in entries {
        lines.push(format!("  - id: {}", entry.id));
        lines.push(format!("    at: {}", quoted(&entry.at)));
        lines.push(format!("    change: {}", quoted(&entry.change)));
    }
    write_lines(&dir.join(FILE), &lines)
}

/// Drops the entries whose ids are in `caught`, each whole, and keeps the rest as they were: how
/// many it dropped.
pub fn prune(dir: &Path, caught: &BTreeSet<String>) -> Result<usize, String> {
    let file = dir.join(FILE);
    let text = std::fs::read_to_string(&file)
        .map_err(|error| format!("couldn't read {}: {error}", file.display()))?;
    let mut kept = Vec::new();
    let mut dropped = 0;
    let mut dropping = false;
    for line in text.lines() {
        if let Some(id) = id_on(line) {
            dropping = caught.contains(id);
            dropped += usize::from(dropping);
        } else if !line.starts_with("    ") {
            dropping = false;
        }
        if !dropping {
            kept.push(line.to_string());
        }
    }
    write_lines(&file, &kept)?;
    Ok(dropped)
}

/// The baseline's mutants a run doesn't find uncaught: caught now, skipped, or gone with their
/// code.
pub fn caught_now(report: &Report, baseline: &BTreeSet<String>) -> BTreeSet<String> {
    baseline
        .difference(&uncaught_ids(report))
        .cloned()
        .collect()
}

/// In single quotes, as YAML has them: a quote inside is doubled.
fn quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn write_lines(file: &Path, lines: &[String]) -> Result<(), String> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(file, lines.join("\n") + "\n")
        .map_err(|error| format!("couldn't write {}: {error}", file.display()))
}

/// `run --fail-on-new`: a run judged on the misses a committed baseline doesn't hold, not on its
/// score, for a package whose score can't reach the threshold yet. A mutant no test catches,
/// missed or reached by none, is what a baseline holds; one the baseline doesn't hold is new, and
/// the run passes while there are none. As the Java edition's `FailOnNew`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FailOnNew {
    /// The ids in the package's baseline.
    pub baseline: BTreeSet<String>,
}

impl FailOnNew {
    /// The ids of the mutants no test catches that the baseline doesn't hold, in the run's order.
    pub fn new_misses(&self, report: &Report) -> Vec<String> {
        report
            .results
            .iter()
            .filter(|result| uncaught(result))
            .map(|result| report.found.mutants[result.mutant].id.clone())
            .filter(|id| !self.baseline.contains(id))
            .collect()
    }

    /// How many of the mutants no test catches the baseline holds: missed before, and still missed.
    pub fn in_baseline(&self, report: &Report) -> usize {
        report
            .results
            .iter()
            .filter(|result| uncaught(result))
            .filter(|result| {
                self.baseline
                    .contains(&report.found.mutants[result.mutant].id)
            })
            .count()
    }

    /// `PASS · no new misses, 12 in the baseline`, or `FAIL · 2 new misses, not in the baseline (12
    /// in it): m1, m2`: the verdict as a line that leads with it says it.
    pub fn verdict(&self, report: &Report) -> String {
        let added = self.new_misses(report);
        let known = self.in_baseline(report);
        if added.is_empty() {
            return format!("PASS · no new misses, {known} in the baseline");
        }
        format!(
            "FAIL · {}: {}",
            self.new_misses_counted(added.len(), known),
            added.join(", ")
        )
    }

    /// `2 new misses, not in the baseline (12 in it)`.
    pub fn new_misses_counted(&self, added: usize, known: usize) -> String {
        format!(
            "{added} new {}, not in the baseline ({known} in it)",
            if added == 1 { "miss" } else { "misses" }
        )
    }

    /// No mutant no test catches is new.
    pub fn passes(&self, report: &Report) -> bool {
        self.new_misses(report).is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(label: &str) -> std::path::PathBuf {
        let folder =
            std::env::temp_dir().join(format!("mutastic-baseline-{label}-{}", std::process::id()));
        std::fs::remove_dir_all(&folder).ok();
        folder
    }

    #[test]
    fn entries_are_written_as_the_java_edition_writes_them_and_pruned_whole() {
        let dir = folder("write");
        let entry = |id: &str, at: &str| Entry {
            id: id.into(),
            at: at.into(),
            change: "`>=` becomes `>`".into(),
        };
        write(
            &dir,
            &[
                entry("mb2f8ae57", "pricing::Pricing::discount, line 26"),
                entry("m7d92063c", "pricing::Pricing::shipping, line 34"),
            ],
        )
        .unwrap();
        let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert_eq!(
            text,
            "# The mutants this project missed when its baseline was taken, written by `mutastic baseline`.\n\
             # Commit it. A miss that isn't here is new: the gate holds only new misses. As tests\n\
             # catch these, `mutastic baseline --prune` drops them; it never takes in new misses.\n\
             baseline:\n  \
             - id: mb2f8ae57\n    at: 'pricing::Pricing::discount, line 26'\n    change: '`>=` becomes `>`'\n  \
             - id: m7d92063c\n    at: 'pricing::Pricing::shipping, line 34'\n    change: '`>=` becomes `>`'\n"
        );
        assert_eq!(
            ids(&dir),
            BTreeSet::from(["mb2f8ae57".to_string(), "m7d92063c".to_string()])
        );
        let dropped = prune(&dir, &BTreeSet::from(["mb2f8ae57".to_string()])).unwrap();
        assert_eq!(dropped, 1);
        let pruned = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(
            pruned.ends_with("baseline:\n  - id: m7d92063c\n    at: 'pricing::Pricing::shipping, line 34'\n    change: '`>=` becomes `>`'\n"),
            "{pruned}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn only_an_id_line_names_an_id() {
        assert_eq!(
            ids_in("baseline:\n  - id: mb2f8ae57\n    at: '- id: m00000000'\n  - id: nothex\n"),
            BTreeSet::from(["mb2f8ae57".to_string()])
        );
        assert!(ids(&folder("none")).is_empty());
    }

    #[test]
    fn the_verdict_names_the_new_misses() {
        let judged = FailOnNew::default();
        assert_eq!(
            judged.new_misses_counted(1, 0),
            "1 new miss, not in the baseline (0 in it)"
        );
        assert_eq!(
            judged.new_misses_counted(2, 12),
            "2 new misses, not in the baseline (12 in it)"
        );
    }
}
