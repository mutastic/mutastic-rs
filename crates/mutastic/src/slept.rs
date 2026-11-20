//! How long the machine slept during a run. A run's Duration counts the time the machine was
//! awake, as `Instant` does: on macOS it stops while the Mac sleeps, so a run the Mac slept through
//! for four minutes said 15.4s. The wall clock keeps going, so the two read over the same stretch
//! tell the sleep apart, and the summary and `run-finished` say it when it's worth saying, as the
//! Java edition does.

use std::time::{Duration, SystemTime};

/// The least sleep worth saying. Under it, the wall clock's own corrections and the moment between
/// reading one clock and the other are as likely the cause.
const AT_LEAST: Duration = Duration::from_secs(5);

/// And at least a tenth of the clock's time: with less, Duration is still about what the clock
/// says, and the line would only be noise.
const TENTHS: u32 = 10;

/// How long the machine slept from `started` on the wall clock, with `awake` gone by since on the
/// awake clock, when it's worth saying; else zero.
pub fn since(started: SystemTime, awake: Duration) -> Duration {
    noticed(started.elapsed().unwrap_or_default(), awake)
}

/// The clock's time less the awake time, when it's at least five seconds and a tenth of the
/// clock's time; else zero. A clock set back gives zero.
pub fn noticed(clock: Duration, awake: Duration) -> Duration {
    let slept = clock.saturating_sub(awake);
    if slept >= AT_LEAST && slept * TENTHS >= clock {
        slept
    } else {
        Duration::ZERO
    }
}

/// The summary's line: `The machine slept for 3m53s during the run, which Duration leaves out.`
pub fn line(slept: Duration) -> String {
    format!(
        "The machine slept for {} during the run, which Duration leaves out.",
        crate::report::duration(slept)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The clocks are given, so nothing sleeps: a run the Mac slept through on 16 Nov took 4m05s
    /// by the clock and 15.4s awake.
    #[test]
    fn the_clock_less_the_time_awake_is_the_sleep() {
        let slept = noticed(Duration::from_secs(245), Duration::from_millis(15_400));

        assert_eq!(slept, Duration::from_millis(229_600));
        assert_eq!(
            line(slept),
            "The machine slept for 3m49s during the run, which Duration leaves out."
        );
    }

    #[test]
    fn five_seconds_is_the_least_worth_saying() {
        assert_eq!(
            noticed(Duration::from_secs(20), Duration::from_secs(15)),
            Duration::from_secs(5)
        );
        assert_eq!(
            noticed(Duration::from_secs(20), Duration::from_millis(15_001)),
            Duration::ZERO
        );
    }

    /// An hour's run that slept for a minute still reads about right.
    #[test]
    fn a_tenth_of_the_clocks_time_is_the_least_worth_saying() {
        assert_eq!(
            noticed(Duration::from_secs(100), Duration::from_secs(90)),
            Duration::from_secs(10)
        );
        assert_eq!(
            noticed(Duration::from_secs(100), Duration::from_millis(90_001)),
            Duration::ZERO
        );
        assert_eq!(
            noticed(Duration::from_secs(3600), Duration::from_secs(3540)),
            Duration::ZERO
        );
    }

    #[test]
    fn a_clock_set_back_is_no_sleep() {
        let ahead = SystemTime::now() + Duration::from_secs(60);

        assert_eq!(since(ahead, Duration::from_secs(30)), Duration::ZERO);
    }

    #[test]
    fn a_run_just_started_has_not_slept() {
        assert_eq!(since(SystemTime::now(), Duration::ZERO), Duration::ZERO);
    }
}
