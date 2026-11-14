//! Why mutastic couldn't run, for a tool or an agent: the `run-failed` event, the last on stdout in
//! the agent and jsonl formats, with the same words stderr has. Its `code` names the kind of
//! failure, stable from release to release, and is the anchor of its entry in
//! `docs/reference/errors.md`; each command the error ends in is in `help`, with whether it can be
//! run as it is. Codes never appear in the text a person reads.

use serde_json::{json, Value};

/// The kind of failure, set where the error is raised (decision 13): an error can't reach a person
/// without one, and nothing works it out from the words afterwards, so rewording a message never
/// moves its code. Each has an entry on the errors page, and the docs test holds the two to each
/// other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    NoDirectory,
    NoProject,
    WorkspaceRoot,
    NoCargo,
    UnknownCommand,
    UnknownOption,
    OptionValue,
    OptionNotTaken,
    ExtraWords,
    NeedsId,
    UnknownId,
    Config,
    Environment,
    NoBaseline,
    Git,
    Shards,
    BothEditions,
    BuildFailed,
    CopyBuildFailed,
    RedBaseline,
    JavaEdition,
    ProcMacro,
    /// Anything else: an error mutastic has no entry for yet, usually one the system gave (a disk
    /// that's full, a file it can't read).
    Other,
    /// A bug in mutastic itself: it panicked.
    Internal,
}

impl Code {
    /// Every code, in the errors page's order.
    pub const ALL: &[Code] = &[
        Code::NoDirectory,
        Code::NoProject,
        Code::WorkspaceRoot,
        Code::NoCargo,
        Code::UnknownCommand,
        Code::UnknownOption,
        Code::OptionValue,
        Code::OptionNotTaken,
        Code::ExtraWords,
        Code::NeedsId,
        Code::UnknownId,
        Code::Config,
        Code::Environment,
        Code::NoBaseline,
        Code::Git,
        Code::Shards,
        Code::BothEditions,
        Code::BuildFailed,
        Code::CopyBuildFailed,
        Code::RedBaseline,
        Code::JavaEdition,
        Code::ProcMacro,
        Code::Other,
        Code::Internal,
    ];

    /// As the event and the errors page write it: `no-directory`.
    pub fn name(self) -> &'static str {
        match self {
            Code::NoDirectory => "no-directory",
            Code::NoProject => "no-project",
            Code::WorkspaceRoot => "workspace-root",
            Code::NoCargo => "no-cargo",
            Code::UnknownCommand => "unknown-command",
            Code::UnknownOption => "unknown-option",
            Code::OptionValue => "option-value",
            Code::OptionNotTaken => "option-not-taken",
            Code::ExtraWords => "extra-words",
            Code::NeedsId => "needs-id",
            Code::UnknownId => "unknown-id",
            Code::Config => "config",
            Code::Environment => "environment",
            Code::NoBaseline => "no-baseline",
            Code::Git => "git",
            Code::Shards => "shards",
            Code::BothEditions => "both-editions",
            Code::BuildFailed => "build-failed",
            Code::CopyBuildFailed => "copy-build-failed",
            Code::RedBaseline => "red-baseline",
            Code::JavaEdition => "java-edition",
            Code::ProcMacro => "proc-macro",
            Code::Other => "other",
            Code::Internal => "internal",
        }
    }

    /// The failure this code names, in `message`'s words.
    pub fn with(self, message: impl Into<String>) -> Failure {
        Failure {
            code: self,
            message: message.into(),
        }
    }
}

/// Why mutastic couldn't run: the kind of failure, and what a person reads after `mutastic: `.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub code: Code,
    pub message: String,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// The failure a panic is: a bug in mutastic, with where it panicked and where to report it.
pub fn internal(panic: &std::panic::PanicHookInfo<'_>) -> Failure {
    let payload = panic.payload();
    let said = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("a panic");
    let place = panic.location().map_or(String::new(), |at| {
        format!(", at {}:{}", at.file(), at.line())
    });
    Code::Internal.with(internal_message(said, &place))
}

fn internal_message(said: &str, place: &str) -> String {
    format!(
        "internal error: {said}{place}. That's a bug in mutastic, not in your code. To report it, \
         with the error above:\n  https://github.com/mutastic/mutastic-rs/issues"
    )
}

/// How sure mutastic is of a command it offers, as rustc grades a suggestion: `exact` runs as it
/// is; `placeholders` has a part to fill in, `<…>`; `maybe` is a guess at what was meant, a close
/// name, for a person or an agent to check before running it.
pub fn applicability(command: &str, message: &str) -> &'static str {
    let guessed = offered(message).into_iter().any(|name| {
        command.split(' ').any(|word| {
            word == name
                || word.starts_with(&format!("{name}="))
                || word.ends_with(&format!("={name}"))
        })
    });
    if command.contains('<') || command.contains('…') {
        "placeholders"
    } else if guessed {
        "maybe"
    } else {
        "exact"
    }
}

/// The close names `message` offers: `There's one called --format.`, `There's a package called
/// shop, in crates/shop.`, without their punctuation; for a package, its directory too.
fn offered(message: &str) -> Vec<&str> {
    let mut names = Vec::new();
    for lead in [
        "There's one called ",
        "There's a package called ",
        "There's a directory called ",
    ] {
        for (_, rest) in message
            .match_indices(lead)
            .map(|(at, _)| message.split_at(at + lead.len()))
        {
            let name = rest.split(['.', ',', ' ', '\n']).next().unwrap_or_default();
            let name = name.trim_end_matches(':');
            names.push(name);
            if let Some((_, after)) = rest.split_once(", in ") {
                names.push(after.split(['.', ' ', '\n']).next().unwrap_or_default());
            }
        }
    }
    names.retain(|name| !name.is_empty());
    names
}

/// The commands `message` ends in: each line of it set in by two spaces that runs mutastic, Cargo
/// or git.
pub fn help(message: &str) -> Vec<String> {
    message
        .lines()
        .filter_map(|line| line.strip_prefix("  "))
        .filter(|line| !line.starts_with(' '))
        .filter(|line| {
            ["mutastic ", "cargo ", "git "]
                .iter()
                .any(|tool| line.starts_with(tool))
        })
        .map(str::to_string)
        .collect()
}

/// The `run-failed` event for `failure`, whose message mutastic prints after `mutastic: `, which
/// ended a command run on `directory`, as typed, with `exit` as its status.
pub fn event(failure: &Failure, directory: &str, exit: u8) -> Value {
    let message = failure.message.as_str();
    // Windows canonicalizes to a verbatim path, `\\?\C:\…`, which no one types.
    let path = std::fs::canonicalize(directory).ok().map(|path| {
        let shown = crate::workspace::slashed(&path);
        shown
            .strip_prefix("//?/")
            .map_or_else(|| shown.clone(), str::to_string)
    });
    let help: Vec<Value> = help(message)
        .into_iter()
        .map(|command| {
            json!({
                "applicability": applicability(&command, message),
                "command": command,
            })
        })
        .collect();
    json!({
        "schema": crate::report::SCHEMA,
        "edition": crate::report::EDITION,
        "reason": "run-failed",
        "code": failure.code.name(),
        "message": message.lines().next().unwrap_or_default(),
        "where": {"directory": directory, "path": path},
        "help": help,
        "exitCode": exit,
        "rendered": format!("mutastic: {message}\n"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_is_said_as_a_bug_with_where_to_report_it() {
        let said = internal_message("index out of bounds", ", at src/engine.rs:12");
        assert!(said.starts_with(
            "internal error: index out of bounds, at src/engine.rs:12. That's a bug in mutastic"
        ));
        let event = event(&Code::Internal.with(said), ".", 2);
        assert_eq!(event["code"], "internal");
        assert_eq!(event["help"], json!([]));
    }

    #[test]
    fn each_code_has_a_name_of_its_own() {
        let mut names: Vec<&str> = Code::ALL.iter().map(|code| code.name()).collect();
        let all = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all);
    }

    #[test]
    fn each_command_says_whether_it_runs_as_it_is() {
        let guessed = "run has no option --formt. There's one called --format.\n  mutastic run --format=agent";
        let event = event(&Code::UnknownOption.with(guessed), "no/such/dir", 2);
        assert_eq!(event["code"], "unknown-option");
        assert_eq!(
            event["message"],
            "run has no option --formt. There's one called --format."
        );
        assert_eq!(
            event["help"],
            json!([{"command": "mutastic run --format=agent", "applicability": "maybe"}])
        );
        assert_eq!(
            event["where"],
            json!({"directory": "no/such/dir", "path": null})
        );
        assert_eq!(event["rendered"], format!("mutastic: {guessed}\n"));
        let placeholder = "there's no project here for mutastic to test.\nGo to your project:\n  mutastic run <project directory>";
        assert_eq!(help(placeholder), ["mutastic run <project directory>"]);
        assert_eq!(
            applicability("mutastic run <project directory>", placeholder),
            "placeholders"
        );
        let red = "t fails without any mutant, so mutastic can't judge one.\nIts output is in x. To run it alone:\n  cargo test -p red --lib -- --exact t";
        assert_eq!(help(red), ["cargo test -p red --lib -- --exact t"]);
        assert_eq!(applicability(&help(red)[0], red), "exact");
        let near = "m1 isn't a mutant of p. There's one called m2. To see its mutants:\n  mutastic mutants .";
        assert_eq!(applicability("mutastic mutants .", near), "exact");
        assert_eq!(applicability("mutastic verify m2 .", near), "maybe");
        let package = "there's no directory shpo.\nThere's a package called shop, in crates/shop. To test it:\n  mutastic run crates/shop";
        assert_eq!(applicability("mutastic run crates/shop", package), "maybe");
    }
}
