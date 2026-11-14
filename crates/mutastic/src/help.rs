//! What `mutastic --help` and `mutastic <command> --help` print, written from the declarations in
//! [`commands`], in the Java edition's layout, which the editions' contract holds both to
//! (decision 10): a `Usage:` line, what the command does, an `Exits` paragraph, examples, the
//! options, the exit codes and a `Docs:` link, every line wrapped to [`WIDTH`]. Help can't offer an
//! option the parser doesn't take, nor leave one out.

use crate::commands::{self, CliOption, Command, Entry};

/// The widest a line of help is, as the Java edition's.
pub const WIDTH: usize = 100;

/// Where an option's description starts.
const COLUMN: usize = 26;

/// A few commands to start from, for `mutastic --help`.
pub const EXAMPLES: &[&str] = &[
    "mutastic run",
    "mutastic run fixtures/pricing --threshold=70",
    "mutastic run --since=main",
    "mutastic verify mb2f8ae57",
];

/// `mutastic --help`: every command, examples, every option by its group, the environment and the
/// exit codes.
pub fn general() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let mut out = vec![
        format!("mutastic {version}: finds the changes your tests don't notice."),
        String::new(),
        "Usage:".to_string(),
    ];
    let usage: Vec<Entry> = commands::COMMANDS
        .iter()
        .map(|command| Entry {
            name: command.synopsis,
            meaning: command.summary,
            detail: "",
        })
        .collect();
    out.extend(entries(&usage));
    out.push(String::new());
    out.push("Examples:".to_string());
    out.extend(EXAMPLES.iter().flat_map(|example| example_lines(example)));
    for group in groups() {
        out.push(String::new());
        out.push(format!("{group}:"));
        for option in commands::OPTIONS.iter().filter(|o| o.group == group) {
            out.extend(described(option));
        }
    }
    out.extend(section("Environment:", commands::ENVIRONMENT));
    out.extend(section("Exit codes:", commands::EXIT_CODES));
    out.push(String::new());
    out.extend(wrapped(
        &format!(
            "mutastic <command> --help says more about each. Docs: {}",
            commands::DOCS
        ),
        WIDTH,
    ));
    out.join("\n")
}

/// `mutastic run --help`: its usage, what it does, how it exits, examples, its options, the exit
/// codes and its page in the docs.
pub fn of(command: &Command) -> String {
    let mut out = vec![format!("Usage: {}", command.synopsis), String::new()];
    out.extend(wrapped(
        &format!("{}. {}", command.summary, command.detail),
        WIDTH,
    ));
    out.push(String::new());
    out.extend(wrapped(&format!("Exits {}", command.exits), WIDTH));
    out.push(String::new());
    out.push("Examples:".to_string());
    out.extend(
        command
            .examples
            .iter()
            .flat_map(|example| example_lines(example)),
    );
    let options = commands::options_for(command.name);
    if !options.is_empty() {
        out.push(String::new());
        out.push("Options:".to_string());
        for option in options {
            out.extend(described(option));
        }
    }
    out.extend(section("Exit codes:", commands::EXIT_CODES));
    out.push(String::new());
    out.push(format!(
        "Docs: {}reference/{}/",
        commands::DOCS,
        command.page
    ));
    out.join("\n")
}

/// The options' groups, in the order the options are declared.
fn groups() -> Vec<&'static str> {
    let mut seen = Vec::new();
    for option in commands::OPTIONS {
        if !seen.contains(&option.group) {
            seen.push(option.group);
        }
    }
    seen
}

/// `  --threshold=N   The score…`, wrapped under its description, with the values it takes and the
/// variables that stand in for it.
fn described(option: &CliOption) -> Vec<String> {
    let mut text = option.help.to_string();
    if !option.choices.is_empty() {
        text.push_str(&format!(" ({})", option.choices.join(", ")));
    }
    if !option.environment.is_empty() {
        text.push_str(&format!(". Also: {}", option.environment));
    }
    hanging(&format!("  {}", option.synopsis()), &text, COLUMN)
}

/// A heading and its entries, each name in a column as wide as the longest, and its meaning beside.
fn section(heading: &str, listed: &[Entry]) -> Vec<String> {
    let mut out = vec![String::new(), heading.to_string()];
    out.extend(entries(listed));
    out
}

fn entries(listed: &[Entry]) -> Vec<String> {
    let column = listed
        .iter()
        .map(|entry| entry.name.chars().count())
        .max()
        .unwrap_or(0)
        + 4;
    listed
        .iter()
        .flat_map(|entry| hanging(&format!("  {}", entry.name), entry.meaning, column))
        .collect()
}

/// `name`, then `text` from `column` on, wrapped there; a name too wide for the column has a line
/// of its own.
fn hanging(name: &str, text: &str, column: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut first = name.to_string();
    if first.chars().count() + 1 > column {
        out.push(first);
        first = String::new();
    }
    for line in wrapped(text, WIDTH - column) {
        out.push(format!("{first:<column$}{line}"));
        first = String::new();
    }
    out
}

/// An example as it's shown, set in by two spaces. One too long for a line is broken between its
/// words, outside quotes, each line but the last ending in ` \` so the whole still pastes into a
/// shell as one command.
fn example_lines(example: &str) -> Vec<String> {
    const INDENT: &str = "  ";
    const MORE: &str = "      ";
    const BREAK: &str = " \\";
    if INDENT.len() + example.chars().count() <= WIDTH {
        return vec![format!("{INDENT}{example}")];
    }
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in shell_words(example) {
        let lead = if lines.is_empty() { INDENT } else { MORE };
        let longer = lead.len() + line.chars().count() + 1 + word.chars().count() + BREAK.len();
        if !line.is_empty() && longer > WIDTH {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    lines.push(line);
    let last = lines.len() - 1;
    lines
        .into_iter()
        .enumerate()
        .map(|(at, line)| {
            let lead = if at == 0 { INDENT } else { MORE };
            let more = if at < last { BREAK } else { "" };
            format!("{lead}{line}{more}")
        })
        .collect()
}

/// `line`'s words, split at spaces outside double quotes.
fn shell_words(line: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut quoted = false;
    let mut start = 0;
    for (at, character) in line.char_indices() {
        match character {
            '"' => quoted = !quoted,
            ' ' if !quoted => {
                words.push(&line[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    words.push(&line[start..]);
    words.retain(|word| !word.is_empty());
    words
}

/// `text` in lines of at most `width` characters, broken between words.
pub fn wrapped(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    lines.push(line);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_help() -> Vec<String> {
        let mut every: Vec<String> = commands::COMMANDS.iter().map(of).collect();
        every.push(general());
        every
    }

    #[test]
    fn no_line_of_help_is_wider_than_the_width() {
        for help in every_help() {
            for line in help.lines() {
                assert!(line.chars().count() <= WIDTH, "{line}");
                assert_eq!(line, line.trim_end(), "{line}");
            }
        }
    }

    #[test]
    fn a_command_help_lists_each_option_it_takes() {
        for command in commands::COMMANDS {
            let help = of(command);
            assert!(help.starts_with(&format!("Usage: {}\n\n", command.synopsis)));
            for option in commands::options_for(command.name) {
                assert!(
                    help.contains(&format!("\n  {}", option.synopsis())),
                    "{}: {}",
                    command.name,
                    option.name
                );
            }
        }
    }

    /// Each command's `Docs:` link is the page the docs site writes for it from the committed
    /// `docs/commands.json`: its `docs` root, `reference/`, and the command's `page`, as
    /// `rust/reference/<page>/`. The top help links the root.
    #[test]
    fn each_docs_link_is_a_page_the_docs_site_writes() {
        let model: serde_json::Value =
            serde_json::from_str(include_str!("../../../docs/commands.json")).unwrap_or_default();
        let root = model["docs"].as_str().unwrap_or_default();
        assert!(root.ends_with("/rust/"), "{root}");
        let pages: Vec<&str> = model["commands"]
            .as_array()
            .map(|commands| commands.iter().filter_map(|c| c["page"].as_str()).collect())
            .unwrap_or_default();
        assert_eq!(pages.len(), commands::COMMANDS.len());
        for (command, page) in commands::COMMANDS.iter().zip(pages) {
            let docs: Vec<String> = of(command)
                .lines()
                .filter_map(|line| line.strip_prefix("Docs: "))
                .map(str::to_string)
                .collect();
            assert_eq!(
                docs,
                [format!("{root}reference/{page}/")],
                "{}",
                command.name
            );
        }
        assert!(general().ends_with(&format!("Docs: {root}")));
    }

    #[test]
    fn a_long_example_breaks_outside_its_quotes() {
        let long = "mutastic ignore mb2f8ae57 --propose --as=unreachable --reason=\"Differs only \
                    when total is 100. That can't happen because prices end in 9.\"";
        let lines = example_lines(long);
        assert_eq!(
            lines,
            [
                "  mutastic ignore mb2f8ae57 --propose --as=unreachable \\",
                "      --reason=\"Differs only when total is 100. That can't happen because prices \
                 end in 9.\""
            ]
        );
        let joined: Vec<&str> = lines
            .iter()
            .map(|line| line.trim().trim_end_matches(" \\"))
            .collect();
        assert_eq!(joined.join(" "), long);
    }

    #[test]
    fn text_breaks_between_words() {
        assert_eq!(wrapped("one two three", 7), ["one two", "three"]);
        assert_eq!(wrapped("", 7), [""]);
    }
}
