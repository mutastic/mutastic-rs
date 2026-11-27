//! The tests watched since a run named them likely flaky (decision 27), as `mutastic-core`'s
//! [`watch`](mutastic_core::watch) keeps them for every edition: here, what a Rust run makes of
//! them.

pub use mutastic_core::watch::*;

use std::collections::BTreeMap;

use crate::engine::{Report, Status};

/// The watch after a full run, and what became of each test it watched. A test the run named
/// starts again at 0 runs held. One it didn't that caught a mutant held, as each catch it made
/// repeated [`CONFIRMATIONS`] times, and adds 1; at [`NEEDED`] it's forgotten. One that caught
/// nothing stays as it was.
pub fn after(
    before: &BTreeMap<String, usize>,
    report: &Report,
) -> (BTreeMap<String, usize>, Vec<Watched>) {
    let mut next = before.clone();
    let mut watched = Vec::new();
    for (test, &held) in before {
        if named(report, test) || left_out(report, test) {
            continue;
        }
        if !caught_something(report, test) {
            watched.push(Watched {
                test: test.clone(),
                held,
                released: false,
            });
            continue;
        }
        let now = held + 1;
        let released = now >= NEEDED;
        if released {
            next.remove(test);
        } else {
            next.insert(test.clone(), now);
        }
        watched.push(Watched {
            test: test.clone(),
            held: now,
            released,
        });
    }
    for named in &report.likely_flaky {
        next.insert(named.test.clone(), 0);
    }
    (next, watched)
}

/// What a run that doesn't change the watch says of it: each test it watched, as it was. A test the
/// run named is said in its likely-flaky line instead.
pub fn unchanged(watch: &BTreeMap<String, usize>, report: &Report) -> Vec<Watched> {
    watch
        .iter()
        .filter(|(test, _)| !named(report, test) && !left_out(report, test))
        .map(|(test, &held)| Watched {
            test: test.clone(),
            held,
            released: false,
        })
        .collect()
}

/// Whether the run left `test` out (`--exclude-tests`, `exclude-tests:`): it ran none of its
/// catches, so the watch says nothing of it and keeps it as it was.
fn left_out(report: &Report, test: &str) -> bool {
    crate::engine::leaves_out(&report.left_out, test)
}

fn named(report: &Report, test: &str) -> bool {
    report.likely_flaky.iter().any(|named| named.test == test)
}

fn caught_something(report: &Report, test: &str) -> bool {
    report
        .results
        .iter()
        .any(|result| result.status == Status::Killed && result.killed_by.as_deref() == Some(test))
}
