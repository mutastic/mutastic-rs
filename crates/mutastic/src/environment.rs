//! The variables that stand in for an option, `MUTASTIC_FORMAT` for `--format`, checked before
//! anything runs. A variable is set once and forgotten, so one with a value mutastic doesn't take
//! is refused, as the Java edition refuses it, rather than quietly read as the default (decision
//! 5): the mistake would otherwise hide for as long as the variable stays set.

use crate::commands;
use crate::failure::{Code, Failure};

/// Each variable that stands in for an option, and the option, whose choices it takes.
const STAND_INS: &[(&str, &str)] = &[
    ("MUTASTIC_FORMAT", "format"),
    ("MUTASTIC_VOCABULARY", "vocabulary"),
    ("MUTASTIC_PROGRESS", "progress"),
];

/// Seconds a log may go without a progress line, read wherever progress is shown.
const HEARTBEAT: &str = "MUTASTIC_HEARTBEAT";

/// Refuses a value `command` would read and doesn't take: a stand-in's, for an option the command
/// takes and the line doesn't give (`given`), `MUTASTIC_HEARTBEAT`'s, where progress is shown, and
/// the gate's budget and wait, for `gate check`.
/// `variable` reads one; set to nothing, it's unset.
pub fn check(
    command: &str,
    given: impl Fn(&str) -> bool,
    variable: impl Fn(&str) -> Option<String>,
) -> Result<(), Failure> {
    let taken = |name: &str| commands::option(name).filter(|o| o.commands.contains(&command));
    let set = |name: &str| variable(name).filter(|value| !value.is_empty());
    for (name, option) in STAND_INS {
        let Some(option) = taken(option).filter(|option| !given(option.name)) else {
            continue;
        };
        if let Some(value) = set(name).filter(|value| !option.choices.contains(&value.as_str())) {
            return Err(Code::Environment.with(format!(
                "{name}: {value} isn't one of {}",
                crate::mistyped::listed(option.choices)
            )));
        }
    }
    if command == "gate check" {
        for name in [crate::gate_budget::BUDGET, crate::gate_budget::COUNT_WAIT] {
            crate::gate_budget::seconds(name, set(name).as_deref())?;
        }
    }
    if taken("progress").is_some() {
        if let Some(value) =
            set(HEARTBEAT).filter(|value| crate::progress::seconds(value).is_none())
        {
            return Err(Code::Environment.with(format!(
                "{HEARTBEAT}: {value} isn't a number of seconds above 0"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        |name| {
            pairs
                .iter()
                .find(|(set, _)| *set == name)
                .map(|(_, value)| value.to_string())
        }
    }

    /// The gate's budget and wait are checked before a check runs, and only for one.
    #[test]
    fn the_gate_s_budget_is_checked_for_gate_check() {
        let none = |_: &str| false;
        let budget = with(&[("MUTASTIC_GATE_BUDGET", "ten")]);
        assert_eq!(
            check("gate check", none, &budget),
            Err(Code::Environment
                .with("MUTASTIC_GATE_BUDGET: ten isn't a whole number of seconds above 0"))
        );
        assert_eq!(check("gate open", none, &budget), Ok(()));
        let wait = with(&[("MUTASTIC_GATE_COUNT_WAIT", "0")]);
        assert_eq!(check("gate check", none, wait), Ok(()));
    }

    #[test]
    fn a_value_an_option_does_not_take_is_refused() {
        let none = |_: &str| false;
        assert_eq!(
            check("run", none, with(&[("MUTASTIC_VOCABULARY", "xyz")])),
            Err(Code::Environment.with("MUTASTIC_VOCABULARY: xyz isn't one of plain or classic"))
        );
        assert_eq!(
            check("next", none, with(&[("MUTASTIC_FORMAT", "agnt")])),
            Err(Code::Environment.with(
                "MUTASTIC_FORMAT: agnt isn't one of full, brief, concise, grouped, agent or jsonl"
            ))
        );
        assert_eq!(
            check("verify", none, with(&[("MUTASTIC_PROGRESS", "bars")])),
            Err(Code::Environment
                .with("MUTASTIC_PROGRESS: bars isn't one of auto, bar, counter or none"))
        );
        assert_eq!(
            check("baseline", none, with(&[("MUTASTIC_HEARTBEAT", "0")])),
            Err(Code::Environment.with("MUTASTIC_HEARTBEAT: 0 isn't a number of seconds above 0"))
        );
    }

    #[test]
    fn a_value_nothing_reads_is_let_be() {
        let none = |_: &str| false;
        // The line's own --format wins, so the variable isn't read.
        let format = |option: &str| option == "format";
        assert_eq!(
            check("run", format, with(&[("MUTASTIC_FORMAT", "xyz")])),
            Ok(())
        );
        // mutants takes no --vocabulary, and shows no progress.
        assert_eq!(
            check(
                "mutants",
                none,
                with(&[("MUTASTIC_VOCABULARY", "xyz"), ("MUTASTIC_HEARTBEAT", "x")])
            ),
            Ok(())
        );
        assert_eq!(check("run", none, with(&[("MUTASTIC_FORMAT", "")])), Ok(()));
        assert_eq!(
            check(
                "run",
                none,
                with(&[
                    ("MUTASTIC_FORMAT", "agent"),
                    ("MUTASTIC_VOCABULARY", "classic"),
                    ("MUTASTIC_PROGRESS", "auto"),
                    ("MUTASTIC_HEARTBEAT", "0.5"),
                ])
            ),
            Ok(())
        );
    }
}
