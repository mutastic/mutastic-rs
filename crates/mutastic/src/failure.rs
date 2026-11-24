//! Why mutastic couldn't run, for a tool or an agent: the `run-failed` event, the last on stdout in
//! the agent and jsonl formats, with the same words stderr has. Its `code` names the kind of
//! failure, stable from release to release, and is the anchor of its entry in
//! `docs/reference/errors.md`; each command the error ends in is in `help`, with whether it can be
//! run as it is. Codes never appear in the text a person reads.

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
    /// `--at` asked for lines with no mutant on them.
    NoMutantAt,
    /// `--at` named a file that isn't one of the package's sources.
    OutsideSources,
    Config,
    Environment,
    NoBaseline,
    Git,
    Shards,
    BothEditions,
    BuildFailed,
    CopyBuildFailed,
    RedBaseline,
    /// A test ended its test process with no mutant in: an exit its code called, or a crash.
    TestsStopped,
    /// A test ran past the coverage pass's limit with no mutant in.
    TestsTimedOut,
    /// The package has tests, and none of them runs.
    NoTests,
    /// The tests ran and passed, and reached none of the code with mutants.
    NothingCovered,
    JavaEdition,
    ProcMacro,
    /// A file mutastic reads or writes couldn't be: a full disk, a directory that isn't writable, a
    /// file where it keeps a directory.
    Disk,
    /// Anything else: an error mutastic has no entry for yet.
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
        Code::NoMutantAt,
        Code::OutsideSources,
        Code::Config,
        Code::Environment,
        Code::NoBaseline,
        Code::Git,
        Code::Shards,
        Code::BothEditions,
        Code::BuildFailed,
        Code::CopyBuildFailed,
        Code::RedBaseline,
        Code::TestsStopped,
        Code::TestsTimedOut,
        Code::NoTests,
        Code::NothingCovered,
        Code::JavaEdition,
        Code::ProcMacro,
        Code::Disk,
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
            Code::NoMutantAt => "no-mutant-at",
            Code::OutsideSources => "outside-sources",
            Code::Config => "config",
            Code::Environment => "environment",
            Code::NoBaseline => "no-baseline",
            Code::Git => "git",
            Code::Shards => "shards",
            Code::BothEditions => "both-editions",
            Code::BuildFailed => "build-failed",
            Code::CopyBuildFailed => "copy-build-failed",
            Code::RedBaseline => "red-baseline",
            Code::TestsStopped => "tests-stopped",
            Code::TestsTimedOut => "tests-timed-out",
            Code::NoTests => "no-tests",
            Code::NothingCovered => "nothing-covered",
            Code::JavaEdition => "java-edition",
            Code::ProcMacro => "proc-macro",
            Code::Disk => "disk",
            Code::Other => "other",
            Code::Internal => "internal",
        }
    }

    /// The failure this code names, in `message`'s words.
    pub fn with(self, message: impl Into<String>) -> Failure {
        Failure {
            code: self,
            message: message.into(),
            correction: None,
        }
    }
}

/// Why mutastic couldn't run: the kind of failure, and what a person reads after `mutastic: `.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub code: Code,
    pub message: String,
    /// A word of the command line, and the close name to write in its place: the line, so
    /// corrected, follows the message, once the line is known.
    pub correction: Option<(String, String)>,
}

impl Failure {
    /// The failure, with `typed`, a word of the command line or part of one, to be corrected to
    /// `instead` in the line that follows it.
    pub fn correcting(mut self, typed: &str, instead: &str) -> Failure {
        self.correction = Some((typed.to_string(), instead.to_string()));
        self
    }

    /// The failure with its correction made in `args`, the line as typed, as the line to copy that
    /// ends its message; as it was when the word isn't in the line.
    pub fn corrected(mut self, args: &[String]) -> Failure {
        let Some((typed, instead)) = self.correction.take() else {
            return self;
        };
        let at = args
            .iter()
            .position(|arg| arg == &typed || arg.split(['=', ',']).any(|part| part == typed));
        if let Some(at) = at {
            let with = if args[at] == typed {
                instead
            } else {
                let (option, list) = args[at].split_once('=').unwrap_or(("", &args[at]));
                let list: Vec<&str> = list
                    .split(',')
                    .map(|part| {
                        if part == typed {
                            instead.as_str()
                        } else {
                            part
                        }
                    })
                    .collect();
                if option.is_empty() {
                    list.join(",")
                } else {
                    format!("{option}={}", list.join(","))
                }
            };
            self.message = format!(
                "{}\n{}",
                self.message,
                crate::mistyped::line(args, at, &with, 0)
            );
        }
        self
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// A file mutastic reads or writes couldn't be, in `said`'s words, which name the file and give the
/// system's reason: `couldn't make target/mutastic: Not a directory (os error 20)`. As the Java
/// edition's `Disk`, what to do about the commonest reasons follows: they're the machine's, not the
/// run's.
pub fn disk(said: impl Into<String>) -> Failure {
    let said = said.into();
    let advice = disk_advice(&said);
    Code::Disk.with(format!("{said}{advice}"))
}

/// What to do about a full disk, a place that can't be written to, or a file where a directory
/// goes, by the system's words for each, on Unix and on Windows; nothing for any other reason.
fn disk_advice(reason: &str) -> &'static str {
    let reason = reason.to_lowercase();
    let says = |any: &[&str]| any.iter().any(|words| reason.contains(words));
    if says(&["no space left", "not enough space"]) {
        ". The disk is full: free some space, and run mutastic again"
    } else if says(&[
        "permission denied",
        "read-only file system",
        "access is denied",
    ]) {
        ". mutastic keeps its work in the package's target directory, and its decisions in \
         .mutastic: both need to be writable"
    } else if says(&["not a directory", "file exists", "already exists"]) {
        ". A file is where mutastic keeps a directory: move it, and run mutastic again"
    } else {
        ""
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_is_said_as_a_bug_with_where_to_report_it() {
        let said = internal_message("index out of bounds", ", at src/engine.rs:12");
        assert!(said.starts_with(
            "internal error: index out of bounds, at src/engine.rs:12. That's a bug in mutastic"
        ));
    }

    #[test]
    fn a_file_failure_is_the_disks_with_what_to_do_about_it() {
        let full =
            disk("couldn't write target/mutastic/tests.log: No space left on device (os error 28)");
        assert_eq!(full.code, Code::Disk);
        assert_eq!(
            full.message,
            "couldn't write target/mutastic/tests.log: No space left on device (os error 28). The \
             disk is full: free some space, and run mutastic again"
        );
        assert!(disk("couldn't make target/mutastic: Permission denied (os error 13)")
            .message
            .ends_with(". mutastic keeps its work in the package's target directory, and its decisions in .mutastic: both need to be writable"));
        assert!(
            disk("couldn't make target/mutastic: Not a directory (os error 20)")
                .message
                .ends_with(
                    ". A file is where mutastic keeps a directory: move it, and run mutastic again"
                )
        );
        let other = disk("couldn't read shares: Input/output error (os error 5)");
        assert_eq!(
            other.message,
            "couldn't read shares: Input/output error (os error 5)"
        );
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
