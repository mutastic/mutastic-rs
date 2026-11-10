//! A flaky test, made so on purpose and the same way every run: it fails the first time it runs
//! in each of mutastic's runs, and passes after, as a test that fails now and then does when
//! mutastic runs it again. The test that runs mutastic names each run in `RAFFLE_RUN`, which
//! mutastic's test processes inherit.

use raffle::{draw, ticket};

#[test]
fn tickets_are_numbered_in_tens() {
    assert_eq!(ticket(0), 1);
    assert_eq!(ticket(2), 21);
}

#[test]
fn a_draw_is_a_place_in_the_raffle() {
    let place = draw(3);
    assert!((1..=3).contains(&place));
    first_try_fails();
}

/// Panics the first time it's called in a run, and passes after: a marker in the temporary
/// directory, named for the run, says it already failed once. A run's own name, rather than the
/// parent process's id, works the same on every platform.
fn first_try_fails() {
    let run = std::env::var("RAFFLE_RUN").unwrap_or_default();
    let marker = std::env::temp_dir().join(format!("mutastic-flaky-fixture-{run}"));
    if std::fs::remove_file(&marker).is_err() {
        std::fs::write(&marker, "failed once").unwrap();
        panic!("failed on its first try");
    }
}
