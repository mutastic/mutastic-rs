//! A command line mutastic doesn't take, answered in the person's own words: the word as they typed
//! it, the one name close to it, if there is one, and last the line again with only that word
//! changed, to copy. At most one line of usage, never the whole help: that's a `--help` away.
//! A close name is offered, never acted on: a guess acted on runs what nobody asked for.

use crate::commands::{self, CliOption, Kind};
use crate::similar;

/// The commands, as typed after `mutastic`.
pub const COMMANDS: &[&str] = &[
    "run", "verify", "explain", "ignore", "gate", "mutants", "next", "baseline", "help",
];

/// `mutastic` and the words after it, with `args[at]` replaced by `with`, and the `taken` words
/// after it left out, as a shell takes them.
pub fn line(args: &[String], at: usize, with: &str, taken: usize) -> String {
    let words: Vec<String> = args
        .iter()
        .enumerate()
        .filter(|(index, _)| !(at + 1..=at + taken).contains(index))
        .map(|(index, word)| if index == at { with } else { word })
        .map(shell)
        .collect();
    format!("  mutastic {}", words.join(" "))
}

/// A word as a shell takes it: in double quotes when it has a space or a quote.
pub fn shell(word: &str) -> String {
    match word.split_once('=') {
        Some((flag, value)) if value.contains([' ', '\'', '"']) => {
            format!("{flag}=\"{}\"", value.replace('"', "\\\""))
        }
        _ if word.contains([' ', '\'', '"']) => format!("\"{}\"", word.replace('"', "\\\"")),
        _ => word.to_string(),
    }
}

/// `args[0]` isn't a command: a close one, the command typed after an option, or the commands.
pub fn command(args: &[String]) -> String {
    command_at(args, 0)
}

/// `args[at]` isn't a command, where one goes: first, or after `help`.
pub fn command_at(args: &[String], at: usize) -> String {
    let word = &args[at];
    if at == 0 && word.starts_with('-') {
        let reordered = args.iter().position(|arg| COMMANDS.contains(&arg.as_str()));
        return match reordered {
            Some(at) => {
                let mut words = vec![args[at].clone()];
                words.extend(
                    args.iter()
                        .enumerate()
                        .filter(|(i, _)| *i != at)
                        .map(|(_, w)| w.clone()),
                );
                format!(
                    "mutastic takes its command first, then the command's options.\n{}",
                    line(&words, 0, &words[0], 0)
                )
            }
            None => format!(
                "mutastic takes a command before its options, and {word} isn't one.\n{}",
                commands_usage()
            ),
        };
    }
    match similar::closest(word, COMMANDS.iter().copied()) {
        Some(close) => format!(
            "mutastic has no command {word}. There's one called {close}.\n{}",
            line(args, at, close, 0)
        ),
        None => format!("mutastic has no command {word}.\n{}", commands_usage()),
    }
}

/// The one line of usage for a command mutastic doesn't have, and where its commands are listed.
fn commands_usage() -> String {
    "Usage: mutastic <command> [dir] [options]. To see the commands:\n  mutastic --help".into()
}

/// The options a command takes, and `help`, every command's; every option for a command that
/// isn't declared, as `gate` before its action.
fn options_of(command: Option<&commands::Command>) -> Vec<&'static str> {
    let mut names: Vec<&str> = match command {
        Some(command) => commands::options_for(command.name)
            .iter()
            .map(|option| option.name)
            .collect(),
        None => commands::OPTIONS.iter().map(|option| option.name).collect(),
    };
    names.push("help");
    names
}

/// `--name`, at `args[at]`, is no option mutastic has: the close one this command takes, with the
/// line again using it, or the command's usage and how to list its options.
pub fn option(
    command: Option<&commands::Command>,
    args: &[String],
    at: usize,
    name: &str,
    value: Option<&str>,
) -> String {
    let typed = command.map_or(args[0].as_str(), |command| command.name);
    let candidates = options_of(command);
    let Some(close) = similar::closest(name, candidates.iter().copied()) else {
        let synopsis = command.map_or("mutastic gate open|check|close [dir]", |c| c.synopsis);
        return format!(
            "{typed} has no option --{name}.\nUsage: {synopsis}. To see its options:\n  mutastic {} --help",
            args[0]
        );
    };
    let (with, taken) = match (commands::option(close), value) {
        (_, Some(value)) => (format!("--{close}={value}"), 0),
        (Some(option), None) if takes_value(option) => match spaced(args, at) {
            Some(next) => (format!("--{close}={next}"), 1),
            None => (option.synopsis(), 0),
        },
        _ => (format!("--{close}"), 0),
    };
    format!(
        "{typed} has no option --{name}. There's one called --{close}.\n{}",
        line(args, at, &with, taken)
    )
}

/// Whether the option needs a value after an `=`.
fn takes_value(option: &CliOption) -> bool {
    matches!(option.kind, Kind::Value | Kind::Repeated)
}

/// The word after `args[at]`, when it reads as that option's value given after a space: a word
/// that isn't an option.
fn spaced(args: &[String], at: usize) -> Option<&str> {
    args.get(at + 1)
        .map(String::as_str)
        .filter(|next| !next.starts_with('-'))
}

/// Options whose own error says what an empty value needs: a mutant's id, the shares' directory,
/// a reason for whoever reviews it.
const SAY_WHAT_THEY_NEED: &[&str] = &["only", "from-shards", "reason"];

/// `--name`, at `args[at]`, needs a value and has none: the form it takes, and its choices. One
/// that must take a value has its value after a space joined to it first (`cli::joined`); an
/// optional one's, `--since main`, is said to need its `=`.
pub fn value(option: &CliOption, args: &[String], at: usize) -> Option<String> {
    let name = option.name;
    match option.kind {
        Kind::Value | Kind::Repeated => {}
        // `--since main` reads `main` as the directory: unless there's one, it was meant as the ref.
        Kind::OptionalValue => {
            let next = spaced(args, at).filter(|next| !std::path::Path::new(next).exists())?;
            return Some(format!(
                "--{name} takes its value after an =, not a space: for the changes since {next}, \
                 write --{name}={next}.\n{}",
                line(args, at, &format!("--{name}={next}"), 1)
            ));
        }
        Kind::Flag => return None,
    }
    // These say what they need in their own words.
    if SAY_WHAT_THEY_NEED.contains(&name) {
        return None;
    }
    let one_of = if option.choices.is_empty() {
        String::new()
    } else {
        format!(", one of {}", listed(option.choices))
    };
    Some(format!(
        "--{name} needs a value{one_of}.\n{}",
        line(args, at, &option.synopsis(), 0)
    ))
}

/// `value`, given to `--name`, isn't one of its choices: they're listed, and a close one is offered,
/// with the line again using it.
pub fn choice(option: &CliOption, args: &[String], at: usize, value: &str) -> String {
    let name = option.name;
    let said = match option.choices {
        [only] => format!("--{name}: {value} isn't {only}."),
        choices => format!("--{name}: {value} isn't one of {}.", listed(choices)),
    };
    match similar::closest(value, option.choices.iter().copied()) {
        Some(close) => {
            let typed = &args[at];
            let with = typed.replacen(&format!("={value}"), &format!("={close}"), 1);
            format!(
                "{said} There's one called {close}.\n{}",
                line(args, at, &with, 0)
            )
        }
        None => said,
    }
}

/// `a, b or c`.
pub fn listed(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [only] => only.to_string(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        line.split(' ').map(str::to_string).collect()
    }

    #[test]
    fn a_close_command_is_offered_in_the_line_typed() {
        assert_eq!(
            command(&words("rnu fixtures/pricing --format=agent")),
            "mutastic has no command rnu. There's one called run.\n  \
             mutastic run fixtures/pricing --format=agent"
        );
        assert_eq!(
            command(&words("build")),
            "mutastic has no command build.\nUsage: mutastic <command> [dir] [options]. To see \
             the commands:\n  mutastic --help"
        );
        assert_eq!(
            command(&words("--format=agent run .")),
            "mutastic takes its command first, then the command's options.\n  \
             mutastic run --format=agent ."
        );
    }

    #[test]
    fn a_close_option_is_offered_with_the_value_typed() {
        let run = commands::command("run");
        assert_eq!(
            option(run, &words("run --formt=agent"), 1, "formt", Some("agent")),
            "run has no option --formt. There's one called --format.\n  mutastic run --format=agent"
        );
        assert_eq!(
            option(run, &words("run --formt agent"), 1, "formt", None),
            "run has no option --formt. There's one called --format.\n  mutastic run --format=agent"
        );
        assert_eq!(
            option(run, &words("run --verbose"), 1, "verbose", None),
            "run has no option --verbose.\nUsage: mutastic run [dir] [options]. To see its \
             options:\n  mutastic run --help"
        );
    }

    #[test]
    fn a_value_after_a_space_is_shown_after_an_equals() {
        let format = commands::option("format").unwrap();
        assert_eq!(
            value(format, &words("run --format"), 1).unwrap(),
            "--format needs a value, one of full, brief, concise, grouped, agent or jsonl.\n  \
             mutastic run --format=FORMAT"
        );
        let since = commands::option("since").unwrap();
        assert_eq!(
            value(since, &words("run --since main"), 1).unwrap(),
            "--since takes its value after an =, not a space: for the changes since main, write \
             --since=main.\n  mutastic run --since=main"
        );
        assert_eq!(value(since, &words("run --since ."), 1), None);
    }

    #[test]
    fn a_close_choice_is_offered() {
        let format = commands::option("format").unwrap();
        assert_eq!(
            choice(format, &words("run --format=agnet"), 1, "agnet"),
            "--format: agnet isn't one of full, brief, concise, grouped, agent or jsonl. There's \
             one called agent.\n  mutastic run --format=agent"
        );
    }
}
