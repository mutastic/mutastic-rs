//! The time a hooked `gate check` has, and what it spends it on, as the Java edition's
//! `GateBudget`.
//!
//! The agents' plugins run `gate check` from a Stop hook that's stopped after ten minutes, and a
//! hook stopped gates nothing. So a hooked check waits for the count of the tests the task started
//! with ([`crate::gate_count`]) only as long as its budget leaves once its own run, which comes
//! after, is set aside: as long as the last check's own run took. A count not done by then is left
//! for the source's test attributes, and a pass says so. When the last check's own run took longer
//! than the whole budget, a hooked check hands over before it runs, saying so, rather than be
//! stopped with nothing said.

use std::path::Path;
use std::time::Duration;

use crate::failure::{Code, Failure};

/// Seconds a hooked check may take: what the hook allows.
pub const BUDGET: &str = "MUTASTIC_GATE_BUDGET";

/// Seconds a check waits for the count, at most.
pub const COUNT_WAIT: &str = "MUTASTIC_GATE_COUNT_WAIT";

/// What the plugins' Stop hook allows a check: `"timeout": 600`.
pub const TOTAL: Duration = Duration::from_secs(600);

/// The longest a check waits for the count, unless [`COUNT_WAIT`] says otherwise: within a hook's
/// ten minutes, with the check's own run still to come, as the Java edition waits.
pub const WAIT: Duration = Duration::from_secs(240);

/// The last check's own run, in the gate's directory: kept from task to task, as a task's first
/// check is often its only one.
const RECORD: &str = "gate-check.properties";

/// What a hooked check may spend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// What a hooked check may take, from [`BUDGET`].
    pub total: Duration,
    /// The longest a check waits for the count, from [`COUNT_WAIT`].
    pub count_wait: Duration,
    /// How long the last check's own run took, its wait for the count left out; none before a
    /// check has run the tests here.
    pub last_run: Option<Duration>,
}

impl Budget {
    /// The budget `variable` sets, with the last run kept in `work`. A value that isn't a whole
    /// number of seconds is refused (decision 5), as is a budget of none.
    pub fn of(variable: impl Fn(&str) -> Option<String>, work: &Path) -> Result<Budget, Failure> {
        Ok(Budget {
            total: seconds(BUDGET, variable(BUDGET).as_deref())?.unwrap_or(TOTAL),
            count_wait: seconds(COUNT_WAIT, variable(COUNT_WAIT).as_deref())?.unwrap_or(WAIT),
            last_run: last_run(work),
        })
    }

    /// Whether the last check's own run took longer than the whole budget: this one would be
    /// stopped before it said anything.
    pub fn overruns(&self) -> bool {
        self.last_run.is_some_and(|run| run > self.total)
    }

    /// How long a check `elapsed` into its budget, its own run still to come, waits for the count:
    /// never so long that the wait and a run as long as the last one pass the budget, and never
    /// longer than [`Budget::count_wait`].
    pub fn count_wait_before_run(&self, elapsed: Duration) -> Duration {
        self.total
            .saturating_sub(elapsed)
            .saturating_sub(self.last_run.unwrap_or_default())
            .min(self.count_wait)
    }

    /// What a hook is told when the check [`Budget::overruns`]: the person's to run, as they can
    /// wait for it. The words are the Java edition's.
    pub fn overrun(&self) -> String {
        let last = self.last_run.unwrap_or_default();
        format!(
            "mutastic gate: this project's gate check takes longer than the hook allows ({}s last time; {BUDGET} allows {}s), so it's over to you: run `mutastic gate check` yourself",
            last.as_millis().div_ceil(1000),
            self.total.as_secs()
        )
    }
}

/// Keeps how long a check's own run took, for the next check's [`Budget::last_run`].
pub fn ran(work: &Path, took: Duration) {
    let _ = std::fs::create_dir_all(work);
    let _ = std::fs::write(
        work.join(RECORD),
        format!(
            "# mutastic gate: how long the last check's own run took\ntook-ms={}\n",
            took.as_millis()
        ),
    );
}

fn last_run(work: &Path) -> Option<Duration> {
    let text = std::fs::read_to_string(work.join(RECORD)).ok()?;
    let millis = text
        .lines()
        .find_map(|line| line.strip_prefix("took-ms="))?;
    millis.trim().parse().ok().map(Duration::from_millis)
}

/// `value`, of `variable`, as a whole number of seconds: above 0 for [`BUDGET`], 0 or more for
/// [`COUNT_WAIT`]; none when it's unset or empty.
pub fn seconds(variable: &str, value: Option<&str>) -> Result<Option<Duration>, Failure> {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let least = u64::from(variable == BUDGET);
    match value.parse::<u64>() {
        Ok(seconds) if seconds >= least => Ok(Some(Duration::from_secs(seconds))),
        _ => Err(Code::Environment.with(format!(
            "{variable}: {value} isn't a whole number of seconds{}",
            if least > 0 { " above 0" } else { "" }
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work(label: &str) -> std::path::PathBuf {
        let work = std::env::temp_dir().join(format!(
            "mutastic-gate-budget-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&work);
        work
    }

    /// The wait and a run as long as the last one never pass the budget, whenever the check
    /// starts waiting; a budget a few seconds long, not the hook's ten minutes.
    #[test]
    fn the_wait_never_pushes_the_check_past_its_budget() {
        for last_run in [None, Some(Duration::from_millis(1_500))] {
            let budget = Budget {
                total: Duration::from_secs(3),
                count_wait: Duration::from_secs(2),
                last_run,
            };
            for millis in (0..=5_000).step_by(250) {
                let elapsed = Duration::from_millis(millis);
                let wait = budget.count_wait_before_run(elapsed);
                let run = last_run.unwrap_or_default();
                assert!(wait <= budget.count_wait, "{elapsed:?} in");
                assert!(
                    wait.is_zero() || elapsed + wait + run <= budget.total,
                    "{elapsed:?} in, waits {wait:?}"
                );
            }
        }
        let budget = Budget {
            total: Duration::from_secs(3),
            count_wait: Duration::from_secs(2),
            last_run: Some(Duration::from_secs(1)),
        };
        assert_eq!(
            budget.count_wait_before_run(Duration::ZERO),
            Duration::from_secs(2)
        );
        assert_eq!(
            budget.count_wait_before_run(Duration::from_millis(1_500)),
            Duration::from_millis(500)
        );
        assert_eq!(
            budget.count_wait_before_run(Duration::from_secs(2)),
            Duration::ZERO
        );
    }

    /// A count still running when the budget runs out is left: the check says it couldn't count.
    #[test]
    fn a_slow_count_is_left_when_the_budget_runs_out() {
        let work = work("slow");
        std::fs::create_dir_all(&work).unwrap();
        let budget = Budget {
            total: Duration::from_secs(2),
            count_wait: Duration::from_secs(60),
            last_run: None,
        };
        let before = std::time::Instant::now();
        let wait = budget.count_wait_before_run(Duration::from_secs(1));
        assert_eq!(
            crate::gate_count::wait(&work, "t1", wait),
            Err("the count didn't finish in 1s".into())
        );
        assert!(before.elapsed() < Duration::from_secs(2));
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn a_last_run_longer_than_the_budget_overruns() {
        let work = work("overrun");
        let two = |name: &str| (name == BUDGET).then(|| "2".to_string());
        assert!(!Budget::of(two, &work).unwrap().overruns(), "no run yet");
        ran(&work, Duration::from_millis(1_500));
        assert!(!Budget::of(two, &work).unwrap().overruns());
        ran(&work, Duration::from_millis(2_001));
        let budget = Budget::of(two, &work).unwrap();
        assert!(budget.overruns());
        assert_eq!(
            budget.overrun(),
            "mutastic gate: this project's gate check takes longer than the hook allows (3s last time; MUTASTIC_GATE_BUDGET allows 2s), so it's over to you: run `mutastic gate check` yourself"
        );
        std::fs::remove_dir_all(&work).ok();
    }

    #[test]
    fn the_defaults_fit_the_plugins_hook() {
        let budget = Budget::of(|_| None, &work("defaults")).unwrap();
        assert_eq!(budget.total.as_secs(), 600, "the hook's \"timeout\": 600");
        assert_eq!(budget.count_wait.as_secs(), 240);
        assert_eq!(seconds(COUNT_WAIT, Some("0")), Ok(Some(Duration::ZERO)));
    }

    /// Decision 5: a value it doesn't take is refused, not read as the default.
    #[test]
    fn a_value_it_does_not_take_is_refused() {
        assert_eq!(
            seconds(BUDGET, Some("0")),
            Err(Code::Environment
                .with("MUTASTIC_GATE_BUDGET: 0 isn't a whole number of seconds above 0"))
        );
        assert_eq!(
            seconds(COUNT_WAIT, Some("4m")),
            Err(Code::Environment
                .with("MUTASTIC_GATE_COUNT_WAIT: 4m isn't a whole number of seconds"))
        );
    }
}
