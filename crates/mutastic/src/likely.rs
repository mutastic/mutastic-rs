//! After the mutants and the false-catch guard ([`crate::guard`]), the tests the run names likely
//! flaky, and none of their catches counting (decision 27), as the Java edition's
//! `LikelyFlakyGuard`. A test is named when, with the code, the tests and mutastic unchanged since
//! the last run, a mutant it caught in one of the two runs changed result; when a catch of it
//! didn't repeat as it was confirmed; or when it failed on its own with no mutant in, which the
//! guard has already dealt with.
//!
//! Each test so named is left out, and each mutant it reached, caught or missed, runs again without
//! it, where the other covering tests decide. A catch that doesn't repeat in those runs names its
//! test in turn. So no score, threshold or gate rests on a catch by a test the run itself doesn't
//! trust.

use std::collections::{BTreeMap, BTreeSet};

use crate::engine::MutantResult;
use crate::guard::{Guarded, Runs};
use crate::watch::History;

/// What named a test, in the order the events list them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reason {
    /// With the code unchanged, a mutant it caught changed result since the last run.
    Changed,
    /// A catch of it didn't repeat when confirmed.
    NotRepeated,
    /// Run again after the mutants on its own with no mutant in, it failed.
    FailsAlone,
}

impl Reason {
    /// As the events say it.
    pub fn name(self) -> &'static str {
        match self {
            Reason::Changed => "changed-since-last-run",
            Reason::NotRepeated => "catch-not-repeated",
            Reason::FailsAlone => "fails-alone",
        }
    }

    /// The reason the events name `name`.
    pub fn named(name: &str) -> Option<Reason> {
        [Reason::Changed, Reason::NotRepeated, Reason::FailsAlone]
            .into_iter()
            .find(|reason| reason.name() == name)
    }
}

/// A test the run named likely flaky, and what came of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LikelyFlaky {
    pub test: String,
    /// What named it, each with the ids of the mutants whose results it changed that way.
    pub by_reason: BTreeMap<Reason, BTreeSet<String>>,
    /// The ids of the mutants it reached that ran again without it.
    pub rerun: BTreeSet<String>,
}

impl LikelyFlaky {
    /// Every mutant whose result it changed, whatever the reason.
    pub fn mutants(&self) -> BTreeSet<String> {
        self.by_reason.values().flatten().cloned().collect()
    }

    /// The mutants it changed for `reason`: none when that didn't name it.
    pub fn mutants_for(&self, reason: Reason) -> Vec<String> {
        self.by_reason
            .get(&reason)
            .map(|ids| ids.iter().cloned().collect())
            .unwrap_or_default()
    }
}

/// Names the tests the run can't trust and runs again, in place in `results`, what each reached;
/// each mutant's first result is kept in `guarded.first`. `id` gives a mutant's id by its index in
/// the crate's mutants.
pub fn check(
    results: &mut [MutantResult],
    guarded: &mut Guarded,
    history: &History,
    runs: &impl Runs,
    id: &dyn Fn(usize) -> String,
) -> Vec<LikelyFlaky> {
    let mut named: BTreeMap<String, LikelyFlaky> = BTreeMap::new();
    for test in &guarded.failing_alone {
        let accused = accused(&mut named, test);
        let caught: BTreeSet<String> = results
            .iter()
            .filter(|result| result.false_catch_by.as_deref() == Some(test))
            .map(|result| id(result.mutant))
            .collect();
        accused
            .by_reason
            .entry(Reason::FailsAlone)
            .or_default()
            .extend(caught);
        let rerun = guarded.rerun.get(test).cloned().unwrap_or_default();
        accused
            .rerun
            .extend(rerun.into_iter().map(|at| id(results[at].mutant)));
    }
    for (test, mutants) in changed_since_last_run(results, history, id) {
        accused(&mut named, &test)
            .by_reason
            .entry(Reason::Changed)
            .or_default()
            .extend(mutants);
    }
    let mut left_out = guarded.left_out.clone();
    let mut handled: BTreeSet<String> = guarded.failing_alone.iter().cloned().collect();
    loop {
        for result in results.iter() {
            if let Some(test) = &result.unconfirmed {
                accused(&mut named, test)
                    .by_reason
                    .entry(Reason::NotRepeated)
                    .or_default()
                    .insert(id(result.mutant));
            }
        }
        let to_drop: BTreeSet<String> = named
            .keys()
            .filter(|test| !handled.contains(*test))
            .cloned()
            .collect();
        if to_drop.is_empty() {
            break;
        }
        handled.extend(to_drop.iter().cloned());
        left_out.extend(to_drop.iter().cloned());
        let rerun = crate::guard::run_again_without(
            results,
            &to_drop,
            &left_out,
            runs,
            &mut guarded.first,
            &|revised, test| revised.flaky_catch_by = Some(test.to_string()),
        );
        for (test, indexes) in rerun {
            let ids: Vec<String> = indexes.iter().map(|&at| id(results[at].mutant)).collect();
            accused(&mut named, &test).rerun.extend(ids);
        }
    }
    named.into_values().collect()
}

fn accused<'a>(named: &'a mut BTreeMap<String, LikelyFlaky>, test: &str) -> &'a mut LikelyFlaky {
    named
        .entry(test.to_string())
        .or_insert_with(|| LikelyFlaky {
            test: test.to_string(),
            by_reason: BTreeMap::new(),
            rerun: BTreeSet::new(),
        })
}

/// Each test that caught a mutant whose result changed since the last run, in whichever run it
/// caught it, with those mutants: none when anything has changed since, or there's no last run.
fn changed_since_last_run(
    results: &[MutantResult],
    history: &History,
    id: &dyn Fn(usize) -> String,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut by_test: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for result in results {
        let mutant = id(result.mutant);
        let Some(before) = history.same_code.get(&mutant) else {
            continue;
        };
        if !crate::record::flipped(before.status, result.status) {
            continue;
        }
        let catcher = if result.status.detected() {
            result.killed_by.clone()
        } else {
            before.caught_by.clone()
        };
        if let Some(test) = catcher {
            by_test.entry(test).or_default().insert(mutant);
        }
    }
    by_test
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Status;
    use crate::record::Entry;
    use std::time::Duration;

    /// Runs that stand in for the engine's: a mutant run again is caught by the first of its
    /// covering tests left in that `catches` names, else missed.
    struct Fake {
        catches: Vec<&'static str>,
    }

    impl Runs for Fake {
        fn time_of(&self, _: &str) -> Duration {
            Duration::ZERO
        }

        fn fails_alone(&self, _: &str) -> bool {
            false
        }

        fn run_again(
            &self,
            results: &[MutantResult],
            indexes: &[usize],
            left_out: &BTreeSet<String>,
        ) -> Vec<MutantResult> {
            indexes
                .iter()
                .map(|&at| {
                    let mut again = results[at].clone();
                    again.unconfirmed = None;
                    let catcher = again
                        .covered_by
                        .iter()
                        .find(|t| !left_out.contains(*t) && self.catches.contains(&t.as_str()))
                        .cloned();
                    again.status = if catcher.is_some() {
                        Status::Killed
                    } else {
                        Status::Survived
                    };
                    again.killed_by = catcher;
                    again
                })
                .collect()
        }
    }

    fn result(mutant: usize, status: Status, by: Option<&str>, covered: &[&str]) -> MutantResult {
        MutantResult {
            mutant,
            status,
            reason: None,
            covered_by: covered.iter().map(|t| t.to_string()).collect(),
            killed_by: by.map(str::to_string),
            killed_with: None,
            killed_input: None,
            killed_by_all: Vec::new(),
            unconfirmed: None,
            failure_origin: None,
            false_catch_by: None,
            flaky_catch_by: None,
            tests_run: 1,
            took: Duration::ZERO,
            inputs: BTreeMap::new(),
            suggestion: None,
            by_assertion: false,
        }
    }

    fn id(mutant: usize) -> String {
        format!("m{mutant}")
    }

    #[test]
    fn a_test_whose_catch_changed_since_the_last_run_has_what_it_reached_run_again_without_it() {
        let mut results = vec![
            result(0, Status::Killed, Some("lucky"), &["lucky", "steady"]),
            result(1, Status::Killed, Some("lucky"), &["lucky"]),
            result(2, Status::Killed, Some("steady"), &["steady"]),
        ];
        let history = History {
            same_code: BTreeMap::from([(
                "m1".to_string(),
                Entry {
                    status: Status::Survived,
                    caught_by: None,
                },
            )]),
            watched: BTreeSet::new(),
        };
        let mut guarded = Guarded::default();

        let named = check(
            &mut results,
            &mut guarded,
            &history,
            &Fake {
                catches: vec!["steady"],
            },
            &id,
        );

        assert_eq!(named.len(), 1);
        assert_eq!(named[0].test, "lucky");
        assert_eq!(named[0].mutants_for(Reason::Changed), vec!["m1"]);
        assert_eq!(
            named[0].rerun,
            BTreeSet::from(["m0".to_string(), "m1".to_string()])
        );
        assert_eq!(results[0].killed_by.as_deref(), Some("steady"));
        assert_eq!(results[0].flaky_catch_by.as_deref(), Some("lucky"));
        assert_eq!(results[1].status, Status::Survived);
        assert_eq!(results[1].flaky_catch_by.as_deref(), Some("lucky"));
        assert_eq!(results[2].flaky_catch_by, None);
        assert_eq!(guarded.first.len(), 2);
    }

    #[test]
    fn a_catch_that_didnt_repeat_names_its_test_and_its_other_catches_dont_count() {
        let mut first = result(0, Status::Survived, None, &["lucky"]);
        first.unconfirmed = Some("lucky".into());
        let mut results = vec![first, result(1, Status::Killed, Some("lucky"), &["lucky"])];
        let mut guarded = Guarded::default();

        let named = check(
            &mut results,
            &mut guarded,
            &History::default(),
            &Fake { catches: vec![] },
            &id,
        );

        assert_eq!(named[0].mutants_for(Reason::NotRepeated), vec!["m0"]);
        assert_eq!(results[1].status, Status::Survived);
        assert_eq!(results[1].flaky_catch_by.as_deref(), Some("lucky"));
    }

    #[test]
    fn with_nothing_named_nothing_runs_again() {
        let mut results = vec![result(0, Status::Killed, Some("steady"), &["steady"])];
        let mut guarded = Guarded::default();

        let named = check(
            &mut results,
            &mut guarded,
            &History::default(),
            &Fake { catches: vec![] },
            &id,
        );

        assert!(named.is_empty());
        assert!(guarded.first.is_empty());
        assert_eq!(results[0].status, Status::Killed);
    }
}
