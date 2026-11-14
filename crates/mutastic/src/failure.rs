//! Why mutastic couldn't run, for a tool or an agent: the `run-failed` event, the last on stdout in
//! the agent and jsonl formats, with the same words stderr has. Its `code` names the kind of
//! failure, stable from release to release, and is the anchor of its entry in
//! `docs/reference/errors.md`; each command the error ends in is in `help`, with whether it can be
//! run as it is. Codes never appear in the text a person reads.

use serde_json::{json, Value};

/// Every code, with what in the error's text says it's that kind, in the order they're tried. The
/// error page has an entry for each, and the docs test holds the two to each other.
pub const CODES: &[(&str, &[&str])] = &[
    ("copy-build-failed", &["mutastic's copy of "]),
    (
        "build-failed",
        &[
            "doesn't build, before any mutant is in it",
            "the build fails, and not because of a mutant",
            "the build still fails after",
            "cargo failed in the copy",
        ],
    ),
    (
        "red-baseline",
        &[" without any mutant, so mutastic can't judge one"],
    ),
    ("no-directory", &["there's no directory "]),
    (
        "no-project",
        &["there's no project ", "isn't a Cargo project"],
    ),
    (
        "workspace-root",
        &[
            "a workspace's root",
            "in a workspace, but in none of its packages",
        ],
    ),
    (
        "no-cargo",
        &[
            "there's no cargo ",
            "and there's no cargo there",
            "couldn't run cargo",
        ],
    ),
    (
        "unknown-command",
        &[
            "has no command ",
            "takes its command first",
            "takes a command before",
        ],
    ),
    ("unknown-option", &[" has no option -"]),
    (
        "shards",
        &[
            "--from-shards",
            "share's events",
            "shares of runs",
            "share of a",
            "the shares in ",
            "shares and put",
            "its events stop before run-finished",
            "of the shares",
            " is there twice, in ",
            " is missing from ",
        ],
    ),
    ("option-not-taken", &[" doesn't take --"]),
    (
        "environment",
        &[
            "MUTASTIC_FORMAT: ",
            "MUTASTIC_VOCABULARY: ",
            "MUTASTIC_PROGRESS: ",
            "MUTASTIC_HEARTBEAT: ",
        ],
    ),
    (
        "option-value",
        &[
            "takes its value after an =",
            " needs a value",
            "isn't one of",
            "isn't a number",
            "isn't a positive number",
            "needs a whole number",
            "--threshold is from ",
            "takes no value",
            "--hook: ",
            "--shard needs",
            "--shard: ",
            "--only=",
            "--only needs",
            "--reason=",
        ],
    ),
    (
        "needs-id",
        &[
            "needs a mutant's id",
            "needs a reason, for whoever reviews it",
        ],
    ),
    ("unknown-id", &[" isn't a mutant of "]),
    (
        "config",
        &[
            "mutastic.yaml",
            "ignores.yaml",
            "proposals.yaml",
            "isn't an operator mutastic has",
            "the plugin ",
            "extends: goes round",
        ],
    ),
    ("no-baseline", &["--fail-on-new needs a baseline"]),
    ("git", &["needs a git repository", "git knows no commit"]),
    ("both-editions", &["so either edition could read it"]),
    (
        "java-edition",
        &[
            "Java edition",
            "JDK",
            "MUTASTIC_JAVA_ENGINE",
            "MUTASTIC_HOME",
        ],
    ),
    ("proc-macro", &["is a proc-macro crate"]),
    ("extra-words", &[" doesn't take "]),
];

/// The code for anything else: an error mutastic has no entry for yet, usually one the system
/// gave (a disk that's full, a file it can't read).
pub const OTHER: &str = "other";

/// The kind of failure `message` says.
pub fn code(message: &str) -> &'static str {
    CODES
        .iter()
        .find(|(_, says)| says.iter().any(|said| message.contains(said)))
        .map_or(OTHER, |(code, _)| code)
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

/// The `run-failed` event for `message`, as mutastic prints it after `mutastic: `, which ended
/// a command run on `directory`, as typed, with `exit` as its status.
pub fn event(message: &str, directory: &str, exit: u8) -> Value {
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
        "code": code(message),
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
    fn an_error_is_coded_by_what_it_says() {
        for (message, expected) in [
            ("run has no option --formt. There's one called --format.\n  mutastic run --format=agent", "unknown-option"),
            ("run has no option -v.\nmutastic's options are long ones", "unknown-option"),
            ("mutastic has no command rnu. There's one called run.\n  mutastic run", "unknown-command"),
            ("--format: agnt isn't one of full, brief, concise, grouped, agent or jsonl. There's one called agent.\n  mutastic run --format=agent", "option-value"),
            ("--threshold is from 0 to 100, not 150", "option-value"),
            ("--threshold needs a whole number, not 72.5", "option-value"),
            ("MUTASTIC_VOCABULARY: xyz isn't one of plain or classic", "environment"),
            ("there's no directory shpo in /p.\n", "no-directory"),
            ("this is a workspace's root, and mutastic tests one package at a time.", "workspace-root"),
            ("t fails without any mutant, so mutastic can't judge one.", "red-baseline"),
            ("pricing doesn't build, before any mutant is in it.", "build-failed"),
            ("mutastic's copy of pricing doesn't build, though pricing does", "copy-build-failed"),
            ("m00000000 isn't a mutant of pricing. To see its mutants:\n  mutastic mutants .", "unknown-id"),
            ("mutastic.yaml line 1: expected extends:", "config"),
            ("--since needs a git repository", "git"),
            ("run doesn't take --top: mutastic run [DIR] [OPTIONS]", "option-not-taken"),
            ("run doesn't take explain m1: mutastic run [DIR] [OPTIONS]", "extra-words"),
            ("disk full", OTHER),
        ] {
            assert_eq!(code(message), expected, "{message}");
        }
    }

    #[test]
    fn each_command_says_whether_it_runs_as_it_is() {
        let guessed = "run has no option --formt. There's one called --format.\n  mutastic run --format=agent";
        let event = event(guessed, "no/such/dir", 2);
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
