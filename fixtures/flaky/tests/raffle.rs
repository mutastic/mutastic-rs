//! A flaky test, made so on purpose and the same way every run: it fails the first time it runs
//! under each mutastic process, and passes after, as a test that fails now and then does when
//! mutastic runs it again.

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

/// Panics the first time it's called under a given parent process, and passes after: a marker in
/// the temporary directory, named for the process that ran the test, says it already failed once.
fn first_try_fails() {
    let marker = std::env::temp_dir().join(format!(
        "mutastic-flaky-fixture-{}",
        std::os::unix::process::parent_id()
    ));
    if std::fs::remove_file(&marker).is_err() {
        std::fs::write(&marker, "failed once").unwrap();
        panic!("failed on its first try");
    }
}
