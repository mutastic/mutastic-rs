//! `.mutastic/baseline.yaml`, as `mutastic-core`'s [`baseline`](mutastic_core::baseline) reads
//! and writes it for every edition: here, which of a Rust run's mutants it holds, and
//! `run --fail-on-new`'s verdict on a run.

pub use mutastic_core::baseline::*;

use std::collections::BTreeSet;

use crate::engine::{MutantResult, Report, Status};

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

/// The baseline's mutants a run doesn't find uncaught: caught now, skipped, or gone with their
/// code.
pub fn caught_now(report: &Report, baseline: &BTreeSet<String>) -> BTreeSet<String> {
    baseline
        .difference(&uncaught_ids(report))
        .cloned()
        .collect()
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
