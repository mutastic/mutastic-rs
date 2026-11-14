//! The variables that stand in for an option, `MUTASTIC_FORMAT` for `--format`, checked before
//! anything runs. A variable is set once and forgotten, so one with a value mutastic doesn't take
//! is refused, as the Java edition refuses it, rather than quietly read as the default (decision
//! 5): the mistake would otherwise hide for as long as the variable stays set.

use crate::commands;

/// Each variable that stands in for an option, and the option, whose choices it takes.
const STAND_INS: &[(&str, &str)] = &[
    ("MUTASTIC_FORMAT", "format"),
    ("MUTASTIC_VOCABULARY", "vocabulary"),
    ("MUTASTIC_PROGRESS", "progress"),
];

/// Seconds a log may go without a progress line, read wherever progress is shown.
const HEARTBEAT: &str = "MUTASTIC_HEARTBEAT";

/// Refuses a value `command` would read and doesn't take: a stand-in's, for an option the command
/// takes and the line doesn't give (`given`), and `MUTASTIC_HEARTBEAT`'s, where progress is shown.
/// `variable` reads one; set to nothing, it's unset.
pub fn check(
    command: &str,
    given: impl Fn(&str) -> bool,
    variable: impl Fn(&str) -> Option<String>,
) -> Result<(), String> {
    let taken = |name: &str| commands::option(name).filter(|o| o.commands.contains(&command));
    let set = |name: &str| variable(name).filter(|value| !value.is_empty());
    for (name, option) in STAND_INS {
        let Some(option) = taken(option).filter(|option| !given(option.name)) else {
            continue;
        };
        if let Some(value) = set(name).filter(|value| !option.choices.contains(&value.as_str())) {
            return Err(format!(
                "{name}: {value} isn't one of {}",
                crate::mistyped::listed(option.choices)
            ));
        }
    }
    if taken("progress").is_some() {
        if let Some(value) =
            set(HEARTBEAT).filter(|value| crate::progress::seconds(value).is_none())
        {
            return Err(format!(
                "{HEARTBEAT}: {value} isn't a number of seconds above 0"
            ));
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

    #[test]
    fn a_value_an_option_does_not_take_is_refused() {
        let none = |_: &str| false;
        assert_eq!(
            check("run", none, with(&[("MUTASTIC_VOCABULARY", "xyz")])),
            Err("MUTASTIC_VOCABULARY: xyz isn't one of plain or classic".into())
        );
        assert_eq!(
            check("next", none, with(&[("MUTASTIC_FORMAT", "agnt")])),
            Err(
                "MUTASTIC_FORMAT: agnt isn't one of full, brief, concise, grouped, agent or jsonl"
                    .into()
            )
        );
        assert_eq!(
            check("verify", none, with(&[("MUTASTIC_PROGRESS", "bars")])),
            Err("MUTASTIC_PROGRESS: bars isn't one of auto, bar, counter or none".into())
        );
        assert_eq!(
            check("baseline", none, with(&[("MUTASTIC_HEARTBEAT", "0")])),
            Err("MUTASTIC_HEARTBEAT: 0 isn't a number of seconds above 0".into())
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
