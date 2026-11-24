//! How long a mutant's tests may run before it's timed out, from each test's own time.
//!
//! A test's own time is how long it takes run plainly, with no mutant in and nothing recorded. The
//! coverage pass records every site a test reaches and the whole numbers each function is called
//! with, and a test that makes millions of calls can take minutes there for seconds plainly. So a
//! test whose recording made 100,000 calls on a thread (the switch's line `b`), and took
//! [`PLAIN_FROM`] or more, runs again plainly for its own time. Any other is counted at its recorded
//! time: one that sleeps or waits takes as long either way, and one that took less is at most a
//! quarter of a second over.
//!
//! Each test may take five times its own time and a second ([`of_test`]). A binary's tests run
//! side by side in one process, so its limit isn't their sum: libtest starts the next test as one
//! finishes, so while each test keeps to its own limit, the run ends within the longest limit and
//! the rest shared out between the tests that run at once ([`Limits::group`]). One run is never
//! stopped before [`FLOOR`]. Above them all is the ceiling: what one mutant's tests may take in
//! all, ten minutes or twenty times the whole suite's own time, or `timeout-ceiling:` in
//! `mutastic.yaml`. A mutant stopped there is timed out with a note that says so.

use std::time::Duration;

/// No run of a mutant's tests is stopped sooner.
pub const FLOOR: Duration = Duration::from_secs(10);

/// A test whose recorded run took less isn't run again plainly, however many calls it recorded: at
/// five times that and a second, its limit is at most a quarter of a second over.
pub const PLAIN_FROM: Duration = Duration::from_millis(50);

/// The least the ceiling is, unless `timeout-ceiling:` sets it.
const CEILING_LEAST: Duration = Duration::from_secs(600);

/// The ceiling is at least this many times the whole suite's own time.
const CEILING_TIMES: u32 = 20;

/// One test's limit: five times its own time, and a second.
pub fn of_test(own: Duration) -> Duration {
    own * 5 + Duration::from_secs(1)
}

/// The limits of one run's mutants: how many of a binary's tests run at once in a worker's share of
/// the machine, and the ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    side_by_side: u32,
    /// The most one mutant's tests may take in all.
    pub ceiling: Duration,
}

impl Limits {
    /// For `workers` running mutants at once, with the ceiling `mutastic.yaml` sets, or one from
    /// `suite`, the sum of every test's own time.
    pub fn new(workers: usize, set: Option<Duration>, suite: Duration) -> Limits {
        let cores = std::thread::available_parallelism().map_or(1, usize::from);
        Limits {
            side_by_side: side_by_side(test_threads(), cores, workers),
            ceiling: set.unwrap_or_else(|| ceiling(suite)),
        }
    }

    /// How long one run of a binary's tests may take, their own times given: with
    /// `one_at_a_time`, as `--test-threads=1` runs them, their limits added up.
    pub fn group(&self, own: impl IntoIterator<Item = Duration>, one_at_a_time: bool) -> Duration {
        let side_by_side = if one_at_a_time { 1 } else { self.side_by_side };
        group(own, side_by_side)
    }
}

/// The ceiling for a suite whose tests' own times add up to `suite`.
fn ceiling(suite: Duration) -> Duration {
    CEILING_LEAST.max(suite * CEILING_TIMES)
}

/// How many tests a binary runs at once in one worker's share of the machine: libtest's threads,
/// and no more than the cores shared between the workers, at least one.
fn side_by_side(threads: usize, cores: usize, workers: usize) -> u32 {
    let share = cores / workers.max(1);
    u32::try_from(threads.min(share).max(1)).unwrap_or(u32::MAX)
}

/// How many threads libtest runs a binary's tests on: `RUST_TEST_THREADS`, which the tests'
/// processes inherit, else the cores.
fn test_threads() -> usize {
    std::env::var("RUST_TEST_THREADS")
        .ok()
        .and_then(|threads| threads.parse::<usize>().ok())
        .filter(|&threads| threads > 0)
        .or_else(|| std::thread::available_parallelism().ok().map(usize::from))
        .unwrap_or(1)
}

/// The limit of tests run `side_by_side` at a time: while each keeps to its own limit, a run that
/// starts the next as one finishes ends within the longest limit and the rest shared out, so this
/// is never shorter than an honest run takes. One at a time, it's their sum.
fn group(own: impl IntoIterator<Item = Duration>, side_by_side: u32) -> Duration {
    let limits: Vec<Duration> = own.into_iter().map(of_test).collect();
    let longest = limits.iter().copied().max().unwrap_or_default();
    let rest = limits.iter().sum::<Duration>() - longest;
    (rest / side_by_side.max(1) + longest).max(FLOOR)
}

/// `timeout-ceiling:`'s value, a whole number with its unit: `90s`, `10m` or `2h`.
pub fn parse_ceiling(text: &str) -> Result<Duration, String> {
    let unit = |suffix: char, seconds: u64| {
        text.strip_suffix(suffix)
            .filter(|number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|number| number.parse::<u64>().ok())
            .and_then(|number| number.checked_mul(seconds))
    };
    match unit('s', 1).or_else(|| unit('m', 60)).or_else(|| unit('h', 3600)) {
        Some(seconds) if seconds > 0 => Ok(Duration::from_secs(seconds)),
        _ => Err(format!(
            "`timeout-ceiling:` takes a time above 0 with its unit, such as 10m, 90s or 2h, not {text}"
        )),
    }
}

/// What a mutant stopped at the ceiling has for its reason, kind first.
pub fn ceiling_note(ceiling: Duration) -> String {
    format!(
        "{CEILING}ran for the most a mutant's tests may, {} (timeout-ceiling:)",
        crate::amounts::duration(ceiling)
    )
}

/// What starts a ceiling's note.
const CEILING: &str = "ceiling: ";

/// What a timed-out mutant's reason says of its tests when the ceiling stopped them, `ran for the
/// most a mutant's tests may, 10m00s (timeout-ceiling:)`; none when they stopped finishing in time.
pub fn at_ceiling(reason: Option<&str>) -> Option<&str> {
    reason.and_then(|reason| reason.strip_prefix(CEILING))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    /// A test's limit is its own time five times over and a second, so a test of a millisecond
    /// still has a second.
    #[test]
    fn a_tests_limit_is_five_times_its_own_and_a_second() {
        assert_eq!(of_test(Duration::ZERO), secs(1));
        assert_eq!(of_test(secs(2)), secs(11));
        assert_eq!(
            of_test(Duration::from_millis(10)),
            Duration::from_millis(1050)
        );
    }

    /// One at a time, a binary's limit is its tests' limits added up; side by side, the longest
    /// and the rest shared out; never under ten seconds.
    #[test]
    fn side_by_side_the_limits_are_shared_out_not_added_up() {
        let twenty_slow =
            || std::iter::once(Duration::ZERO).chain(std::iter::repeat_n(secs(1), 20));
        // Twenty tests of a second and one of none: 20 × 6s + 1s.
        assert_eq!(group(twenty_slow(), 1), secs(121));
        // On eight threads: the longest, 6s, and the other 115s shared out.
        assert_eq!(group(twenty_slow(), 8), Duration::from_millis(20_375));
        // On many, the floor.
        assert_eq!(group(twenty_slow(), 64), FLOOR);
        assert_eq!(group([Duration::ZERO], 8), FLOOR);
        assert_eq!(group([secs(4)], 8), secs(21));
    }

    /// A worker's share is the cores between the workers, at most libtest's threads, at least one.
    #[test]
    fn a_workers_share_is_its_cores_and_libtests_threads() {
        assert_eq!(side_by_side(8, 8, 1), 8);
        assert_eq!(side_by_side(8, 8, 4), 2);
        assert_eq!(side_by_side(2, 8, 1), 2);
        assert_eq!(side_by_side(8, 4, 8), 1);
        let limits = Limits {
            side_by_side: 4,
            ceiling: secs(600),
        };
        let own = [secs(2), secs(2), secs(2)];
        assert_eq!(limits.group(own, false), Duration::from_millis(16_500));
        assert_eq!(limits.group(own, true), secs(33));
    }

    /// The ceiling is ten minutes, or twenty times the suite's own time when that's longer.
    #[test]
    fn the_ceiling_is_ten_minutes_or_twenty_suites() {
        assert_eq!(ceiling(secs(3)), secs(600));
        assert_eq!(ceiling(secs(60)), secs(1200));
        assert_eq!(Limits::new(1, Some(secs(5)), secs(60)).ceiling, secs(5));
    }

    #[test]
    fn a_ceiling_is_a_whole_number_with_its_unit() {
        assert_eq!(parse_ceiling("90s"), Ok(secs(90)));
        assert_eq!(parse_ceiling("10m"), Ok(secs(600)));
        assert_eq!(parse_ceiling("2h"), Ok(secs(7200)));
        for wrong in ["10", "0m", "m", "1.5m", "-1s", "10 m", "ten minutes", ""] {
            let refused = parse_ceiling(wrong).unwrap_err();
            assert!(
                refused.contains("such as 10m, 90s or 2h"),
                "{wrong}: {refused}"
            );
        }
    }
}
