//! The false-catch guard, as the Java edition's `FalseCatchGuard`. After every mutant has its
//! result, each test that caught one runs again, once, on the code as it is: on its own, in a
//! process of its own, after the mutants, so what their runs left outside the process (a file) is
//! still there. A test that fails there didn't need a mutant to fail, so its catches weren't the
//! mutants' doing.
//!
//! Its catches don't count: it's left out, as a flaky test is, and each mutant it reached runs
//! again without it, where the other covering tests decide, so no result rests on it. A test those
//! runs newly find catching is checked the same way. A mutant's tests stop at the first failure, so
//! a catch rests on that test alone, and only the tests that caught are checked: a healthy run
//! costs one run of each.
//!
//! The coverage pass already runs every test on its own before any mutant, so what's found here is
//! what the mutants' runs left behind, or what a test needs that only another test in its process
//! sets up and the coverage pass happened to give it.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use crate::engine::{MutantResult, Status};

/// What checking the catches found.
#[derive(Debug, Default)]
pub struct Guarded {
    /// Each mutant that ran again, by its index in `results`: its first result, which its
    /// `mutant-finished` gives, before the `mutant-revised` with the one that replaced it.
    pub first: BTreeMap<usize, MutantResult>,
    /// The tests that caught a mutant and then failed on their own with no mutant in, in the order
    /// found.
    pub failing_alone: Vec<String>,
}

/// How the guard reaches the tests: the engine's runs, behind a seam the tests can stand in for.
pub trait Runs {
    /// How long `test` took in the coverage pass: the fastest are checked first, as a mutant's
    /// tests run.
    fn time_of(&self, test: &str) -> Duration;
    /// Whether `test` fails on its own on the code as it is. Running out of time isn't evidence
    /// that it does, as load can do that, and nor is a process that couldn't start.
    fn fails_alone(&self, test: &str) -> bool;
    /// The mutants at `indexes` of `results` run again with the tests in `left_out` left out of
    /// coverage, so none of them runs again: their new results, in the same order.
    fn run_again(
        &self,
        results: &[MutantResult],
        indexes: &[usize],
        left_out: &BTreeSet<String>,
    ) -> Vec<MutantResult>;
}

/// Checks every catch in `results`, replacing in place each result a test that fails on its own
/// reached.
pub fn check(results: &mut [MutantResult], runs: &impl Runs) -> Guarded {
    let mut guarded = Guarded::default();
    let mut checked = BTreeSet::new();
    let mut left_out = BTreeSet::new();
    loop {
        let to_check = unchecked(results, &checked, runs);
        if to_check.is_empty() {
            break;
        }
        checked.extend(to_check.iter().cloned());
        let failing: BTreeSet<String> = to_check
            .into_iter()
            .filter(|test| runs.fails_alone(test))
            .collect();
        if failing.is_empty() {
            break;
        }
        guarded.failing_alone.extend(failing.iter().cloned());
        left_out.extend(failing.iter().cloned());
        run_again_without(results, &failing, &left_out, runs, &mut guarded.first);
    }
    guarded
}

/// The tests that caught a mutant and haven't been checked, fastest first, then by name. A crash's
/// catch names no test, so there's none to check.
fn unchecked(
    results: &[MutantResult],
    checked: &BTreeSet<String>,
    runs: &impl Runs,
) -> Vec<String> {
    let catchers: BTreeSet<&String> = results
        .iter()
        .filter(|result| result.status == Status::Killed)
        .filter_map(|result| result.killed_by.as_ref())
        .filter(|test| !checked.contains(*test))
        .collect();
    let mut ordered: Vec<(Duration, String)> = catchers
        .into_iter()
        .map(|test| (runs.time_of(test), test.clone()))
        .collect();
    ordered.sort();
    ordered.into_iter().map(|(_, test)| test).collect()
}

/// Each mutant one of `failing` reached, run again without the tests `left_out`, in place in
/// `results`; its first result kept in `first`.
fn run_again_without(
    results: &mut [MutantResult],
    failing: &BTreeSet<String>,
    left_out: &BTreeSet<String>,
    runs: &impl Runs,
    first: &mut BTreeMap<usize, MutantResult>,
) {
    let reached: Vec<usize> = (0..results.len())
        .filter(|&index| reached_by(&results[index], failing))
        .collect();
    let again = runs.run_again(results, &reached, left_out);
    for (index, mut revised) in reached.into_iter().zip(again) {
        let before = &results[index];
        revised.false_catch_by = false_catcher(before, failing).or(before.false_catch_by.clone());
        first.entry(index).or_insert_with(|| before.clone());
        results[index] = revised;
    }
}

/// Whether one of `failing` reached the mutant, so its result may rest on it: a miss as much as a
/// catch, as a mutant that only passes because of what a test leaves behind isn't missed either.
fn reached_by(result: &MutantResult, failing: &BTreeSet<String>) -> bool {
    result.status != Status::Ignored && result.covered_by.iter().any(|t| failing.contains(t))
}

/// The test that caught the mutant, if it's one of `failing`.
fn false_catcher(result: &MutantResult, failing: &BTreeSet<String>) -> Option<String> {
    result
        .killed_by
        .clone()
        .filter(|test| result.status == Status::Killed && failing.contains(test))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Runs that stand in for the engine's: `fail_alone` fail on their own, and a mutant run again
    /// is caught by the first of its covering tests left in that `catches` names, else missed.
    struct Fake {
        fail_alone: BTreeSet<&'static str>,
        catches: BTreeSet<&'static str>,
        checked: RefCell<Vec<String>>,
    }

    impl Runs for Fake {
        fn time_of(&self, test: &str) -> Duration {
            Duration::from_millis(test.len() as u64)
        }

        fn fails_alone(&self, test: &str) -> bool {
            self.checked.borrow_mut().push(test.to_string());
            self.fail_alone.contains(test)
        }

        fn run_again(
            &self,
            results: &[MutantResult],
            indexes: &[usize],
            left_out: &BTreeSet<String>,
        ) -> Vec<MutantResult> {
            indexes
                .iter()
                .map(|&index| {
                    let covering: Vec<String> = results[index]
                        .covered_by
                        .iter()
                        .filter(|test| !left_out.contains(*test))
                        .cloned()
                        .collect();
                    let catcher = covering
                        .iter()
                        .find(|test| self.catches.contains(test.as_str()))
                        .cloned();
                    let status = match (&catcher, covering.is_empty()) {
                        (Some(_), _) => Status::Killed,
                        (None, true) => Status::NoCoverage,
                        (None, false) => Status::Survived,
                    };
                    MutantResult {
                        status,
                        covered_by: covering,
                        killed_by: catcher,
                        ..results[index].clone()
                    }
                })
                .collect()
        }
    }

    fn result(
        mutant: usize,
        status: Status,
        covered_by: &[&str],
        by: Option<&str>,
    ) -> MutantResult {
        MutantResult {
            mutant,
            status,
            reason: None,
            covered_by: covered_by.iter().map(|t| t.to_string()).collect(),
            killed_by: by.map(str::to_string),
            killed_with: by.map(|_| "panic".to_string()),
            false_catch_by: None,
            tests_run: 1,
            took: Duration::ZERO,
            inputs: BTreeMap::new(),
            suggestion: None,
        }
    }

    fn fake(fail_alone: &[&'static str], catches: &[&'static str]) -> Fake {
        Fake {
            fail_alone: fail_alone.iter().copied().collect(),
            catches: catches.iter().copied().collect(),
            checked: RefCell::new(Vec::new()),
        }
    }

    /// When every catch holds, each catching test runs once, fastest first, and nothing changes.
    #[test]
    fn a_healthy_run_checks_each_catcher_once_and_changes_nothing() {
        let mut results = vec![
            result(0, Status::Killed, &["slower", "b"], Some("slower")),
            result(1, Status::Killed, &["b"], Some("b")),
            result(2, Status::Killed, &["b"], Some("b")),
            result(3, Status::Survived, &["c"], None),
        ];
        let runs = fake(&[], &[]);
        let guarded = check(&mut results, &runs);
        assert_eq!(*runs.checked.borrow(), ["b", "slower"]);
        assert!(guarded.failing_alone.is_empty());
        assert!(guarded.first.is_empty());
        assert_eq!(results[0].killed_by.as_deref(), Some("slower"));
    }

    /// A test that fails alone has its catches taken back: each mutant it reached runs again
    /// without it, whether it caught it or not, and names it only where it was the catch.
    #[test]
    fn a_test_that_fails_alone_has_what_it_reached_run_again_without_it() {
        let mut results = vec![
            // Caught only by the broken test; the other covering test misses it.
            result(0, Status::Killed, &["broken", "weak"], Some("broken")),
            // Only the broken test reaches it.
            result(1, Status::Killed, &["broken"], Some("broken")),
            // Missed, and the broken test reached it: its miss may rest on it too.
            result(2, Status::Survived, &["broken", "weak"], None),
            // Caught by a sound test: nothing to revise.
            result(3, Status::Killed, &["sound"], Some("sound")),
            // Skipped: never revised.
            result(4, Status::Ignored, &["broken"], None),
        ];
        let runs = fake(&["broken"], &["sound"]);
        let guarded = check(&mut results, &runs);
        assert_eq!(guarded.failing_alone, ["broken"]);
        assert_eq!(guarded.first.keys().copied().collect::<Vec<_>>(), [0, 1, 2]);
        assert_eq!(guarded.first[&0].status, Status::Killed);
        assert_eq!(results[0].status, Status::Survived);
        assert_eq!(results[0].covered_by, ["weak"]);
        assert_eq!(results[0].false_catch_by.as_deref(), Some("broken"));
        assert_eq!(results[1].status, Status::NoCoverage);
        assert_eq!(results[1].false_catch_by.as_deref(), Some("broken"));
        assert_eq!(results[2].status, Status::Survived);
        assert_eq!(results[2].false_catch_by, None);
        assert_eq!(results[3].killed_by.as_deref(), Some("sound"));
        assert_eq!(results[4].status, Status::Ignored);
    }

    /// A test the runs again newly find catching is checked too, and a mutant whose catch was
    /// false twice names the test whose catch it was last.
    #[test]
    fn a_new_catcher_is_checked_in_turn() {
        let mut results = vec![result(0, Status::Killed, &["aa", "bbb"], Some("aa"))];
        let runs = fake(&["aa", "bbb"], &["bbb"]);
        let guarded = check(&mut results, &runs);
        assert_eq!(guarded.failing_alone, ["aa", "bbb"]);
        assert_eq!(*runs.checked.borrow(), ["aa", "bbb"]);
        assert_eq!(results[0].status, Status::NoCoverage);
        assert_eq!(results[0].false_catch_by.as_deref(), Some("bbb"));
        assert_eq!(guarded.first[&0].killed_by.as_deref(), Some("aa"));
    }
}
