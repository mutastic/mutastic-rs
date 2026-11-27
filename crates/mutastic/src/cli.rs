//! The `mutastic` command, also run as `cargo mutastic`.

#![expect(
    clippy::disallowed_methods,
    reason = "reads the variables the command declares: the format, the colour, the agent"
)]

use std::ffi::OsString;
use std::io::IsTerminal;
use std::process::ExitCode;

use crate::edition::{self, Edition, Route};
use crate::engine::{self, Options, Status};
use crate::failure::{Code, Failure};
use crate::progress::{Progress, Shown};
use crate::report::files::Kind;
use crate::report::{self, Format, Style};
use crate::shares::EVENTS;
use crate::sites::UnsafeMode;
use crate::test_times::{self, TestTimes};
use crate::{
    baseline, commands, gate, gate_budget, gate_count, ignores, java, sites, swift, testrun,
    untrusted, workspace, wrapper,
};

pub fn main(args: Vec<OsString>) -> ExitCode {
    if wrapper::active(&args) {
        return wrapper::run(&args);
    }
    if gate_count::active() {
        return gate_count::run(&args);
    }
    let mut args: Vec<String> = args
        .iter()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    // `cargo mutastic run` runs `cargo-mutastic mutastic run`.
    if args.first().is_some_and(|arg| arg == "mutastic") {
        args.remove(0);
    }
    // Nothing after `--` is mutastic's own: not its edition's choice, nor the directory it's on.
    let (own, build) = crate::build_args::split_line(&args);
    let args = joined(own);
    if let Some(build) = &build {
        crate::build_args::typed(build.clone());
    }
    hook_panics(&args);
    let args = match edition::chosen(&args).and_then(|(chosen, args)| {
        front_door(chosen, &args, build.as_deref()).map(|forwarded| (forwarded, args))
    }) {
        Ok((Some(code), _)) => return code,
        Ok((None, args)) => args,
        Err(failure) => return failed(&args, &failure),
    };
    match dispatch(&args) {
        Ok(code) => code,
        Err(failure) => failed(&args, &failure.corrected(&args)),
    }
}

/// The status mutastic exits with when it couldn't run: 2 for every failure but the tests failing
/// without any mutant until 2.0, when it moves to the Java edition's 2 and 4 (decision 2).
const FAILED: u8 = 2;

/// The status for tests that fail without any mutant, or a test that fails on its own, as the
/// Java edition has it: told apart from a usage error, as there's a way past it.
const TESTS_FAIL: u8 = 3;

/// A panic is a bug in mutastic: it ends as any failure does, said on stderr and, in the agent and
/// jsonl formats, as a `run-failed` event with code `internal` (decision 2), not as Rust's own
/// message and exit 101, which a tool reading the stream can't tell from a crash. A panic in a
/// worker ends the run there, rather than leave the others to finish without it.
fn hook_panics(args: &[String]) {
    let args = args.to_vec();
    std::panic::set_hook(Box::new(move |panic| {
        // failed() says the failure and gives back FAILED, the status exit() ends with here: a
        // hook returns nothing.
        let _ = failed(&args, &crate::failure::internal(panic));
        std::process::exit(i32::from(FAILED));
    }));
}

/// The line with each value given after a space joined to its option, `--format agent` as
/// `--format=agent`, for an option that must take a value, as the Java edition has taken it since
/// 1.0 (decision 1). Only an optional value, `--since`, needs its `=`, as the word after it may be
/// the directory. Joined first, so every reader of the line, from the edition's choice to the
/// directory an error names, sees one form.
fn joined(args: Vec<String>) -> Vec<String> {
    let mut line = Vec::with_capacity(args.len());
    let mut words = args.into_iter().peekable();
    while let Some(word) = words.next() {
        let takes_value = word
            .strip_prefix("--")
            .and_then(commands::option)
            .is_some_and(|option| {
                matches!(
                    option.kind,
                    commands::Kind::Value | commands::Kind::Repeated
                )
            });
        match words.next_if(|next| takes_value && !next.starts_with('-')) {
            Some(value) => line.push(format!("{word}={value}")),
            None => line.push(word),
        }
    }
    line
}

/// mutastic couldn't run: why, on stderr; and in the agent and jsonl formats, the same as the
/// stream's last event, `run-failed`, on stdout, which is all a tool reading the stream reads.
fn failed(args: &[String], failure: &Failure) -> ExitCode {
    let mut failure = failure.clone();
    failure.message = crate::build_args::with_ending(&failure.message);
    let exit = if failure.code == Code::RedBaseline {
        failure.message = way_past(&failure.message, agent_format(args));
        TESTS_FAIL
    } else {
        FAILED
    };
    // A build's or a test's output comes with it: escaped, a line at a time. Its logs are named
    // as typed from here, and linked, after the escaping, which would undo the links.
    stderr_line!(
        "mutastic: {}",
        report::files::logs(&untrusted::escape_lines(&failure.message), linked(args))
    );
    if streams_events(args) {
        stdout_line!(
            "{}",
            report::run_failed(&failure, &directory_typed(args), exit)
        );
    }
    ExitCode::from(exit)
}

/// The tests failing without any mutant, `message`, with the way past them before where their
/// output is, as the Java edition says it, so the commands that run them alone stay last, to copy:
/// for a person, leaving them out with a reason, or for this run; for an agent, never that, which
/// is the person's call, but finding out what's wrong and telling them.
fn way_past(message: &str, agent: bool) -> String {
    let Some((names, _)) = crate::flaky::failing_tests(message) else {
        return message.to_string();
    };
    let line = if agent {
        AGENT_WAY_PAST.to_string()
    } else if names.len() == 1 {
        format!(
            "To run without it until it's fixed, list it under exclude-tests: in mutastic.yaml \
             with a reason, or for this run add --exclude-tests={}",
            names[0]
        )
    } else {
        format!(
            "To run without them until they're fixed, list them under exclude-tests: in \
             mutastic.yaml with a reason, or for this run add --exclude-tests={}",
            names.join(",")
        )
    };
    let output = ["\nIts output is in ", "\nTheir output is in "]
        .iter()
        .filter_map(|said| message.rfind(said))
        .max();
    match output {
        Some(at) => format!("{}\n{line}{}", &message[..at], &message[at..]),
        None => format!("{message}\n{line}"),
    }
}

/// What an agent is told of tests failing without any mutant, in place of the way past them.
const AGENT_WAY_PAST: &str = "Find out whether the change or the test is wrong. Don't leave the \
     test out yourself: tell the person, or propose leaving it out under exclude-tests: in \
     mutastic.yaml with its reason, for them to decide.";

/// Whether the output is the agent format, as [`streams_events`] reads it from the line alone.
fn agent_format(args: &[String]) -> bool {
    let given = args
        .iter()
        .rev()
        .find_map(|arg| arg.strip_prefix("--format="))
        .filter(|format| Format::parse(format).is_some())
        .map(str::to_string)
        .or_else(|| std::env::var("MUTASTIC_FORMAT").ok());
    match given.as_deref() {
        Some(format) => format == "agent",
        None => std::env::var_os("MUTASTIC_AGENT").is_some_and(|v| v == "1"),
    }
}

/// Plain text, with file links when `--hyperlinks` (the last one given) or the terminal says so, as
/// the Java edition decides it: read from the line alone, as a failure can come from parsing it.
fn linked(args: &[String]) -> Style {
    let dumb = std::env::var("TERM").is_ok_and(|term| term == "dumb");
    let links = match args
        .iter()
        .rev()
        .find_map(|arg| arg.strip_prefix("--hyperlinks="))
    {
        Some("always") => true,
        Some("never") => false,
        _ => std::io::stdout().is_terminal() && !dumb,
    };
    Style {
        links,
        ..Style::default()
    }
}

/// Whether the command line, or the environment, asks for the agent or jsonl format, which end in
/// an event when mutastic can't run.
fn streams_events(args: &[String]) -> bool {
    // A --format value mutastic doesn't take is the failure itself: the stream it ends is the one
    // the environment asks for.
    let given = args
        .iter()
        .rev()
        .find_map(|arg| arg.strip_prefix("--format="))
        .filter(|format| Format::parse(format).is_some())
        .map(str::to_string)
        .or_else(|| std::env::var("MUTASTIC_FORMAT").ok());
    match given.as_deref() {
        Some(format) => matches!(format, "agent" | "jsonl"),
        None => std::env::var_os("MUTASTIC_AGENT").is_some_and(|v| v == "1"),
    }
}

/// The directory the command was run on, as typed: its last word that isn't an option, a mutant's
/// id or the command's own, or `.`.
fn directory_typed(args: &[String]) -> String {
    let actions = ["open", "check", "close"];
    args.iter()
        .skip(1)
        .rfind(|word| {
            !word.starts_with('-')
                && !ids(word).all(|id| is_mutant_id(&id))
                && !(args[0] == "gate" && actions.contains(&word.as_str()))
        })
        .cloned()
        .unwrap_or_else(|| ".".to_string())
}

/// Hands the command to the Java edition when the project it points at is a Maven or Gradle
/// project, or `--edition=java` says, and returns its exit status; to the Swift edition likewise,
/// for a Swift package or an Xcode project, which is refused until it's released. `None` when this
/// edition runs it.
fn front_door(
    chosen: Option<Edition>,
    args: &[String],
    build: Option<&[String]>,
) -> Result<Option<ExitCode>, Failure> {
    let Some(command) = args.first() else {
        return Ok(None);
    };
    if matches!(
        command.as_str(),
        "--help" | "-h" | "help" | "--version" | "-V"
    ) {
        return Ok(None);
    }
    let given = edition::pointed_at(args);
    let route = match chosen {
        Some(edition) => Route {
            edition,
            project: None,
            build: None,
        },
        None => edition::route(std::path::Path::new(&given))?,
    };
    if command == "run" {
        if let Some(note) = edition::nested_note(&given, &route) {
            stderr_line!("{note}");
        }
    }
    // A coding agent's gate in a Swift project, with no Swift edition to run, is the gate in a
    // project mutastic can't read: this edition's, which lets the agent go.
    let unread = route.edition == Edition::Swift && command == "gate" && !swift::given();
    if route.edition == Edition::Rust || unread {
        return Ok(None);
    }
    let mut line = args.to_vec();
    if let Some(build) = build {
        line.push("--".to_string());
        line.extend_from_slice(build);
    }
    if route.edition == Edition::Swift {
        return Ok(Some(swift::forward(&swift::engine(&route)?, &line)));
    }
    Ok(Some(java::forward(&java::engine()?, &line)))
}

/// This command's version, and each edition's: the Rust edition is this one, and the Java
/// edition is there once it's been fetched.
fn versions() -> String {
    let own = env!("CARGO_PKG_VERSION");
    let java = match java::present() {
        Some((version, command)) => format!("Java edition {version}, at {}", command.display()),
        None => "Java edition: not fetched yet, which happens the first time mutastic runs in a \
                 Maven or Gradle project"
            .to_string(),
    };
    format!("mutastic {own}\nRust edition {own}, built in\n{java}")
}

#[derive(Clone)]
struct Parsed {
    positional: Vec<String>,
    only: Vec<String>,
    /// `--at`: the lines of source files whose mutants run, or are listed, as typed.
    at: Vec<crate::at::At>,
    format: Option<Format>,
    threshold: f64,
    operators: Vec<&'static str>,
    workers: usize,
    fraction: f64,
    unsafe_mode: UnsafeMode,
    progress: Option<Shown>,
    /// `--vocabulary`: the words for a mutant's fate, `MUTASTIC_VOCABULARY`'s unless given.
    vocabulary: Option<report::Vocabulary>,
    /// `--color` and `--hyperlinks`: `None` is `auto`, `Some(true)` `always`, `Some(false)` `never`.
    color: Option<bool>,
    hyperlinks: Option<bool>,
    /// The report files to write, from `--report`: `None` for the defaults.
    reports: Option<Vec<(Kind, Option<String>)>>,
    /// `ignore`'s: why, what sort of decision (`--as`), and what to do with it.
    reason: Option<String>,
    kind: String,
    propose: bool,
    accept: bool,
    remove: bool,
    prune: bool,
    /// `--since`: the commit the change is measured from, `""` for the default branch.
    since: Option<String>,
    /// `--operators`, as given, which replaces `mutastic.yaml`'s `operators:`.
    operators_given: Option<String>,
    /// The operators the run chose, as it chose them, `stronger`: `--operators`, else
    /// `mutastic.yaml`'s, else the defaults. A baseline keeps them.
    operators_chosen: String,
    /// The operators a run would choose without `--operators`: `mutastic.yaml`'s, else the
    /// defaults. The gate's open tells the agent to run with `--operators` when its set isn't these.
    run_operators: Vec<&'static str>,
    /// `mutastic.yaml`'s `gate:` `operators:`, as written, each with where: the gate's set unless
    /// `--operators` chose.
    gate_configured: Vec<String>,
    gate_configured_from: Vec<String>,
    /// `mutastic.yaml`'s rules, once read.
    rules: Vec<crate::config::Rule>,
    /// `gate`'s: `--help` for a subcommand, `--hook=claude`, and `--tries`.
    help: bool,
    hook: String,
    tries: usize,
    /// `--no-suggestions`: no test is looked for, so none of the project's code runs outside its
    /// tests.
    no_suggestions: bool,
    /// `--force-coverage`: the build and the coverage pass run even with no mutant to run.
    force_coverage: bool,
    /// `--confirm-catches` and `--kill-matrix`: how a caught mutant's tests run.
    catches: engine::Catches,
    /// `--exclude-tests`: the tests the run leaves out, as given.
    exclude_tests: Vec<String>,
    /// mutastic.yaml's `exclude-tests:`: the tests every run leaves out, each with its reason.
    configured_tests: Vec<crate::config::ExcludedTest>,
    /// mutastic.yaml's `timeout-ceiling:`.
    timeout_ceiling: Option<std::time::Duration>,
    /// `--each-mutant`: a card for every missed mutant, not one for each line's.
    each_mutant: bool,
    /// `--shard`: the share of the mutants this run runs.
    shard: Option<crate::shares::Shard>,
    /// `--from-shards`: the directory of the shares this run is made of.
    from_shards: Option<String>,
    /// `--top`: how many of `next`'s queue to show.
    top: usize,
    /// `--fail-on-new`: the run passes on the misses the baseline doesn't hold, not its score.
    fail_on_new: bool,
    /// `mutants --estimate`: measure the tests first, and estimate what each set would take.
    estimate: bool,
}

/// The command as it's declared: `gate open` for `gate` and its action. `None` for one that isn't,
/// such as `--help`, or `gate` with an action it doesn't have, which is an error of its own.
fn declared(command: &str, args: &[String]) -> Option<&'static commands::Command> {
    if command == "gate" {
        let action = args.iter().find(|arg| !arg.starts_with("--"))?;
        return commands::command(&format!("gate {action}"));
    }
    commands::command(command)
}

/// `command`'s options and words. An option is taken only as [`commands`] declares it, on the
/// commands it names there, so what the docs say a command takes is what it takes.
fn parse(command: &str, args: &[String]) -> Result<Parsed, Failure> {
    let declared = declared(command, args);
    let mut parsed = Parsed {
        positional: Vec::new(),
        only: Vec::new(),
        at: Vec::new(),
        format: None,
        vocabulary: None,
        color: None,
        hyperlinks: None,
        threshold: 80.0,
        operators: sites::DEFAULTS.to_vec(),
        workers: testrun::default_workers(),
        fraction: 1.0,
        unsafe_mode: UnsafeMode::Skip,
        progress: None,
        reports: None,
        reason: None,
        kind: String::new(),
        propose: false,
        accept: false,
        remove: false,
        prune: false,
        since: None,
        operators_given: None,
        operators_chosen: "defaults".to_string(),
        run_operators: sites::DEFAULTS.to_vec(),
        gate_configured: Vec::new(),
        gate_configured_from: Vec::new(),
        rules: Vec::new(),
        help: false,
        hook: String::new(),
        tries: 3,
        no_suggestions: false,
        force_coverage: false,
        catches: engine::Catches::default(),
        exclude_tests: Vec::new(),
        configured_tests: Vec::new(),
        timeout_ceiling: None,
        each_mutant: false,
        shard: None,
        from_shards: None,
        top: 1,
        fail_on_new: false,
        estimate: false,
    };
    // The line as typed, the command first, for an error to give back with one word changed.
    let line: Vec<String> = std::iter::once(command.to_string())
        .chain(args.iter().cloned())
        .collect();
    for (index, arg) in args.iter().enumerate() {
        let at = index + 1;
        let Some(option) = arg.strip_prefix("--") else {
            if arg == "-h" {
                parsed.help = true;
                continue;
            }
            if arg.len() > 1 && arg.starts_with('-') {
                return Err(Code::UnknownOption.with(short_option(command, arg)));
            }
            parsed.positional.push(arg.clone());
            continue;
        };
        let (name, value) = match option.split_once('=') {
            Some((name, value)) => (name, Some(value)),
            None => (option, None),
        };
        // `--help` is every command's, and `--fraction` the measuring `build`'s, not a user's.
        let internal = name == "help" || (name == "fraction" && command == "build");
        if !internal {
            let Some(declared_option) = commands::option(name) else {
                return Err(Code::UnknownOption
                    .with(crate::mistyped::option(declared, &line, at, name, value)));
            };
            commands::accepts(declared, name, value)?;
            if value.is_none() {
                if let Some(error) = crate::mistyped::value(declared_option, &line, at) {
                    return Err(Code::OptionValue.with(error));
                }
            }
            if let Some(value) = value {
                let kind = value.split_once(':').map_or(value, |(kind, _)| kind);
                let chosen = if name == "report" { kind } else { value };
                if !declared_option.choices.is_empty() && !declared_option.choices.contains(&chosen)
                {
                    return Err(Code::OptionValue.with(crate::mistyped::choice(
                        declared_option,
                        &line,
                        at,
                        chosen,
                    )));
                }
            }
        }
        let value = value.unwrap_or("");
        match name {
            "from-shards" if value.is_empty() => {
                return Err(Code::Shards
                    .with("--from-shards needs the shares' directory: --from-shards=DIR"))
            }
            "shard" => parsed.shard = Some(crate::shares::Shard::parse(value)?),
            _ => {
                let known =
                    set(&mut parsed, name, value).map_err(|error| Code::OptionValue.with(error))?;
                if !known {
                    return Err(Code::UnknownOption
                        .with(crate::mistyped::option(declared, &line, at, name, None)));
                }
            }
        }
    }
    Ok(parsed)
}

/// Sets option `name` to `value` (empty for a flag), checked as that option takes it; false for
/// a name no option has. Every error is a value the option doesn't take.
fn set(parsed: &mut Parsed, name: &str, value: &str) -> Result<bool, String> {
    match name {
        "only" => {
            if value.trim().is_empty() {
                return Err("--only needs a mutant's id: --only=ID, as a card shows it".into());
            }
            if value.split(',').any(|id| id.trim().is_empty()) {
                return Err(format!(
                    "--only={value} has an empty id between its commas: --only=ID,ID"
                ));
            }
            parsed.only.extend(ids(value))
        }
        "at" => parsed.at.extend(crate::at::parse(value)?),
        "format" => {
            parsed.format = Some(Format::parse(value).ok_or_else(|| {
                crate::mistyped::not_one_of(
                    "--format",
                    value,
                    &["full", "brief", "concise", "grouped", "agent", "jsonl"],
                )
            })?)
        }
        "threshold" => parsed.threshold = f64::from(whole("--threshold", value, 0, 100)?),
        "workers" => {
            parsed.workers = match value.parse::<usize>() {
                Ok(0) => return Err("--workers is at least 1, not 0".into()),
                Ok(workers) => workers,
                Err(_) => return Err(format!("--workers needs a whole number, not {value}")),
            }
        }
        "operators" => {
            // Given again, it adds, as `--report` does: `--operators=stronger --operators=-stream`.
            let given = match parsed.operators_given.take() {
                Some(earlier) => format!("{earlier},{value}"),
                None => value.to_string(),
            };
            // A name only a rules file has is checked once the rules files are read.
            if let Ok(chosen) = operators(&given) {
                parsed.operators = chosen;
            }
            parsed.operators_given = Some(given);
        }
        "unsafe" => {
            parsed.unsafe_mode = UnsafeMode::parse(value).ok_or_else(|| {
                crate::mistyped::not_one_of("--unsafe", value, &["skip", "enclosing", "all"])
            })?
        }
        "reason" => parsed.reason = Some(value.to_string()),
        "as" => {
            if !ignores::KINDS.contains(&value) {
                return Err(crate::mistyped::not_one_of("--as", value, ignores::KINDS));
            }
            parsed.kind = value.to_string()
        }
        "propose" => parsed.propose = true,
        "accept" => parsed.accept = true,
        "remove" => parsed.remove = true,
        "prune" => parsed.prune = true,
        "fail-on-new" => parsed.fail_on_new = true,
        "estimate" => parsed.estimate = true,
        "since" => parsed.since = Some(value.to_string()),
        "help" => parsed.help = true,
        "no-suggestions" => parsed.no_suggestions = true,
        "force-coverage" => parsed.force_coverage = true,
        "confirm-catches" => parsed.catches.confirm = true,
        "kill-matrix" => parsed.catches.every = true,
        "each-mutant" => parsed.each_mutant = true,
        // `--exclude-tests=tests::races,ports`: split at commas, as the Java edition takes them.
        "exclude-tests" => parsed.exclude_tests.extend(
            value
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string),
        ),
        "from-shards" => parsed.from_shards = Some(value.to_string()),
        "hook" => {
            if value != "claude" {
                return Err(crate::mistyped::not_one_of("--hook", value, &["claude"]));
            }
            parsed.hook = value.to_string();
        }
        "top" => {
            let most = u32::try_from(report::next::MOST).unwrap_or(u32::MAX);
            parsed.top = whole("--top", value, 1, most)? as usize
        }
        "tries" => parsed.tries = whole("--tries", value, 1, 100)? as usize,
        "report" => {
            let reports = parsed.reports.get_or_insert_with(Vec::new);
            let (kind, path) = match value.split_once(':') {
                Some((kind, path)) => (kind, Some(path.to_string())),
                None => (value, None),
            };
            if kind != "none" {
                let kind = Kind::parse(kind).ok_or_else(|| {
                    format!("--report: unknown report {kind} (one of json, html, sarif, none)")
                })?;
                reports.push((kind, path));
            }
        }
        "vocabulary" => {
            parsed.vocabulary = Some(report::Vocabulary::parse(value).ok_or_else(|| {
                crate::mistyped::not_one_of("--vocabulary", value, &["plain", "classic"])
            })?)
        }
        "color" => parsed.color = choice("--color", value)?,
        "hyperlinks" => parsed.hyperlinks = choice("--hyperlinks", value)?,
        "progress" => {
            parsed.progress = Shown::parse(value).ok_or_else(|| {
                crate::mistyped::not_one_of(
                    "--progress",
                    value,
                    &["auto", "bar", "counter", "none"],
                )
            })?
        }
        // Which edition runs is decided before this one reads its options.
        "edition" => {
            Edition::parse(value).ok_or_else(|| {
                crate::mistyped::not_one_of("--edition", value, &["java", "rust", "swift"])
            })?;
        }
        "fraction" => {
            parsed.fraction = value
                .parse()
                .map_err(|_| format!("--fraction: {value} isn't a number"))?
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// `value` as a whole number from `min` to `max`, in the Java edition's words: a score is a whole
/// percentage, as the schema and mutation.json's thresholds have it.
fn whole(option: &str, value: &str, min: u32, max: u32) -> Result<u32, String> {
    match (value.parse::<u32>(), value.parse::<i64>()) {
        (Ok(number), _) if (min..=max).contains(&number) => Ok(number),
        (_, Ok(number)) => Err(format!("{option} is from {min} to {max}, not {number}")),
        _ => Err(format!("{option} needs a whole number, not {value}")),
    }
}

/// A word such as `-v`, where a directory goes: mutastic's options are all long ones, so it's an
/// option mutastic doesn't have, not a directory.
fn short_option(command: &str, word: &str) -> String {
    format!(
        "{command} has no option {word}.\n\
         mutastic's options are long ones, such as --format=full. To see them all:\n  mutastic {command} --help"
    )
}

/// Whether `args` (without the program's name) is a command mutastic takes, with options it knows,
/// without running it: for checking the commands the docs show.
pub fn check(args: &[String]) -> Result<(), String> {
    let (args, _) = crate::build_args::split_line(args);
    let args = args.as_slice();
    checked(args).map_err(|failure| failure.message)
}

fn checked(args: &[String]) -> Result<(), Failure> {
    let args = joined(args.to_vec());
    let Some(command) = args.first() else {
        return Err(Code::UnknownCommand.with("no command"));
    };
    match command.as_str() {
        "run" | "verify" | "explain" | "ignore" | "gate" | "mutants" | "next" | "baseline"
        | "help" | "--help" | "-h" | "--version" | "-V" => {
            let parsed = parse(command, &args[1..])?;
            arguments(command, &parsed)?;
            match &parsed.operators_given {
                Some(given) => operators(given).map(|_| ()),
                None => Ok(()),
            }
        }
        _ => Err(Code::UnknownCommand.with(crate::mistyped::command(&args))),
    }
}

/// Whether the words that aren't options are ones `command` takes: a mutant's id where it needs
/// one, then a directory, and nothing after. A word it doesn't take would otherwise be dropped
/// without a word, and the run would be one nobody asked for.
fn arguments(command: &str, parsed: &Parsed) -> Result<(), Failure> {
    let synopsis = |name: &str| commands::command(name).map_or("", |command| command.synopsis);
    let (usage, takes_id, most) = match command {
        "run" | "mutants" | "next" | "baseline" => (synopsis(command), false, 1),
        "build" => ("mutastic build [DIR] --fraction=SHARE", false, 1),
        "verify" => (
            synopsis(command),
            true,
            1 + verify_ids(&parsed.positional).1,
        ),
        "explain" => (synopsis(command), true, 2),
        "ignore" if parsed.prune => ("mutastic ignore --prune [DIR]", false, 1),
        "ignore" => (synopsis(command), true, 2),
        "gate" => ("mutastic gate open|check|close [DIR]", false, 2),
        _ => return Ok(()),
    };
    if command == "gate" && !parsed.help {
        gate_action(parsed)?;
    }
    if command == "ignore" && parsed.propose && (parsed.accept || parsed.remove || parsed.prune) {
        return Err(Code::OptionValue
            .with("--propose records a proposal: not with --accept, --remove or --prune"));
    }
    if command == "run" {
        if !parsed.only.is_empty() && !parsed.at.is_empty() {
            return Err(Code::OptionNotTaken.with(
                "--only and --at both choose the mutants that run: give one, the ids or the lines",
            ));
        }
        sharing(parsed)?;
        if let Some(choosing) = choosing(parsed).filter(|_| parsed.fail_on_new) {
            return Err(Code::OptionNotTaken.with(format!(
                "--fail-on-new judges a run of the package on its new misses: not with \
                 {choosing}, whose mutants each pass when they're caught"
            )));
        }
    }
    if takes_id {
        if let Some(given) = parsed.positional.first() {
            let mut named = ids(given).peekable();
            if named.peek().is_none() || !named.all(|id| is_mutant_id(&id)) {
                return Err(Code::NeedsId.with(needs_id(command, given)));
            }
        }
    }
    match parsed.positional.get(most..) {
        Some(extra) if !extra.is_empty() => Err(Code::ExtraWords.with(format!(
            "{command} doesn't take {}: {usage}",
            extra.join(" ")
        ))),
        _ => Ok(()),
    }
}

/// The option that chose some of the mutants, `--only` or `--at`; none when neither did.
fn choosing(parsed: &Parsed) -> Option<&'static str> {
    if !parsed.only.is_empty() {
        Some("--only")
    } else if !parsed.at.is_empty() {
        Some("--at")
    } else {
        None
    }
}

/// Whether the run is of some mutants, asked for by id (`--only`) or by their lines (`--at`): each
/// one's fate is its result, not a score, and it keeps no record and writes no report.
fn partial(parsed: &Parsed) -> bool {
    choosing(parsed).is_some()
}

/// A run is a share or made of them, not both; made of them, it runs no mutants itself, so the
/// options for what runs are the shares'.
fn sharing(parsed: &Parsed) -> Result<(), Failure> {
    if parsed.from_shards.is_none() {
        return Ok(());
    }
    let given = [
        ("--shard", parsed.shard.is_some()),
        ("--only", !parsed.only.is_empty()),
        ("--at", !parsed.at.is_empty()),
        ("--since", parsed.since.is_some()),
        ("--force-coverage", parsed.force_coverage),
    ];
    match given.iter().find(|(_, given)| *given) {
        Some((option, _)) => Err(Code::Shards.with(format!(
            "--from-shards puts the shares' results together as the whole run, and runs no mutant \
             itself: not with {option}, which the shares run with"
        ))),
        None => Ok(()),
    }
}

/// `verify`'s ids, as the Java edition takes them: the first word, split at its commas as a card's
/// `done when` writes them, then each word after it that's an id; and how many words they took.
/// The directory, if any, comes after them.
fn verify_ids(positional: &[String]) -> (Vec<String>, usize) {
    let Some(first) = positional.first() else {
        return (Vec::new(), 0);
    };
    let mut named: Vec<String> = ids(first).collect();
    let more = positional[1..]
        .iter()
        .take_while(|word| is_mutant_id(word))
        .count();
    named.extend(positional[1..=more].iter().cloned());
    (named, 1 + more)
}

/// A mutant's id, as cards and `mutastic mutants` show it: `m` and eight hex digits.
fn is_mutant_id(word: &str) -> bool {
    word.strip_prefix('m')
        .is_some_and(|hex| hex.len() == 8 && hex.bytes().all(|digit| digit.is_ascii_hexdigit()))
}

/// What's wrong with the word where `command` needs a mutant's id, in the Java edition's words,
/// and the command with one to fill in: one that keeps the directory, when that's what was given.
fn needs_id(command: &str, given: &str) -> String {
    if std::path::Path::new(given).is_dir() {
        format!(
            "{given} is a directory, not a mutant id. {}",
            id_first(command, Some(given))
        )
    } else {
        format!("{given} isn't a mutant id. {}", id_first(command, None))
    }
}

/// What `command` needs first, as a run names a mutant, and the command with it to fill in, with
/// `dir` after it, and for `ignore` its reason.
fn id_first(command: &str, dir: Option<&str>) -> String {
    let dir = dir.map_or(String::new(), |dir| format!(" {}", shell(dir)));
    let reason = if command == "ignore" {
        " --reason=\"<why>\""
    } else {
        ""
    };
    format!(
        "{command} needs a mutant id first, an m and eight hex digits as a run prints them:\n  \
         mutastic {command} <id>{dir}{reason}"
    )
}

fn ids(value: &str) -> impl Iterator<Item = String> + '_ {
    value
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

fn operators(value: &str) -> Result<Vec<&'static str>, Failure> {
    let entries: Vec<String> = value.split(',').map(|e| e.trim().to_string()).collect();
    crate::config::resolve(&entries, "--operators")
}

/// Reads the package's nearest `mutastic.yaml`, if there is one: its rules, its rules files'
/// operators, and the operators it chooses, unless `--operators` chose.
fn configure(command: &str, parsed: &mut Parsed) -> Result<(), Failure> {
    let at = dir_slot(command, parsed).unwrap_or(1);
    let given = parsed.positional.get(at).map_or(".", String::as_str);
    let config = match configured_dir(given) {
        Some(dir) => crate::config::Configuration::read(&dir)?,
        None => crate::config::Configuration::default(),
    };
    let mut rules = Vec::new();
    for (plugin, from) in config.plugins.iter().zip(&config.plugins_from) {
        rules.extend(crate::callrules::read(plugin, from)?);
    }
    crate::callrules::load(rules);
    crate::build_args::settle(config.build_args.as_deref())?;
    parsed.rules = config.rules;
    if !config.operators.is_empty() {
        parsed.run_operators =
            crate::config::resolve_from(&config.operators, &config.operators_from)?;
    }
    parsed.gate_configured = config.gate_operators;
    parsed.gate_configured_from = config.gate_operators_from;
    parsed.configured_tests = config.exclude_tests;
    parsed.timeout_ceiling = config.timeout_ceiling;
    match &parsed.operators_given {
        Some(given) => {
            parsed.operators = operators(given)?;
            parsed.operators_chosen = chosen(given.split(','));
        }
        None if !config.operators.is_empty() => {
            parsed.operators =
                crate::config::resolve_from(&config.operators, &config.operators_from)?;
            parsed.operators_chosen = chosen(config.operators.iter().map(String::as_str));
        }
        None => {}
    }
    Ok(())
}

/// Operators as chosen, `stronger,-stream`: each word trimmed, split by commas.
fn chosen<'w>(words: impl Iterator<Item = &'w str>) -> String {
    words
        .map(str::trim)
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

/// Where `command`'s directory goes among the words that aren't options: after `verify`'s ids,
/// `explain`'s or `ignore`'s id and `gate`'s action, first for the rest. `None` where it names no
/// directory, or lacks the words before one, which it says itself.
fn dir_slot(command: &str, parsed: &Parsed) -> Option<usize> {
    let words = parsed.positional.len();
    match command {
        "run" | "mutants" | "next" | "baseline" => Some(0),
        "ignore" if parsed.prune => Some(0),
        "verify" if words > 0 => Some(verify_ids(&parsed.positional).1),
        "gate" if parsed.help => None,
        "explain" | "ignore" | "gate" if words > 0 => Some(1),
        _ => None,
    }
}

/// Checks the directory `command` runs in before anything runs there, and puts the package's in
/// its place where that's another: below a package, it's the package's; a package's name typed
/// where there's no such directory is that package's, said in a note. The ignores, the baseline,
/// `mutastic.yaml` and `--since` are then all read from the package's directory.
fn locate(command: &str, args: &[String], parsed: &mut Parsed) -> Result<(), Failure> {
    let Some(slot) = dir_slot(command, parsed) else {
        return Ok(());
    };
    let typed = parsed
        .positional
        .get(slot)
        .map_or(".", String::as_str)
        .to_string();
    // At a workspace's root, `--at`'s file picks the member, as a directory inside one does.
    let picked = matches!(command, "run" | "mutants")
        .then(|| crate::at::member(&typed, &parsed.at))
        .flatten()
        .map(|member| crate::locate::shown(&typed, &crate::locate::here(), &member));
    let given = picked.as_deref().unwrap_or(&typed);
    let before: Vec<&str> = std::iter::once(command)
        .chain(parsed.positional.iter().take(slot).map(String::as_str))
        .collect();
    let options: Vec<String> = args
        .iter()
        .skip(1)
        .filter(|arg| arg.starts_with("--"))
        .map(|arg| shell(arg))
        .collect();
    let line = crate::locate::Line::new(&before.join(" "), &options);
    let located = crate::locate::resolve(given, &line)?;
    if let Some(note) = &located.note {
        stderr_line!("{note}");
    }
    if located.dir != typed {
        match parsed.positional.get_mut(slot) {
            Some(word) => *word = located.dir,
            None => parsed.positional.push(located.dir),
        }
    }
    Ok(())
}

/// The directory of the package's nearest `mutastic.yaml`, as a path from `given`, the directory
/// the run is on, so what it names reads as it did there. The package is the one `given` names,
/// which may be a workspace's top with one package below it; where `given` isn't a package, the
/// command says so itself, and the search starts at `given`.
fn configured_dir(given: &str) -> Option<std::path::PathBuf> {
    let run = std::fs::canonicalize(given).ok()?;
    let package = workspace::read(given)
        .map(|project| project.workspace.join(&project.package_dir))
        .unwrap_or_else(|_| run.clone());
    let found = crate::config::nearest(&package, &run)?;
    let from_run = crate::config::relative(&run, &found);
    Some(std::path::Path::new(given).join(from_run))
}

fn dispatch(args: &[String]) -> Result<ExitCode, Failure> {
    // With nothing to do, the help goes where errors go: stdout holds only results.
    let Some(command) = args.first() else {
        stderr_line!("{}", crate::help::general());
        return Ok(ExitCode::from(2));
    };
    if let Some(help) = help(args)? {
        stdout_line!("{help}");
        return Ok(ExitCode::SUCCESS);
    }
    if !crate::mistyped::COMMANDS.contains(&command.as_str())
        && !matches!(command.as_str(), "build" | "--version" | "-V")
    {
        return Err(Code::UnknownCommand.with(crate::mistyped::command(args)));
    }
    let mut parsed = parse(command, &args[1..])?;
    arguments(command, &parsed)?;
    if let Some(declared) = declared(command, &args[1..]) {
        let given = |option: &str| {
            let typed = format!("--{option}=");
            args.iter().any(|arg| arg.starts_with(&typed))
        };
        crate::environment::check(declared.name, given, |name| std::env::var(name).ok())?;
    }
    if let Err(error) = locate(command, args, &mut parsed) {
        // A hook runs in whatever project the agent is in: where mutastic reads no Cargo package,
        // there's nothing to gate, and it says nothing.
        if command == "gate" && !parsed.hook.is_empty() {
            return Ok(ExitCode::SUCCESS);
        }
        return Err(error);
    }
    if matches!(
        command.as_str(),
        "run" | "verify" | "explain" | "ignore" | "mutants" | "gate" | "next" | "baseline"
    ) {
        configure(command, &mut parsed)?;
    }
    match command.as_str() {
        "run" => run(&parsed),
        "verify" => verify(&parsed),
        "explain" => explain(&parsed),
        "ignore" => ignore(&parsed),
        "gate" => gate_command(&parsed),
        "mutants" => mutants(&parsed),
        "next" => next(&parsed),
        "baseline" => baseline_command(&parsed),
        // For measuring the schema itself: build with a share of the mutants, and say how it went.
        "build" => build(&parsed),
        "--version" | "-V" => {
            stdout_line!("{}", versions());
            Ok(ExitCode::SUCCESS)
        }
        _ => Err(Code::UnknownCommand.with(crate::mistyped::command(args))),
    }
}

/// What `--help` asks for, when it's asked: mutastic's usage, or one command's, for `help run`,
/// `run --help` or `run -h`, before anything runs. `gate --help` without its action is each
/// action's.
fn help(args: &[String]) -> Result<Option<String>, Failure> {
    let asks = |word: &String| word == "--help" || word == "-h";
    let (command, rest) = match args.first().map(String::as_str) {
        Some("help" | "--help" | "-h") => match args.get(1) {
            Some(command) => (command, &args[2..]),
            None => return Ok(Some(crate::help::general())),
        },
        Some(_) if args[1..].iter().any(asks) => (&args[0], &args[1..]),
        _ => return Ok(None),
    };
    if command == "gate" {
        let action = rest.iter().find(|word| !word.starts_with('-'));
        let text = match action {
            Some(action) => gate_help(action)?,
            None => ["open", "check", "close"]
                .iter()
                .filter_map(|action| commands::command(&format!("gate {action}")))
                .map(crate::help::of)
                .collect::<Vec<_>>()
                .join("\n\n"),
        };
        return Ok(Some(text));
    }
    match commands::command(command) {
        Some(declared) => Ok(Some(crate::help::of(declared))),
        None if args[0] == *command => Ok(None),
        None => Err(Code::UnknownCommand.with(crate::mistyped::command_at(args, 1))),
    }
}

fn format(parsed: &Parsed) -> Format {
    parsed
        .format
        .or_else(|| {
            std::env::var("MUTASTIC_FORMAT")
                .ok()
                .and_then(|name| Format::parse(&name))
        })
        .unwrap_or_else(|| {
            if std::env::var_os("MUTASTIC_AGENT").is_some_and(|v| v == "1") {
                Format::Agent
            } else if std::io::stdout().is_terminal() {
                Format::Full
            } else {
                Format::Concise
            }
        })
}

/// `auto` as `None`, `always` as `Some(true)`, `never` as `Some(false)`.
fn choice(option: &str, value: &str) -> Result<Option<bool>, String> {
    match value {
        "auto" => Ok(None),
        "always" => Ok(Some(true)),
        "never" => Ok(Some(false)),
        _ => Err(crate::mistyped::not_one_of(
            option,
            value,
            &["auto", "always", "never"],
        )),
    }
}

/// The style for a stream, `terminal` when it's one, as the Java edition chooses it. Colour:
/// `--color`, or on a terminal unless `NO_COLOR` is set or `TERM=dumb`, and anywhere `FORCE_COLOR`
/// asks. Links: `--hyperlinks`, or on a terminal that isn't `TERM=dumb`. Words: `--vocabulary`,
/// `MUTASTIC_VOCABULARY`, or plain.
fn style(parsed: &Parsed, terminal: bool) -> Style {
    let dumb = std::env::var("TERM").is_ok_and(|term| term == "dumb");
    let color = parsed.color.unwrap_or_else(|| {
        std::env::var_os("FORCE_COLOR").is_some()
            || (terminal && std::env::var_os("NO_COLOR").is_none() && !dumb)
    });
    let links = parsed.hyperlinks.unwrap_or(terminal && !dumb);
    let words = parsed
        .vocabulary
        .or_else(|| {
            std::env::var("MUTASTIC_VOCABULARY")
                .ok()
                .and_then(|name| report::Vocabulary::parse(&name))
        })
        .unwrap_or_default();
    // Checked before anything runs: one it doesn't take is refused there.
    let labels = std::env::var("MUTASTIC_PILLS")
        .ok()
        .and_then(|name| report::Labels::parse(&name))
        .unwrap_or_default();
    Style {
        color,
        links,
        words,
        labels,
        each_mutant: parsed.each_mutant,
    }
}

/// The run's progress on stderr, started before the package is read, so the first line comes
/// before anything slow. `files`: each source file's line as its last mutant finishes.
fn start(parsed: &Parsed, files: bool) -> Progress {
    let shown = crate::progress::resolve(parsed.progress);
    let style = style(parsed, std::io::stderr().is_terminal());
    // With --fail-on-new the new misses decide, not the score.
    let threshold = (!parsed.fail_on_new).then_some(parsed.threshold);
    Progress::start(shown, style, (files, parsed.workers), threshold)
}

fn options(parsed: &Parsed, only: Vec<String>) -> Options {
    Options {
        only,
        among: None,
        operators: parsed.operators.clone(),
        threshold: parsed.threshold,
        workers: parsed.workers,
        unsafe_mode: parsed.unsafe_mode,
        since: None,
        rules: parsed.rules.clone(),
        suggestions: false,
        shard: None,
        force_coverage: parsed.force_coverage,
        catches: parsed.catches,
        exclude_tests: engine::LeftOut::listed(&parsed.configured_tests, &parsed.exclude_tests),
        audit_arid: std::env::var("MUTASTIC_ARID").is_ok_and(|value| value == "audit"),
        timeout_ceiling: parsed.timeout_ceiling,
        history: crate::watch::History::default(),
    }
}

fn run(parsed: &Parsed) -> Result<ExitCode, Failure> {
    no_baseline_to_fail_on(parsed)?;
    if let Some(shares) = &parsed.from_shards {
        return from_shards(parsed, shares);
    }
    let dir = parsed.positional.first().map_or(".", String::as_str);
    let format = format(parsed);
    let progress = start(parsed, true);
    let project = workspace::read(dir)?;
    progress.package(&project.package);
    // A share's events and report files go in a folder of its own: its report is only its share's,
    // so `explain` and the next whole run never take it for the whole run's.
    let folder = parsed
        .shard
        .map(|shard| shard.folder(&project.mutastic_dir()));
    let share = match (parsed.shard, &folder) {
        (Some(shard), Some(folder)) => {
            crate::shares::forget(&folder.join(EVENTS))?;
            Some(crate::shares::Tag::new(
                shard,
                crate::shares::share_code(&project)?,
            ))
        }
        _ => None,
    };
    let mut options = options(parsed, parsed.only.clone());
    options.suggestions = !parsed.no_suggestions;
    options.shard = parsed.shard;
    if let Some(since) = &parsed.since {
        options.since = Some(crate::since::changed_since(
            std::path::Path::new(dir),
            since,
        )?);
    }
    let at = on_the_lines(parsed, &project, &mut options)?;
    let records = folder.clone().unwrap_or_else(|| project.mutastic_dir());
    options.history = history(parsed, &project, &options, &records)?;
    if format == Format::Jsonl {
        progress.stream(crate::progress::Events {
            unsafe_mode: parsed.unsafe_mode,
            share: share.as_ref().map(crate::shares::Tag::json),
            at: at.iter().map(crate::at::Lines::typed).collect(),
        });
    }
    // A share's events as the run goes, so a share stopped part way keeps what it finished.
    if let (Some(tag), Some(folder)) = (&share, &folder) {
        progress.keep_live(
            &folder.join(EVENTS),
            crate::progress::Events {
                unsafe_mode: parsed.unsafe_mode,
                share: Some(tag.json()),
                at: at.iter().map(crate::at::Lines::typed).collect(),
            },
        );
    }
    let mut report = engine::run(&project, &options, &|step| progress.step(step))?;
    report.share = share;
    report.at = at;
    progress.finished();
    // Only a whole run of every test measured every site its mutants are at, as `mutants`
    // estimates from them.
    let whole = !partial(parsed) && parsed.since.is_none() && parsed.shard.is_none();
    if whole && parsed.exclude_tests.is_empty() && report.built {
        keep_test_times(&report);
    }
    report.streamed = progress.streamed();
    finish(parsed, report, format, folder.as_deref())
}

/// What earlier runs say of the tests' catches (decision 27): the package's watch, and, for a run of
/// every mutant (or a share), the last comparable run's results in `records` when nothing it ran
/// has changed since. A run of some mutants, or of a change, is compared with no run, as its record
/// isn't kept.
fn history(
    parsed: &Parsed,
    project: &workspace::Project,
    options: &Options,
    records: &std::path::Path,
) -> Result<crate::watch::History, Failure> {
    let watched = crate::watch::read(&project.mutastic_dir())
        .into_keys()
        .collect();
    if partial(parsed) || parsed.since.is_some() {
        return Ok(crate::watch::History {
            same_code: std::collections::BTreeMap::new(),
            watched,
        });
    }
    let code = crate::record::code_without(
        &crate::shares::code(project)?,
        &engine::LeftOut::names(&options.exclude_tests),
    );
    let code =
        crate::record::code_built_with(&code, &crate::build_args::key(&project.mutastic_dir())?);
    let now = crate::record::before(project, &options.operators, &code, &crate::record::tool());
    let last = crate::records::RunRecords::new(records)
        .comparable_to(&now)
        .filter(|last| now.same_code_and_tool_as(last));
    Ok(crate::watch::History {
        same_code: last.map(|last| last.results).unwrap_or_default(),
        watched,
    })
}

/// `options` confirming each catch a watched test makes [`crate::watch::CONFIRMATIONS`] more times,
/// as every command that runs mutants does (decision 27). Only `run` of every mutant compares with
/// the last run's results too ([`history`]); none but `run` writes the watch.
fn watching(options: &mut Options, project: &workspace::Project) {
    options.history.watched = crate::watch::read(&project.mutastic_dir())
        .into_keys()
        .collect();
}

/// The watch of likely flaky tests after `report` (decision 27), and what became of each test it
/// watched: a full run of the package, or the `--from-shards` merge, keeps the watch it leaves; a
/// share or a run of some mutants confirms against it and leaves it as it was.
fn watched(parsed: &Parsed, report: &mut engine::Report) -> Result<(), Failure> {
    let folder = report.project.mutastic_dir();
    let before = crate::watch::read(&folder);
    let keeps =
        report.share.is_none() && !partial(parsed) && report.scope.is_none() && report.built;
    if !keeps {
        report.watched = crate::watch::unchanged(&before, report);
        return Ok(());
    }
    let (after, watched) = crate::watch::after(&before, report);
    if after != before {
        crate::watch::write(&folder, &after).map_err(|error| {
            crate::failure::disk(format!(
                "couldn't keep the watch of likely flaky tests in {}: {error}",
                folder.display()
            ))
        })?;
    }
    report.watched = watched;
    Ok(())
}

/// With `--at`, the lines asked for, and `options` choosing only the mutants on them, of those the
/// run would have: under its operators, and with `--since` on the change. Lines with none on them
/// are refused.
fn on_the_lines(
    parsed: &Parsed,
    project: &workspace::Project,
    options: &mut Options,
) -> Result<Vec<crate::at::Lines>, Failure> {
    if parsed.at.is_empty() {
        return Ok(Vec::new());
    }
    let (found, _) = engine::find_for_run(project, options)?;
    let lines = crate::at::files(&parsed.at, project, &found)?;
    let on_the_change = |mutant: &crate::sites::Mutant| {
        options.since.as_ref().is_none_or(|changed| {
            changed.touches(&project.workspace.join(&mutant.file), mutant.line)
        })
    };
    let ids = crate::at::on(&lines, &found, on_the_change, &project.given)?;
    options.among = Some(ids);
    Ok(lines)
}

/// `--fail-on-new` with no baseline to tell new misses from: every miss would be new, or none, and
/// either would be a guess. Said before the package is read.
fn no_baseline_to_fail_on(parsed: &Parsed) -> Result<(), Failure> {
    let dir = parsed.positional.first().map_or(".", String::as_str);
    if !parsed.fail_on_new || baseline::exists(std::path::Path::new(dir)) {
        return Ok(());
    }
    let shown = std::fs::canonicalize(dir)
        .map_or_else(|_| dir.to_string(), |path| path.display().to_string());
    Err(Code::NoBaseline.with(format!(
        "--fail-on-new needs a baseline to tell new misses from, and {shown} has no {}: \
         `mutastic baseline` writes one, to commit",
        baseline::FILE
    )))
}

/// With `--fail-on-new`, how the run is judged: on the misses the package's baseline doesn't hold.
/// A share's are judged against it as the whole run's are, and a run made of shares judges the
/// whole.
fn judged_on_new(parsed: &Parsed) -> Option<baseline::FailOnNew> {
    let dir = parsed.positional.first().map_or(".", String::as_str);
    parsed.fail_on_new.then(|| baseline::FailOnNew {
        baseline: baseline::ids(std::path::Path::new(dir)),
    })
}

/// `run --from-shards`: no mutant runs; the shares' results, put together, are the whole run's,
/// with its summary, cards, reports and threshold. Shares that can't be put together are refused
/// before the package is read.
fn from_shards(parsed: &Parsed, shares: &str) -> Result<ExitCode, Failure> {
    let shares =
        crate::shares::Shares::read(std::path::Path::new(shares), env!("CARGO_PKG_VERSION"))?;
    let dir = parsed.positional.first().map_or(".", String::as_str);
    let format = format(parsed);
    let progress = start(parsed, true);
    let project = workspace::read(dir)?;
    progress.package(&project.package);
    let code = crate::shares::share_code(&project)?;
    let report = shares.whole(&project, &code, &options(parsed, Vec::new()), &|step| {
        progress.step(step)
    })?;
    progress.finished();
    finish(parsed, report, format, None)
}

/// A run's end: its report files, a share's events in its folder, then the result, and whether it
/// passed.
fn finish(
    parsed: &Parsed,
    mut report: engine::Report,
    format: Format,
    folder: Option<&std::path::Path>,
) -> Result<ExitCode, Failure> {
    let folder = folder.map_or_else(
        || report.project.mutastic_dir(),
        std::path::Path::to_path_buf,
    );
    // A run with no mutant to run measured nothing: the last run that did stays the one `explain`,
    // `next` and the next run read, unless a report was asked for by name.
    let kept = report.built || report.share.is_some();
    // Only a run of every mutant compares with the last one and is kept for the next: a run of one
    // says nothing about the others, and a change's score says nothing about the whole package's.
    if !partial(parsed) && report.scope.is_none() && kept {
        report.comparison = recorded(&report, &folder)?;
    }
    watched(parsed, &mut report)?;
    // Before the reports, whose `passed` says what the exit code will.
    report.fail_on_new = judged_on_new(parsed);
    // Only a full run writes reports: a run of some mutants would leave out the rest.
    if !partial(parsed) && (kept || parsed.reports.is_some()) {
        let writing = std::time::Instant::now();
        let reports = parsed.reports.clone().unwrap_or_else(|| {
            report::files::DEFAULT
                .iter()
                .map(|(kind, path)| (*kind, path.map(str::to_string)))
                .collect()
        });
        report.written =
            report::files::write(&report, &reports, &folder).map_err(crate::failure::disk)?;
        report.reports = listed(&report);
        // The ids the output shows open the HTML report on their mutant, but not from a share's own
        // folder: its report is only part of the run.
        let html = reports
            .iter()
            .zip(&report.written)
            .filter(|((kind, _), _)| *kind == Kind::Html)
            .map(|(_, file)| file.clone())
            .next_back();
        if let (Some(html), None) = (html, &report.share) {
            report.pages =
                Some(report::files::write_pages(&report, &html).map_err(crate::failure::disk)?);
        }
        // The run isn't over until its reports are written: with thousands of tests that took
        // longer than the mutants did, and the Duration row said only the rest.
        let reporting = writing.elapsed();
        report.reporting = Some(reporting);
        report.took += reporting;
    }
    // A share's events, kept for `--from-shards`: as `--format=jsonl` writes them, whatever the
    // format shown.
    if report.share.is_some() {
        let events = folder.join(EVENTS);
        let given = std::path::Path::new(&report.project.given);
        // Written empty first, so its path is shown as the reports' are.
        crate::shares::keep(&events, "")?;
        report.reports.push(crate::workspace::shown(given, &events));
        report.written.push(report::files::whole(&events));
        // Whole, `run-started` included, even when it went out live.
        // A share is judged with the whole run, not on its own.
        let streamed = std::mem::replace(&mut report.streamed, false);
        let judged = report.fail_on_new.take();
        crate::shares::keep(
            &events,
            &report::render(&report, Format::Jsonl, Style::default()),
        )?;
        report.streamed = streamed;
        report.fail_on_new = judged;
    }
    stdout_text!(
        "{}",
        report::render(
            &report,
            format,
            style(parsed, std::io::stdout().is_terminal())
        )
    );
    to_github(&report)?;
    if format != Format::Jsonl && !partial(parsed) && report.scope.is_none() {
        baseline_note(&report, &parsed.operators_chosen);
    }
    if let Some(choosing) = choosing(parsed).filter(|_| reports_named(parsed)) {
        stderr_line!("{}", reports_left_out_with(choosing));
    }
    // Property tests mutastic couldn't seed: their results can change from run to run.
    if let Some(note) = crate::properties::note(&report.project).filter(|_| report.built) {
        stderr_line!("{note}");
    }
    if !report.failing_alone().is_empty() {
        // A test fails on the code with no mutant in, as one that fails before any mutant does.
        return Ok(ExitCode::from(TESTS_FAIL));
    }
    Ok(if report.passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// On GitHub Actions, an annotation on each of the first misses, on stderr so a format's own output
/// stays as it is, and the job's summary, added to the file other steps write to as well: see
/// [`report::github`].
fn to_github(report: &engine::Report) -> Result<(), Failure> {
    use std::io::Write as _;
    let variable = |name: &str| std::env::var(name).ok();
    if !report::github::on(variable) {
        return Ok(());
    }
    for annotation in report::github::annotations(report) {
        stderr_line!("{annotation}");
    }
    let Some(file) = report::github::summary_file(variable) else {
        return Ok(());
    };
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .and_then(|mut summary| writeln!(summary, "{}", report::github::summary(report)))
        .map_err(|error| {
            Code::Other.with(format!(
                "couldn't write the job summary to {}: {error}",
                file.display()
            ))
        })
}

/// A baseline is pruned only under the operators it was taken under: a mutant only those make
/// would look gone under others, and be dropped.
fn under_the_baselines_operators(here: &std::path::Path, operators: &str) -> Result<(), Failure> {
    match baseline::operators(here) {
        Some(taken) if taken != operators => Err(Code::Config.with(format!(
            "the baseline was taken under operators: {taken}, and this run chooses {operators}, so \
             a mutant only the baseline's operators make would look gone: prune it under {taken}, \
             or take it again"
        ))),
        _ => Ok(()),
    }
}

/// `1 of the baseline's mutants is caught now`, `2 of the baseline's mutants are caught now`.
fn caught_now(count: usize) -> String {
    format!(
        "{count} of the baseline's mutants {} caught now",
        if count == 1 { "is" } else { "are" }
    )
}

/// How many of the mutants the tests catch, as people say it: `catch the only mutant`, `catch all
/// 6 mutants`, `catch 4 of the 6 mutants`.
fn catches(caught: usize, scored: usize) -> String {
    if scored == 1 {
        return if caught == 1 {
            "catch the only mutant".to_string()
        } else {
            "don't catch the only mutant".to_string()
        };
    }
    if caught == scored {
        format!("catch all {scored} mutants")
    } else {
        format!("catch {caught} of the {scored} mutants")
    }
}

/// What a run with `--only` says of the reports `--report` named, as the Java edition says it.
pub const REPORTS_LEFT_OUT: &str = "note: --report is left out with --only: a run of some mutants \
     writes no reports, as it says nothing about the rest; run without --only to write them";

/// What a run of some mutants, chosen by `option` (`--only` or `--at`), says of the reports
/// `--report` named, as the Java edition says it.
pub fn reports_left_out_with(option: &str) -> String {
    REPORTS_LEFT_OUT.replace("--only", option)
}

/// Whether `--report` named a report. With `--only` or `--at` none is written, and a note on
/// stderr says why, so one asked for isn't missed in silence. A note, not a failure: the run's exit
/// code is its own.
fn reports_named(parsed: &Parsed) -> bool {
    parsed
        .reports
        .as_ref()
        .is_some_and(|named| !named.is_empty())
}

/// After a full run, how many of the baseline's mutants it found caught: they can be pruned. Not
/// under other operators than the baseline's, where a mutant only those make would look caught.
fn baseline_note(report: &engine::Report, operators: &str) {
    let here = std::path::Path::new(&report.project.given);
    if baseline::operators(here).is_some_and(|taken| taken != operators) {
        return;
    }
    let older = baseline::ids(here);
    let caught = baseline::caught_now(report, &older).len();
    if !older.is_empty() && caught > 0 {
        stdout_line!(
            "note: {}: `mutastic baseline --prune` drops {}",
            caught_now(caught),
            if caught == 1 { "it" } else { "them" }
        );
    }
}

/// `baseline`: every mutant no test catches, written for the package to commit; or with `--prune`,
/// the baseline's mutants the tests catch now dropped. Never rewritten whole once there, as taking
/// in new misses would hide them.
fn baseline_command(parsed: &Parsed) -> Result<ExitCode, Failure> {
    let dir = parsed.positional.first().map_or(".", String::as_str);
    let here = std::path::Path::new(dir);
    if parsed.prune != baseline::exists(here) {
        if parsed.prune {
            stderr_line!("mutastic: there's no baseline to prune: `mutastic baseline` writes one");
        } else {
            stderr_line!(
                "mutastic: there's a baseline already, in {}. `mutastic baseline --prune` drops its \
                 mutants the tests catch now; it's never rewritten to take in new misses, which \
                 would hide them",
                baseline::FILE
            );
        }
        return Ok(ExitCode::FAILURE);
    }
    if parsed.prune {
        under_the_baselines_operators(here, &parsed.operators_chosen)?;
    }
    let progress = start(parsed, false);
    let project = workspace::read(dir)?;
    progress.package(&project.package);
    let mut options = options(parsed, Vec::new());
    watching(&mut options, &project);
    let report = engine::run(&project, &options, &|step| progress.step(step))?;
    progress.finished();
    if parsed.prune {
        let caught = baseline::caught_now(&report, &baseline::ids(here));
        let dropped = baseline::prune(here, &caught).map_err(crate::failure::disk)?;
        stdout_line!(
            "mutastic: {}, and {} dropped from {}",
            caught_now(dropped),
            if dropped == 1 { "is" } else { "are" },
            baseline::FILE
        );
    } else {
        let entries = baseline::entries(&report);
        baseline::write(here, &parsed.operators_chosen, &entries).map_err(crate::failure::disk)?;
        stdout_line!(
            "mutastic: {} no test catches {} in {}: commit it. A miss that isn't in it is new: the \
             gate holds only new misses, and next offers them first",
            crate::amounts::count(entries.len(), "mutant"),
            if entries.len() == 1 { "is" } else { "are" },
            baseline::FILE
        );
    }
    Ok(ExitCode::SUCCESS)
}

/// `next`: the head of the queue of missed mutants, or with `--top` the first few, as the format
/// has them. While the code under test, the operators, the unsafe code mutated, the scope and
/// mutastic are as they were at the last `next`, it runs only the mutants that one left uncaught,
/// since a test written since can only catch some of them; and none when it left none. Otherwise
/// it runs every mutant, and keeps that run as `run` does, for `explain` and the next run to read.
/// A mutant proposed for ignoring waits on a person, not a test: it's counted, not queued. Exits 1
/// while something is queued, so a loop can run it until it's done.
fn next(parsed: &Parsed) -> Result<ExitCode, Failure> {
    let dir = parsed.positional.first().map_or(".", String::as_str);
    let format = format(parsed);
    let progress = start(parsed, false);
    let project = workspace::read(dir)?;
    progress.package(&project.package);
    let folder = project.mutastic_dir();
    let found = engine::find_with(&project, &parsed.operators, parsed.unsafe_mode);
    let key = crate::nextrun::key(
        &crate::nextrun::code_under_test(&found),
        &parsed.operators,
        parsed.unsafe_mode.name(),
        &format!(
            "{}{}",
            parsed.since.as_deref().unwrap_or("-"),
            crate::build_args::key(&folder)?
        ),
        &crate::record::tool(),
    );
    let last = crate::nextrun::NextRun::read(&folder, &key);
    let report = match &last {
        Some(left) if left.uncaught.is_empty() => None,
        _ => {
            let only = last
                .as_ref()
                .map(|left| left.uncaught.iter().cloned().collect())
                .unwrap_or_default();
            Some(next_pass(parsed, &project, only, &progress)?)
        }
    };
    if let Some(report) = &report {
        let results: Vec<(&str, Status)> = report
            .results
            .iter()
            .map(|r| (report.found.mutants[r.mutant].id.as_str(), r.status))
            .collect();
        let asked = last.map_or_else(
            || results.iter().map(|(id, _)| id.to_string()).collect(),
            |left| left.uncaught,
        );
        let uncaught = crate::nextrun::left_uncaught(&asked, results);
        crate::nextrun::NextRun { key, uncaught }
            .write(&folder)
            .map_err(crate::failure::disk)?;
    }
    let proposed: std::collections::BTreeSet<String> =
        ignores::proposed(std::path::Path::new(dir))?
            .into_iter()
            .map(|entry| entry.id)
            .collect();
    let older = baseline::ids(std::path::Path::new(dir));
    // The package by its directory's name, as the Java edition names a project by its own.
    let named = project
        .workspace
        .join(&project.package_dir)
        .file_name()
        .map_or_else(
            || project.package.clone(),
            |name| name.to_string_lossy().into_owned(),
        );
    let asked = report::next::Asked {
        proposed: &proposed,
        baseline: &older,
        project: &named,
        dir: &project.given,
        format,
        style: style(parsed, std::io::stdout().is_terminal()),
        top: parsed.top,
    };
    let (text, queued) = report::next::next(report.as_ref(), &asked);
    progress.finished();
    stdout_text!("{text}");
    Ok(if queued {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// The run `next` ranks: of `only`, or of every mutant when it's empty. A run of every mutant,
/// with no `--since`, is kept as `run` keeps one: its record, and the report files `explain` reads.
fn next_pass(
    parsed: &Parsed,
    project: &workspace::Project,
    only: Vec<String>,
    progress: &Progress,
) -> Result<engine::Report, Failure> {
    let whole = only.is_empty();
    let mut options = options(parsed, only);
    options.suggestions = !parsed.no_suggestions;
    if let Some(since) = &parsed.since {
        options.since = Some(crate::since::changed_since(
            std::path::Path::new(&project.given),
            since,
        )?);
    }
    // A watched test's catches are confirmed here as in `run`; `next` keeps no watch of its own.
    watching(&mut options, project);
    let mut report = engine::run(project, &options, &|step| progress.step(step))?;
    progress.finished();
    if whole && report.scope.is_none() && report.built {
        let folder = project.mutastic_dir();
        report.comparison = recorded(&report, &folder)?;
        let reports: Vec<(Kind, Option<String>)> = report::files::DEFAULT
            .iter()
            .map(|(kind, path)| (*kind, path.map(str::to_string)))
            .collect();
        report.written =
            report::files::write(&report, &reports, &folder).map_err(crate::failure::disk)?;
        report.reports = listed(&report);
    }
    Ok(report)
}

/// The files `report` wrote, as `run-finished` lists them: from the directory mutastic was pointed
/// at when they're inside it, whole otherwise.
fn listed(report: &engine::Report) -> Vec<String> {
    let given = std::path::Path::new(&report.project.given);
    report
        .written
        .iter()
        .map(|file| crate::workspace::shown(given, file))
        .collect()
}

/// Keeps `report` as the last run in `folder`, with its comparison with the one before: only an
/// earlier run of the same package, in the same place, under the same operators, says anything
/// about it. A share's folder is its own, so a whole run is never compared with part of one.
fn recorded(
    report: &engine::Report,
    folder: &std::path::Path,
) -> Result<Option<crate::record::Comparison>, Failure> {
    let code = crate::record::code_without(
        &crate::shares::code(&report.project)?,
        &engine::LeftOut::names(&report.left_out),
    );
    let code = crate::record::code_built_with(
        &code,
        &crate::build_args::key(&report.project.mutastic_dir())?,
    );
    let record = crate::record::of(report, &code, &crate::record::tool());
    let records = crate::records::RunRecords::new(folder);
    // When the last run used other operators, it's said so instead.
    let comparison = records
        .comparable_to(&record)
        .map(|previous| record.against(&previous))
        .or_else(|| under_other_operators(&records, &record));
    records.keep(&record).map_err(|error| {
        crate::failure::disk(format!(
            "couldn't keep the run's record in {}: {error}",
            folder.display()
        ))
    })?;
    Ok(comparison)
}

/// The last run of this package, when it used other operators than `record`'s: by the set's name,
/// `defaults`, when it was one.
fn under_other_operators(
    records: &crate::records::RunRecords,
    record: &crate::record::RunRecord,
) -> Option<crate::record::Comparison> {
    let last = records.latest()?;
    (record.same_project_as(&last)
        && !last.operators.is_empty()
        && last.operators != record.operators)
        .then(|| crate::record::Comparison::under_other_operators(&set_named(&last.operators)))
}

/// The named set whose operators are `ids`, `defaults`, or how many there are.
fn set_named(ids: &str) -> String {
    for set in ["defaults", "stronger", "extended", "all"] {
        if let Ok(mut operators) = operators(set) {
            operators.sort_unstable();
            if operators.join(",") == ids {
                return set.to_string();
            }
        }
    }
    format!("{} operators", ids.split(',').count())
}

fn verify(parsed: &Parsed) -> Result<ExitCode, Failure> {
    let (only, taken) = verify_ids(&parsed.positional);
    if only.is_empty() {
        return Err(Code::NeedsId.with(id_first("verify", None)));
    }
    let dir = parsed.positional.get(taken).map_or(".", String::as_str);
    let progress = start(parsed, false);
    let project = workspace::read(dir)?;
    progress.package(&project.package);
    let mut options = options(parsed, only.clone());
    // A watched test's catches are confirmed here too, so `verify` never passes on a lucky one.
    watching(&mut options, &project);
    let report = engine::run(&project, &options, &|step| progress.step(step))?;
    progress.finished();
    let mut settled = true;
    let result_of = |id: &str| {
        report
            .results
            .iter()
            .find(|result| report.found.mutants[result.mutant].id == id)
    };
    for result in &report.results {
        let mutant = &report.found.mutants[result.mutant];
        // A twin is said with the result of the mutant it's the same as, which ran for it and
        // decides; that one's own line comes only when it was named too.
        if let Some(runs) = crate::sites::twins::partner_of(mutant).and_then(result_of) {
            settled &= runs.status.settled();
            stdout_line!(
                "mutastic: {} is the same as {}, which {}",
                mutant.id,
                report.found.mutants[runs.mutant].id,
                fate(runs, &report)
            );
        } else if only.contains(&mutant.id) {
            settled &= result.status.settled();
            stdout_line!("mutastic: {} {}", mutant.id, fate(result, &report));
        }
    }
    stdout_text!("{}", report::unmatched_lines(&report));
    if !report.failing_alone().is_empty() {
        stdout_text!("{}", report::failing_alone(&report));
        return Ok(ExitCode::from(TESTS_FAIL));
    }
    Ok(if settled {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// What `verify` says of a mutant after its id: caught, missed, or what became of it instead.
fn fate(result: &engine::MutantResult, report: &engine::Report) -> String {
    let project = &report.project;
    let id = &report.found.mutants[result.mutant].id;
    let likely_flaky = &report.likely_flaky;
    match result.status {
            Status::Killed => caught(result, project),
            Status::Timeout => match crate::limits::at_ceiling(result.reason.as_deref()) {
                Some(note) => format!("is caught by a timeout: its tests {note}"),
                None => "is caught by a timeout".to_string(),
            },
            Status::Survived => "is still missed".to_string(),
            Status::NoCoverage => match report::only_flaky(likely_flaky, id, result) {
                Some(said) => format!("isn't covered: {said}"),
                None => "isn't reached by any test".to_string(),
            },
            Status::Ignored => format!(
                "is skipped ({}), so it isn't run and needs no test",
                untrusted::value(result.reason.as_deref().unwrap_or("skipped"))
            ),
            Status::RuntimeError => format!(
                "crashes, and no test can be said to catch it ({}): it's left out of the score; check the unsafe code it reaches with Miri",
                untrusted::value(result.reason.as_deref().unwrap_or("crash"))
            ),
    }
}

/// "is caught by tests::big_orders", and how, when it wasn't by a test's own check.
/// `explain <id>`: runs the mutant, then shows its card, or what became of it, and what's known of
/// it, with how the last full run went. It explains; it doesn't judge, so it exits 0.
fn explain(parsed: &Parsed) -> Result<ExitCode, Failure> {
    let Some(id) = parsed.positional.first() else {
        return Err(Code::NeedsId.with(id_first("explain", None)));
    };
    if let Some(other) = parsed
        .format
        .filter(|format| !matches!(format, Format::Full | Format::Jsonl))
    {
        return Err(Code::OptionValue.with(format!(
            "explain takes --format=full or --format=jsonl, not --format={}",
            other.name()
        )));
    }
    let dir = parsed.positional.get(1).map_or(".", String::as_str);
    let progress = start(parsed, false);
    let project = workspace::read(dir)?;
    progress.package(&project.package);
    let mut options = options(parsed, vec![id.clone()]);
    options.suggestions = !parsed.no_suggestions;
    watching(&mut options, &project);
    let report = engine::run(&project, &options, &|step| progress.step(step))?;
    progress.finished();
    // A twin didn't run: the mutant it's the same as ran for it, and is what there is to explain.
    let runs = report
        .found
        .mutants
        .iter()
        .find(|mutant| &mutant.id == id)
        .and_then(crate::sites::twins::partner_of)
        .map(str::to_string);
    let explained = runs.as_deref().unwrap_or(id);
    let last = report::last_run(&project.mutastic_dir().join("mutation.json"), explained);
    if let (Some(runs), false) = (&runs, parsed.format == Some(Format::Jsonl)) {
        stdout_line!(
            "mutastic: {id} is the same as {runs}, which runs for it: what follows is that one's"
        );
    }
    let shown = if parsed.format == Some(Format::Jsonl) {
        report::explained_jsonl(&report, last)
    } else {
        report::explained(
            &report,
            last,
            style(parsed, std::io::stdout().is_terminal()),
        )
    };
    stdout_text!("{shown}");
    Ok(ExitCode::SUCCESS)
}

/// The rest of an ignore's command line after `ignore`: its id, its directory as given, a reason to
/// fill in, and its other options, as the Java edition writes it.
fn with_reason(id: &str, parsed: &Parsed) -> String {
    let mut words = vec![shell(id)];
    words.extend(parsed.positional.iter().skip(1).map(|word| shell(word)));
    words.push("--reason=\"<why>\"".to_string());
    if !parsed.kind.is_empty() {
        words.push(format!("--as={}", parsed.kind));
    }
    if parsed.propose {
        words.push("--propose".to_string());
    }
    words.join(" ")
}

/// `ignore`: records a decision not to test a mutant in `.mutastic/ignores.yaml`, with the line it's
/// on and the reason; proposes one, in `.mutastic/proposals.yaml`, for a person to accept; accepts
/// a proposal; removes one; or removes those that no longer hold. Runs no tests, and says how to
/// undo what it did, in the Java edition's words.
fn ignore(parsed: &Parsed) -> Result<ExitCode, Failure> {
    let Some(id) = parsed.positional.first().filter(|_| !parsed.prune).cloned() else {
        if parsed.prune {
            let dir = parsed.positional.first().map_or(".", String::as_str);
            return prune(dir);
        }
        return Err(Code::NeedsId.with(id_first("ignore", None)));
    };
    let dir = parsed
        .positional
        .get(1)
        .map_or(".", String::as_str)
        .to_string();
    let here = std::path::Path::new(&dir);
    // The directory goes after the words only when it isn't where mutastic runs, as in the Java
    // edition.
    let after = if crate::report::files::is_here(here) {
        String::new()
    } else {
        format!(" {}", crate::untrusted::shell_word(&dir))
    };
    let command = |words: &[&str]| {
        let words: Vec<String> = words.iter().map(|word| shell(word)).collect();
        format!(
            "mutastic ignore {}{after}{}",
            words.join(" "),
            crate::build_args::ending()
        )
    };
    if parsed.accept {
        return accept(&id, here, &command);
    }
    if parsed.remove {
        return remove(&id, here, &command);
    }
    let Some(reason) = parsed
        .reason
        .clone()
        .filter(|reason| !reason.trim().is_empty())
    else {
        return Err(Code::NeedsId.with(format!(
            "ignore needs a reason, for whoever reviews it:\n  mutastic ignore {}",
            with_reason(&id, parsed)
        )));
    };
    if parsed.propose {
        argued(&reason, &parsed.kind)?;
    }
    let project = workspace::read(&dir)?;
    let found = engine::find_all(&project);
    let Some(index) = found.mutants.iter().position(|m| m.id == id) else {
        return Err(engine::not_a_mutant(&id, &project.package, &dir, &found));
    };
    // A decision on a twin is the mutant's that runs for it, so it holds where that one runs.
    let (id, index) = match crate::sites::twins::partner_of(&found.mutants[index])
        .and_then(|partner| found.mutants.iter().position(|m| m.id == partner))
    {
        Some(runs) => {
            stdout_line!(
                "mutastic: {id} is the same as {}, which runs for it: the decision is recorded for that one",
                found.mutants[runs].id
            );
            (found.mutants[runs].id.clone(), runs)
        }
        None => (id, index),
    };
    let mutant = &found.mutants[index];
    if let Some(skipped) = &mutant.skipped {
        stdout_line!(
            "mutastic: {id} doesn't run anyway ({}): nothing to ignore",
            untrusted::value(skipped)
        );
        return Ok(ExitCode::SUCCESS);
    }
    let function = &found.functions[mutant.function];
    let entry = ignores::Entry {
        id: id.clone(),
        at: format!("{}, line {}", function.path, mutant.line),
        change: crate::words::advise(mutant, &function.name, &Default::default()).change,
        code: engine::source_line(&found, index).trim().to_string(),
        reason: reason.clone(),
        kind: parsed.kind.clone(),
    };
    let proposals: Vec<ignores::Entry> = ignores::proposed(here)?
        .into_iter()
        .filter(|existing| existing.id != id)
        .collect();
    if parsed.propose {
        let mut proposals = proposals;
        proposals.push(entry);
        ignores::propose(here, &proposals)?;
        stdout_line!(
            "mutastic: {id} is proposed for ignoring: {}\n  recorded in {}: it counts as missed until a person accepts it\n  to accept: {}\n  to withdraw: {}",
            untrusted::value(&reason),
            from_here(here, ignores::PROPOSALS),
            command(&["--accept", &id]),
            command(&["--remove", &id])
        );
        return Ok(ExitCode::SUCCESS);
    }
    let mut entries: Vec<ignores::Entry> = ignores::read(here)?
        .into_iter()
        .filter(|existing| existing.id != id)
        .collect();
    entries.push(entry);
    ignores::write(here, &entries)?;
    // Decided now: a proposal for it is settled.
    ignores::propose(here, &proposals)?;
    stdout_line!(
        "mutastic: {id} is ignored: {}\n  recorded in {}: commit it, as a decision to review\n  to undo: {}",
        untrusted::value(&reason),
        from_here(here, ignores::FILE),
        command(&["--remove", &id])
    );
    Ok(ExitCode::SUCCESS)
}

/// A proposal's reason, which a person checks in place of a test: filled in, and for a mutant said
/// to be equivalent or unreachable, saying why its case can't happen, as the Java edition asks.
fn argued(reason: &str, kind: &str) -> Result<(), Failure> {
    if reason.contains("<case>") || reason.contains("<why>") {
        return Err(Code::OptionValue.with(
            "the reason still has the template's <case> or <why>: fill them in, as \
             --reason=\"Differs only when count is negative. That can't happen because count is a \
             length.\"",
        ));
    }
    let because = reason
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word.eq_ignore_ascii_case("because"));
    if ["equivalent", "unreachable"].contains(&kind) && !because {
        return Err(Code::OptionValue.with(format!(
            "a proposal that a mutant is {kind} needs a reason a person can check: the one case \
             where it differs, and why that can't happen, with a because: --reason=\"{}\"",
            crate::report::REASON
        )));
    }
    Ok(())
}

/// A proposal accepted: it moves into the ignores, with its reason and sort as proposed.
fn accept(
    id: &str,
    here: &std::path::Path,
    command: &dyn Fn(&[&str]) -> String,
) -> Result<ExitCode, Failure> {
    let proposals = ignores::proposed(here)?;
    let Some(proposal) = proposals.iter().find(|entry| entry.id == id).cloned() else {
        stderr_line!(
            "mutastic: {id} isn't proposed in {}: nothing to accept{}",
            from_here(here, ignores::PROPOSALS),
            close_id(id, &proposals, "--accept", command)
        );
        return Ok(ExitCode::FAILURE);
    };
    let mut entries: Vec<ignores::Entry> = ignores::read(here)?
        .into_iter()
        .filter(|existing| existing.id != id)
        .collect();
    entries.push(proposal.clone());
    ignores::write(here, &entries)?;
    let left: Vec<ignores::Entry> = proposals.into_iter().filter(|e| e.id != id).collect();
    ignores::propose(here, &left)?;
    stdout_line!(
        "mutastic: {id} is ignored, as proposed: {}\n  recorded in {}: commit it, as a decision to review\n  to undo: {}",
        untrusted::value(&proposal.reason),
        from_here(here, ignores::FILE),
        command(&["--remove", id])
    );
    Ok(ExitCode::SUCCESS)
}

/// An ignore taken back, or a proposal withdrawn.
fn remove(
    id: &str,
    here: &std::path::Path,
    command: &dyn Fn(&[&str]) -> String,
) -> Result<ExitCode, Failure> {
    let entries = ignores::read(here)?;
    let again = |entry: &ignores::Entry, propose: bool| {
        let reason = format!("--reason={}", entry.reason);
        let kind = format!("--as={}", entry.kind);
        let mut words = vec![entry.id.as_str()];
        if propose {
            words.push("--propose");
        }
        words.push(&reason);
        if !entry.kind.is_empty() {
            words.push(&kind);
        }
        command(&words)
    };
    if let Some(removed) = entries.iter().find(|entry| entry.id == id) {
        let kept: Vec<ignores::Entry> = entries.iter().filter(|e| e.id != id).cloned().collect();
        ignores::write(here, &kept)?;
        stdout_line!(
            "mutastic: {id} is no longer ignored: it runs again\n  removed from {}\n  to undo: {}",
            from_here(here, ignores::FILE),
            again(removed, false)
        );
        return Ok(ExitCode::SUCCESS);
    }
    let proposals = ignores::proposed(here)?;
    if let Some(proposal) = proposals.iter().find(|entry| entry.id == id) {
        let left: Vec<ignores::Entry> = proposals.iter().filter(|e| e.id != id).cloned().collect();
        ignores::propose(here, &left)?;
        stdout_line!(
            "mutastic: {id} is no longer proposed\n  removed from {}\n  to undo: {}",
            from_here(here, ignores::PROPOSALS),
            again(proposal, true)
        );
        return Ok(ExitCode::SUCCESS);
    }
    stderr_line!(
        "mutastic: {id} isn't ignored in {}: nothing to remove{}",
        from_here(here, ignores::FILE),
        close_id(id, &entries, "--remove", command)
    );
    Ok(ExitCode::FAILURE)
}

/// The one id among `entries` a slip away from `id`, said with the command for it, as the Java
/// edition says it: never acted on.
fn close_id(
    id: &str,
    entries: &[ignores::Entry],
    action: &str,
    command: &dyn Fn(&[&str]) -> String,
) -> String {
    crate::similar::closest_within(id, entries.iter().map(|entry| entry.id.as_str()), 1).map_or(
        String::new(),
        |close| {
            format!(
                ". There's one called {close}:\n  {}",
                command(&[action, close])
            )
        },
    )
}

/// A file of `dir`'s, as the person would type it from where mutastic runs: `.mutastic/ignores.yaml`
/// there, `fixtures/pricing/.mutastic/ignores.yaml` from above it.
fn from_here(dir: &std::path::Path, file: &str) -> String {
    crate::report::files::from_here(&dir.join(file))
}

/// `ignore --prune`: takes away every ignore whose line has changed since it was made.
fn prune(dir: &str) -> Result<ExitCode, Failure> {
    let here = std::path::Path::new(dir);
    let project = workspace::read(dir)?;
    let found = engine::find_all(&project);
    let entries = ignores::read(here)?;
    let holds = |entry: &ignores::Entry| {
        found
            .mutants
            .iter()
            .position(|m| m.id == entry.id)
            .is_some_and(|index| entry.holds_for(&engine::source_line(&found, index)))
    };
    let (kept, stale): (Vec<ignores::Entry>, Vec<ignores::Entry>) =
        entries.into_iter().partition(holds);
    if stale.is_empty() {
        stdout_line!("mutastic: every ignore still holds: nothing to remove");
        return Ok(ExitCode::SUCCESS);
    }
    ignores::write(here, &kept)?;
    let mut ids: Vec<String> = stale
        .iter()
        .map(|entry| untrusted::value(&entry.id))
        .collect();
    ids.sort();
    stdout_line!(
        "mutastic: removed {} ignore{} that no longer {}: {}\n  from {}\n  to undo: git checkout -- {}",
        stale.len(),
        if stale.len() == 1 { "" } else { "s" },
        if stale.len() == 1 { "holds" } else { "hold" },
        ids.join(", "),
        from_here(here, ignores::FILE),
        crate::untrusted::shell_word(&from_here(here, ignores::FILE))
    );
    Ok(ExitCode::SUCCESS)
}

/// A word of a command mutastic offers, as a shell takes it: as it is when it's plain, else in
/// single quotes, where nothing in it is run, and with what can't be seen escaped. As the Java
/// edition's `Invocation.quoted`.
fn shell(word: &str) -> String {
    crate::untrusted::shell_word(word)
}

/// `gate open`, `gate check` or `gate close`, as the Java edition's: see [`gate`].
fn gate_command(parsed: &Parsed) -> Result<ExitCode, Failure> {
    // A hooked check's budget counts from here.
    let started = std::time::Instant::now();
    let action = gate_action(parsed)?.to_string();
    if parsed.help {
        stdout_line!("{}", gate_help(&action)?);
        return Ok(ExitCode::SUCCESS);
    }
    let dir = parsed.positional.get(1).map_or(".", String::as_str);
    let hooked = !parsed.hook.is_empty();
    // A hook runs in whatever project the agent is in: where mutastic reads no Cargo package,
    // there's nothing to gate, and it says nothing.
    let project = match workspace::read(dir) {
        Ok(project) => project,
        Err(_) if hooked => return Ok(ExitCode::SUCCESS),
        Err(error) => return Err(error),
    };
    let session = if hooked {
        let mut input = String::new();
        let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut input);
        gate::session(&input)
    } else {
        String::new()
    };
    let work = project.mutastic_dir();
    let here = std::path::Path::new(dir);
    match action.as_str() {
        "open" => gate_open(parsed, &project, here, &session),
        "close" => {
            gate_count::stop(&work);
            let closed = gate::State::close(&work);
            if !hooked {
                stdout_line!(
                    "{}",
                    if closed {
                        "mutastic gate: closed"
                    } else {
                        "mutastic gate: none open here"
                    }
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        // `check`, the one action left: `gate_action` takes no other.
        _ => gate_check(parsed, &project, here, &work, started),
    }
}

fn gate_help(action: &str) -> Result<String, Failure> {
    commands::command(&format!("gate {action}"))
        .map(crate::help::of)
        .ok_or_else(|| no_gate_action(action))
}

/// `gate`'s action: open, check or close. Checked before the project is looked for, so a mistyped
/// one is said as that wherever it's run.
fn gate_action(parsed: &Parsed) -> Result<&str, Failure> {
    match parsed.positional.first().map(String::as_str) {
        None => Err(Code::UnknownCommand.with(
            "gate needs what to do: open, check or close. To see what each does:\n  mutastic --help",
        )),
        Some(action @ ("open" | "check" | "close")) => Ok(action),
        Some(other) => Err(no_gate_action(other)),
    }
}

/// `gate`'s action isn't one it has: the close one, in the line, or where the commands are listed.
fn no_gate_action(action: &str) -> Failure {
    match crate::similar::closest(action, ["open", "check", "close"]) {
        Some(close) => Code::UnknownCommand
            .with(format!(
                "gate has no command {action}. There's one called {close}."
            ))
            .correcting(action, close),
        None => Code::UnknownCommand.with(format!(
            "gate has no command {action}. {}",
            crate::mistyped::commands_usage()
        )),
    }
}

/// The operators a gated task runs, chosen when it opens, as chosen and resolved: `--operators`,
/// else `mutastic.yaml`'s `gate:` `operators:`, else its `operators:`, else the defaults. Checked
/// here, so a set the project doesn't have is said at the open, where it was chosen.
fn gate_operators(parsed: &Parsed) -> Result<(String, Vec<&'static str>), Failure> {
    if parsed.operators_given.is_none() && !parsed.gate_configured.is_empty() {
        let resolved =
            crate::config::resolve_from(&parsed.gate_configured, &parsed.gate_configured_from)?;
        return Ok((
            chosen(parsed.gate_configured.iter().map(String::as_str)),
            resolved,
        ));
    }
    Ok((parsed.operators_chosen.clone(), parsed.operators.clone()))
}

/// Whether two sets of operators are the same, whatever their order.
fn same_set(first: &[&str], second: &[&str]) -> bool {
    let mut first = first.to_vec();
    let mut second = second.to_vec();
    first.sort_unstable();
    first.dedup();
    second.sort_unstable();
    second.dedup();
    first == second
}

/// The check, running the operators its open chose, so a check can't run fewer than the task
/// opened with. Given `--operators` that come to another set, it's refused rather than run. A task
/// an older gate opened kept none: its checks choose as an open would.
fn under_the_opens_operators(parsed: &Parsed, open: &gate::State) -> Result<Parsed, Failure> {
    let (chosen_at_open, resolved) = if open.operators.is_empty() {
        gate_operators(parsed)?
    } else {
        let entries: Vec<String> = open.operators.split(',').map(str::to_string).collect();
        let resolved = crate::config::resolve(&entries, "the gate's operators")?;
        (open.operators.clone(), resolved)
    };
    if let Some(given) = &parsed.operators_given {
        if !same_set(&parsed.operators, &resolved) {
            return Err(Code::OptionValue.with(format!(
                "the gate opened with operators: {chosen_at_open}, and a check runs what the open \
                 chose: `mutastic gate close`, then `mutastic gate open --operators={}`, to choose \
                 again",
                chosen(given.split(','))
            )));
        }
    }
    let mut under = parsed.clone();
    under.operators = resolved;
    under.operators_chosen = chosen_at_open;
    Ok(under)
}

/// Keeps where the task starts; with a hook, once for each session, and at each later prompt takes
/// in the ignores the person made between turns. A hook's output is the agent's to read: it's told
/// the task is gated, and how to check its work as it goes.
fn gate_open(
    parsed: &Parsed,
    project: &workspace::Project,
    here: &std::path::Path,
    session: &str,
) -> Result<ExitCode, Failure> {
    let hooked = !parsed.hook.is_empty();
    let work = &project.mutastic_dir();
    let open =
        gate::State::read(work).filter(|state| !session.is_empty() && state.session == session);
    if let Some(open) = open.filter(|_| hooked) {
        // A later prompt: what the person ignored between turns is the task's to start from.
        if open.stopped.is_some() {
            open.prompted(&ignored_ids(here)?)
                .write(work)
                .map_err(crate::failure::disk)?;
        }
        return Ok(ExitCode::SUCCESS);
    }
    let Some(base) = crate::since::git(here, &["rev-parse", "HEAD"]) else {
        return Err(Code::Git.with("the gate needs a git repository with a commit"));
    };
    let base = base.trim().to_string();
    let (operators, resolved) = gate_operators(parsed)?;
    // The tests as libtest lists them, each case its own, are counted in the background, so the
    // agent needn't wait for a build: a check reads the count, and until then the source's test
    // attributes stand in.
    let counting = gate_count::start(here, work, &gate::tree(here));
    let state = gate::State {
        session: session.to_string(),
        base: base.clone(),
        ignored: ignored_ids(here)?,
        baselined: Some(baseline::ids(here)),
        excluded: Some(configured_names(parsed)),
        shape: gate::Shape::of(here),
        uncounted: if counting.is_empty() {
            "the count couldn't start".into()
        } else {
            String::new()
        },
        counting,
        operators: operators.clone(),
        ..gate::State::default()
    };
    state.write(work).map_err(crate::failure::disk)?;
    let run_flag = if same_set(&resolved, &parsed.run_operators) {
        String::new()
    } else {
        format!(" --operators={operators}")
    };
    stdout_line!(
        "mutastic gate: this task is gated from {} (operators: {operators}): before you finish, check the lines it changes with `mutastic run --since={base}{run_flag} --format=agent{}`; {}. {}",
        &base[..base.len().min(7)],
        crate::build_args::ending(),
        if hooked {
            "the gate checks them when you finish, so you needn't run `mutastic gate check` yourself"
        } else {
            "`mutastic gate check` says when it's done"
        },
        crate::report::NO_REWRITE
    );
    Ok(ExitCode::SUCCESS)
}

fn ignored_ids(here: &std::path::Path) -> Result<std::collections::BTreeSet<String>, Failure> {
    Ok(ignores::read(here)?
        .into_iter()
        .map(|entry| entry.id)
        .collect())
}

/// Whether the task is done: as a hook's answer, or as the exit status, with what's left.
fn gate_check(
    parsed: &Parsed,
    project: &workspace::Project,
    here: &std::path::Path,
    work: &std::path::Path,
    started: std::time::Instant,
) -> Result<ExitCode, Failure> {
    let hooked = !parsed.hook.is_empty();
    let Some(open) = gate::State::read(work) else {
        if !hooked {
            stderr_line!("mutastic gate: none open here: `mutastic gate open` starts one");
        }
        // Nothing gated: a hooked agent may stop.
        return Ok(ExitCode::from(if hooked { 0 } else { 2 }));
    };
    let parsed = &under_the_opens_operators(parsed, &open)?;
    let time = CheckTime {
        hooked,
        budget: gate_budget::Budget::of(|name| std::env::var(name).ok(), work)?,
        own_since: started,
    };
    let (open, waited) = counted(open, here, work, &time)?;
    // The check's own time, from here on: its wait for the count left out.
    let time = CheckTime {
        own_since: started + waited,
        ..time
    };
    let verdict = uncounted_note(&open, gate_verdict(parsed, project, here, &open, &time)?);
    let mut state = open.clone();
    state.red.extend(verdict.red.clone());
    // As a hook, a check that lets the agent stop notes what's ignored: what's ignored after it, and
    // before the next prompt, is the person's.
    let stopped = if hooked {
        Some(ignored_ids(here)?)
    } else {
        open.stopped.clone()
    };
    if verdict.passes {
        state.blocks = 0;
        state.tree.clear();
        state.stopped = stopped;
        state.write(work).map_err(crate::failure::disk)?;
        return Ok(gate_answer(hooked, &for_the_person(here, &state, verdict)));
    }
    // Sent back, unless, as a hook, it changed nothing since it last was, or it has had its tries:
    // then it's let stop, and the person told what's left.
    let tree = gate::tree(here);
    let unchanged = open.unchanged_since_blocked(&tree);
    state.blocks += 1;
    state.tree = tree;
    let handed_over = if hooked && unchanged {
        Some(format!(
            "mutastic gate: nothing changed since the agent was sent back, so it's over to you. {}",
            verdict.text
        ))
    } else if hooked && state.blocks > parsed.tries {
        Some(format!(
            "mutastic gate: the agent was sent back {} times, so it's over to you. {}",
            parsed.tries, verdict.text
        ))
    } else {
        None
    };
    let Some(text) = handed_over else {
        // Sent back, the agent has the turn: what it ignores now is its own.
        state.stopped = None;
        state.write(work).map_err(crate::failure::disk)?;
        return Ok(gate_answer(hooked, &verdict));
    };
    state.blocks = 0;
    state.tree.clear();
    state.stopped = stopped;
    state.write(work).map_err(crate::failure::disk)?;
    let answer = for_the_person(here, &open, gate::Verdict::pass(text));
    Ok(gate_answer(hooked, &answer))
}

/// What a check may spend, and since when its own time counts: see [`gate_budget`].
struct CheckTime {
    hooked: bool,
    budget: gate_budget::Budget,
    own_since: std::time::Instant,
}

impl CheckTime {
    /// As a hook, the hand-over of a check whose last run took longer than the hook allows: stopped
    /// by the hook, it would say nothing, and gate nothing.
    fn overrun(&self) -> Option<gate::Verdict> {
        (self.hooked && self.budget.overruns())
            .then(|| gate::Verdict::hand_over(self.budget.overrun()))
    }
}

/// The gate with the count of the tests the task started with read, once a check needs it, and
/// how long it waited: a task that changed nothing needs no count. A count that failed, or isn't
/// done within the wait, leaves the source's test attributes standing in, and says why. As a hook,
/// the wait is what the budget leaves for it, with a run as long as the last still to come.
fn counted(
    mut open: gate::State,
    here: &std::path::Path,
    work: &std::path::Path,
    time: &CheckTime,
) -> Result<(gate::State, std::time::Duration), Failure> {
    let no_wait = std::time::Duration::ZERO;
    if open.counting.is_empty() || open.shape.listed || gate::untouched(here, &open.base) {
        return Ok((open, no_wait));
    }
    if time.overrun().is_some() {
        // It hands over before it runs: the count is kept for the person's check.
        return Ok((open, no_wait));
    }
    let wait = if time.hooked {
        time.budget.count_wait_before_run(time.own_since.elapsed())
    } else {
        time.budget.count_wait
    };
    let waiting = std::time::Instant::now();
    match gate_count::wait(work, &open.counting, wait) {
        Ok(names) => open.shape = gate::Shape::listed(&names),
        Err(why) => {
            gate_count::stop(work);
            open.uncounted = why;
        }
    }
    open.counting.clear();
    open.write(work).map_err(crate::failure::disk)?;
    Ok((open, waiting.elapsed()))
}

/// A pass, saying so when the tests the task started with couldn't be counted as libtest lists
/// them, as the Java edition says it of its count.
fn uncounted_note(state: &gate::State, verdict: gate::Verdict) -> gate::Verdict {
    if !verdict.passes || verdict.handed_over || state.uncounted.is_empty() {
        return verdict;
    }
    gate::Verdict {
        text: format!(
            "{}. The tests the task started with couldn't be counted ({}), so the gate compared the test attributes in the source only",
            verdict.text,
            untrusted::value(&state.uncounted)
        ),
        ..verdict
    }
}

/// A pass or a hand-over, with the tests there when the task started that it changed what they
/// expect: the person's to check, since the score can't.
fn for_the_person(
    here: &std::path::Path,
    state: &gate::State,
    verdict: gate::Verdict,
) -> gate::Verdict {
    let changed = gate::changed_expectations(here, &state.base);
    if changed.is_empty() {
        return verdict;
    }
    let files: Vec<String> = changed.iter().map(|file| untrusted::value(file)).collect();
    gate::Verdict {
        text: format!(
            "{}\n\nThe task changed what tests that were there when it started expect, in the files below: check that's what you asked for\n\n{}",
            verdict.text,
            untrusted::fence(&files.join("\n"))
        ),
        ..verdict
    }
}

/// The verdict, as the hook's answer, or printed, with the exit status.
fn gate_answer(hooked: bool, verdict: &gate::Verdict) -> ExitCode {
    if hooked {
        stdout_line!("{}", gate::claude_hook(verdict));
        return ExitCode::SUCCESS;
    }
    stdout_line!("{}", verdict.text);
    if verdict.passes {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// What the gate says of the task: done, or what's left.
fn gate_verdict(
    parsed: &Parsed,
    project: &workspace::Project,
    here: &std::path::Path,
    state: &gate::State,
    time: &CheckTime,
) -> Result<gate::Verdict, Failure> {
    let untouched = gate::untouched(here, &state.base);
    // The tests there now, by the names a run gives them, where a check below needs them.
    let names = if !state.red.is_empty() || (state.shape.listed && !untouched) {
        Some(gate::listed(project)?)
    } else {
        None
    };
    // First: taking the test away may leave no line changed.
    let gone = names
        .as_ref()
        .map(|names| gate::gone(&state.red, names, here))
        .unwrap_or_default();
    if !gone.is_empty() {
        let one = gone.len() == 1;
        let names: Vec<String> = gone.iter().map(|test| untrusted::value(test)).collect();
        return Ok(gate::Verdict::block(format!(
            "mutastic gate: {} failed at an earlier check, and {} gone now. Put {} back: the gate doesn't pass a task by taking a failing test away. Leave a failing test in place and make the code pass it, unless the spec or plan is what's wrong, in which case say so and stop\n\n{}",
            if one {
                "the test named below".to_string()
            } else {
                format!("the {} tests named below", gone.len())
            },
            if one { "it's" } else { "they're" },
            if one { "it" } else { "them" },
            untrusted::fence(&names.join("\n"))
        )));
    }
    // Whatever lines it changed: a task that only ignores a mutant itself changes none mutastic
    // mutates, and mustn't pass for it. Checked even on a tree git sees as untouched, which it is
    // when the project's git ignores `.mutastic/`.
    let added: Vec<String> = ignored_ids(here)?
        .difference(&state.ignored)
        .cloned()
        .collect();
    if !added.is_empty() {
        return Ok(gate::Verdict::block(format!(
            "mutastic gate: the task ignored {} itself, and whether a mutant is worth a test is a person's call. Take it back with `mutastic ignore --remove ID`, and propose it instead: `mutastic ignore ID --propose --as=KIND --reason=\"…\"`",
            ids_shown(&added)
        )));
    }
    // A test the task left out under `exclude-tests:` itself is a test taken away: leaving one out
    // is a person's call. A file an older gate wrote kept no list, and nothing is compared.
    if let Some(opened) = &state.excluded {
        let added: Vec<String> = configured_names(parsed)
            .difference(opened)
            .cloned()
            .collect();
        if !added.is_empty() {
            return Ok(gate::Verdict::block(self_excluded(&added)));
        }
    }
    // Compared with the baseline when the gate opened, not the start commit's: where git ignores
    // `.mutastic/`, the commit has none, and every older entry would look like the task's.
    let opened = state
        .baselined
        .clone()
        .unwrap_or_else(|| baseline::ids_at(here, &state.base));
    let older = baseline::ids(here);
    let baselined: Vec<String> = older.difference(&opened).cloned().collect();
    if !baselined.is_empty() {
        let one = baselined.len() == 1;
        return Ok(gate::Verdict::block(format!(
            "mutastic gate: the task added {} to {} itself, and whether a miss is the project's backlog is a person's call. Take {} out, and write the test, or propose ignoring it: `mutastic ignore ID --propose --as=KIND --reason=\"…\"`",
            ids_shown(&baselined),
            baseline::FILE,
            if one { "it" } else { "them" }
        )));
    }
    // A task that changed nothing else can't have taken a test away or broken one.
    if untouched {
        return Ok(gate::Verdict::pass(
            "mutastic gate: passed: the task changed no lines mutastic mutates".into(),
        ));
    }
    let now = match &names {
        Some(names) if state.shape.listed => gate::Shape::listed(names),
        _ => gate::Shape::of(here),
    };
    let before = state.shape;
    if now.fewer_than(&before) {
        return Ok(gate::Verdict::block(format!(
            "mutastic gate: fewer tests run than when the task started ({}, {} ignored; now {}, {} ignored). Put them back: the gate doesn't pass a change by taking tests away",
            before.tests, before.disabled, now.tests, now.disabled
        )));
    }
    let changed = crate::since::changed_since(here, &state.base)?;
    let found = engine::find_all(project);
    let ours = found.files.iter().any(|file| {
        std::fs::canonicalize(project.workspace.join(&file.path))
            .is_ok_and(|path| changed.lines.contains_key(&path))
    });
    if let Some(overrun) = time.overrun() {
        return Ok(overrun);
    }
    if !ours {
        // A change to a test, a build file or a test's data may still break a test, so they run.
        return match gate_run(parsed, project, changed, time) {
            Ok(_) => Ok(gate::Verdict::pass(
                "mutastic gate: passed: the tests pass, and the task changed no lines mutastic mutates"
                    .into(),
            )),
            Err(error) => red_verdict(here, state, &error),
        };
    }
    let mut report = match gate_run(parsed, project, changed, time) {
        Ok(report) => report,
        Err(error) => return red_verdict(here, state, &error),
    };
    if !report.failing_alone().is_empty() {
        return Ok(gate::Verdict::block(gate::false_caught(&report)));
    }
    let proposed: std::collections::BTreeSet<String> = ignores::proposed(here)?
        .into_iter()
        .map(|entry| entry.id)
        .collect();
    // A miss the baseline had before the task is the project's backlog, not the task's.
    let ids_with = |status: Status| -> Vec<String> {
        report
            .results
            .iter()
            .filter(|r| r.status == status)
            .map(|r| report.found.mutants[r.mutant].id.clone())
            .filter(|id| !older.contains(id))
            .collect()
    };
    let backlog = report
        .results
        .iter()
        .filter(|r| baseline::uncaught(r))
        .filter(|r| older.contains(&report.found.mutants[r.mutant].id))
        .count();
    let (survived, uncovered) = (ids_with(Status::Survived), ids_with(Status::NoCoverage));
    let missed: Vec<String> = survived.iter().chain(&uncovered).cloned().collect();
    let open = |ids: &[String]| -> Vec<String> {
        ids.iter()
            .filter(|id| !proposed.contains(*id))
            .cloned()
            .collect()
    };
    let (open_missed, open_uncovered) = (open(&survived), open(&uncovered));
    if open_missed.is_empty() && open_uncovered.is_empty() {
        let caught = report.count(Status::Killed) + report.count(Status::Timeout);
        let scored = report.results.len() - report.count(Status::Ignored);
        let waiting = if missed.is_empty() {
            String::new()
        } else {
            format!(
                "; the rest wait for a person, proposed in .mutastic/proposals.yaml: {}",
                missed.join(", ")
            )
        };
        let backlog = if backlog == 0 {
            String::new()
        } else {
            format!(
                "; {backlog}, missed before the task and in {}, {} the project's backlog, not the task's",
                baseline::FILE,
                if backlog == 1 { "is" } else { "are" }
            )
        };
        if scored == 0 {
            return Ok(gate::Verdict::pass(
                format!(
                    "mutastic gate: passed: the tests pass, and the lines the task changed hold no mutant to run (operators: {})",
                    parsed.operators_chosen
                ),
            ));
        }
        return Ok(gate::Verdict::pass(format!(
            "mutastic gate: passed: the tests {} on the lines the task changed (operators: {}){waiting}{backlog}",
            catches(caught, scored),
            parsed.operators_chosen
        )));
    }
    let waiting: Vec<String> = missed
        .iter()
        .filter(|id| proposed.contains(*id))
        .cloned()
        .collect();
    // The cards are for what holds the gate: a proposed mutant waits on a person, not the agent.
    report.results.retain(|r| {
        let id = &report.found.mutants[r.mutant].id;
        !older.contains(id)
            && (!proposed.contains(id)
                || !matches!(r.status, Status::Survived | Status::NoCoverage))
    });
    let cards: String = report::render(&report, Format::Agent, Style::default())
        .lines()
        .skip_while(|line| line.starts_with("mutastic: "))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(gate::Verdict::block(format!(
        "{}\n\n{cards}",
        gate::held_by(
            &open_missed,
            &open_uncovered,
            &waiting,
            &parsed.operators_chosen
        )
    )))
}

/// The gate's run of the lines the task changed. It always builds and runs the tests, as
/// `--force-coverage` does: a change with no mutant on its lines (a constant, an import, a log line)
/// can still break a test, and the gate mustn't pass it.
fn gate_run(
    parsed: &Parsed,
    project: &workspace::Project,
    changed: crate::since::ChangedLines,
    time: &CheckTime,
) -> Result<engine::Report, Failure> {
    let mut options = options(parsed, Vec::new());
    options.since = Some(changed);
    options.force_coverage = true;
    // A watched test's catch counts only if it repeats, so the gate never passes on a lucky one.
    watching(&mut options, project);
    let progress = Progress::start(Shown::None, Style::default(), (false, parsed.workers), None);
    let report = engine::run(project, &options, &|step| progress.step(step));
    // Red or not, it ran: how long it took is the next check's estimate of its own.
    gate_budget::ran(&project.mutastic_dir(), time.own_since.elapsed());
    report
}

/// Tests that fail before any mutant, each time they run. One that was there when the task
/// started is most likely broken by the change, and rewriting what it expects would pass a wrong
/// change: the agent is told to find out which is wrong, and to ask. The gate remembers them, so
/// taking one away later holds the gate. The words are the Java edition's.
fn red_verdict(
    here: &std::path::Path,
    state: &gate::State,
    failure: &Failure,
) -> Result<gate::Verdict, Failure> {
    let Some((failing, error)) = crate::flaky::failing_tests(&failure.message) else {
        return Err(failure.clone());
    };
    // `the tests fail without any mutant: a (it timed out), b; their output is in <log>`: the
    // tests' names are the project's, so they're named in a fence, after what to do.
    let (listed, log) = error
        .strip_prefix("the tests fail without any mutant: ")
        .and_then(|rest| rest.rsplit_once("; their output is in "))
        .unwrap_or_default();
    let before: Vec<&String> = failing
        .iter()
        .filter(|test| {
            let name = gate::function_of(test, here);
            crate::since::git(here, &["grep", "-q", "-w", &name, &state.base, "--", "."]).is_some()
        })
        .collect();
    let named: Vec<String> = crate::flaky::listed(listed)
        .into_iter()
        .map(|test| {
            let name = test.split(' ').next().unwrap_or(test);
            let there = if before.iter().any(|was| was.as_str() == name) {
                " (there when the task started)"
            } else {
                ""
            };
            format!("{}{there}", untrusted::value(test))
        })
        .collect();
    let fenced = untrusted::fence(&named.join("\n"));
    let log = untrusted::value(log);
    let text = if before.is_empty() {
        format!(
            "mutastic gate: the tests named below fail without any mutant; their output is in {log}. Make them pass first, unless the spec or plan is what's wrong, in which case say so and stop. Either way, leave a failing test in place: the gate doesn't pass a task by taking it away\n\n{fenced}"
        )
    } else {
        let one = before.len() == 1;
        format!(
            "mutastic gate: the tests named below fail without any mutant; their output is in {log}.\n\n{} there when the task started, marked so below: if {} passed then, the task's change broke {}. Find out whether the change or the test is wrong. Don't change what a test expects, or take it away, to make it pass unless the task asked for that behaviour, and if you're not sure, ask\n\n{fenced}",
            if one {
                "One was".to_string()
            } else {
                format!("{} were", before.len())
            },
            if one { "it" } else { "they" },
            if one { "it" } else { "them" }
        )
    };
    Ok(gate::Verdict {
        red: gate::remember(&failing, here),
        ..gate::Verdict::block(text)
    })
}

/// Ids from a file the project keeps (ignores, a baseline), as a message names them: each
/// [`untrusted::value`]d, since nothing but mutastic checks what's in them.
/// The tests mutastic.yaml's `exclude-tests:` leaves out, by name.
fn configured_names(parsed: &Parsed) -> std::collections::BTreeSet<String> {
    parsed
        .configured_tests
        .iter()
        .map(|test| test.test.clone())
        .collect()
}

/// What the gate says of tests the task left out under `exclude-tests:` itself.
fn self_excluded(added: &[String]) -> String {
    let one = added.len() == 1;
    format!(
        "mutastic gate: the task left out {} under exclude-tests: in mutastic.yaml itself, which takes {} away, and leaving a test out is a person's call. Take {} back out of the list and leave the test in place: find out whether the change or the test is wrong, then tell the person, or propose leaving it out, for them to decide",
        ids_shown(added),
        if one { "a test" } else { "tests" },
        if one { "it" } else { "them" }
    )
}

fn ids_shown(ids: &[String]) -> String {
    ids.iter()
        .map(|id| untrusted::value(id))
        .collect::<Vec<_>>()
        .join(", ")
}

fn caught(result: &engine::MutantResult, project: &workspace::Project) -> String {
    let how = result.killed_with.as_deref().unwrap_or("panic");
    if let Some(crash) = how.strip_prefix("crash: ") {
        return format!(
            "is caught only because the test process crashed ({}): no test's check caught it",
            untrusted::value(crash)
        );
    }
    let test = format!(
        "{}{}",
        untrusted::value(result.killed_by.as_deref().unwrap_or("a test")),
        crate::cases::caught(project, result)
            .map(|case| case.said())
            .unwrap_or_default()
            + &crate::properties::said(result.killed_input.as_deref())
    );
    match how.strip_prefix("panic: ") {
        Some(message) => format!(
            "is caught by {test}, through a panic: {}",
            untrusted::value(message)
        ),
        None => format!("is caught by {test}"),
    }
}

fn mutants(parsed: &Parsed) -> Result<ExitCode, Failure> {
    let dir = parsed.positional.first().map_or(".", String::as_str);
    let project = workspace::read(dir)?;
    let estimates = if parsed.estimate {
        measured_estimates(parsed, &project)?
    } else if parsed.operators_given.is_some() {
        vec![recorded_estimate(parsed, &project)?]
    } else {
        Vec::new()
    };
    let found = engine::find_with(&project, &parsed.operators, parsed.unsafe_mode);
    let listed: Vec<&crate::sites::Mutant> = if parsed.at.is_empty() {
        found.mutants.iter().collect()
    } else {
        let lines = crate::at::files(&parsed.at, &project, &found)?;
        let on = crate::at::on(&lines, &found, |_| true, &project.given)?;
        found
            .mutants
            .iter()
            .filter(|mutant| on.contains(&mutant.id))
            .collect()
    };
    for mutant in &listed {
        let function = &found.functions[mutant.function];
        stdout_line!(
            "{} {}:{}:{} {} {} in {}{}",
            mutant.id,
            untrusted::value(&mutant.file.display().to_string()),
            mutant.line,
            mutant.column,
            mutant.operator,
            untrusted::escape(&crate::words::plain(
                &crate::words::advise(mutant, &function.name, &Default::default()).change
            )),
            untrusted::value(&function.path),
            mutant
                .near_unsafe
                .map(|near| format!(" (near unsafe: {near})"))
                .unwrap_or_default()
        );
    }
    for estimate in &estimates {
        stdout_line!("{estimate}");
    }
    stdout_line!("{}", crate::amounts::count(listed.len(), "mutant"));
    // What wasn't mutated is counted over the package, not the lines `--at` asked for.
    if !parsed.at.is_empty() {
        return Ok(ExitCode::SUCCESS);
    }
    if found.not_mutated_unsafe > 0 {
        stdout_line!(
            "not mutated: unsafe ({}): --unsafe=enclosing or --unsafe=all includes them",
            found.not_mutated_unsafe
        );
    }
    for (cfg, count) in &found.not_built {
        stdout_line!("not built here: `{}` ({count})", untrusted::value(cfg));
    }
    Ok(ExitCode::SUCCESS)
}

/// Where a package's tests' times are kept: beside its last run's record.
fn test_times_file(project: &workspace::Project) -> std::path::PathBuf {
    project.mutastic_dir().join(test_times::FILE)
}

/// Keeps what a whole run's coverage pass measured, for `mutants --operators` to estimate from. An
/// estimate is a convenience: a record that can't be written leaves the next one saying there's
/// none, and the run goes on.
fn keep_test_times(report: &engine::Report) {
    let record = TestTimes {
        kind: test_times::Kind::Run,
        at: report::files::utc(report.started),
        operators: test_times::joined(&report.operators),
        code: test_times::code(&report.found),
        base: report.found.mutants.iter().map(|m| m.id.clone()).collect(),
        sites: report.measured.clone(),
    };
    let _ = record.write(&test_times_file(&report.project));
}

/// `mutants --operators=…`: what the set listed adds to the tests' time over the last run's, from
/// its record, or why there's no estimate. Nothing is built or run.
fn recorded_estimate(parsed: &Parsed, project: &workspace::Project) -> Result<String, Failure> {
    let Some(record) = TestTimes::read(&test_times_file(project)) else {
        return Ok(test_times::NO_RECORD.to_string());
    };
    // As a run would find them, with the project's ignores and rules: a skipped mutant costs
    // nothing.
    let (found, _) = engine::find_for_run(project, &options(parsed, Vec::new()))?;
    let now = test_times::Now {
        code: &test_times::code(&found),
        configured: &test_times::joined(&parsed.run_operators),
        mutants: &found.mutants,
    };
    Ok(record.against(&now, set_named))
}

/// `mutants --estimate`: one coverage pass with every mutant's site measured and no mutant run,
/// kept as the record the next `mutants --operators` estimates from, then what each of the
/// defaults, stronger and all would take.
fn measured_estimates(
    parsed: &Parsed,
    project: &workspace::Project,
) -> Result<Vec<String>, Failure> {
    let started = std::time::SystemTime::now();
    let progress = start(parsed, false);
    progress.package(&project.package);
    let under = |set: &str| -> Result<Options, Failure> {
        let mut options = options(parsed, Vec::new());
        options.operators = operators(set)?;
        Ok(options)
    };
    let sites = engine::estimate(project, &under("all")?, &|step| progress.step(step))?;
    progress.finished();
    let mut configured = options(parsed, Vec::new());
    configured.operators = parsed.run_operators.clone();
    let (base, _) = engine::find_for_run(project, &configured)?;
    let record = TestTimes {
        kind: test_times::Kind::CoveragePass,
        at: report::files::utc(started),
        operators: test_times::joined(&parsed.run_operators),
        code: test_times::code(&base),
        base: base.mutants.iter().map(|m| m.id.clone()).collect(),
        sites,
    };
    let _ = record.write(&test_times_file(project));
    ["defaults", "stronger", "all"]
        .into_iter()
        .map(|set| {
            let (found, _) = engine::find_for_run(project, &under(set)?)?;
            Ok(record.whole_set(set, &found.mutants))
        })
        .collect()
}

/// `mutastic build [dir] --fraction=0.25`: writes the schema with that share of the mutants (every
/// site's mutants together, spread evenly), builds it with rollback, and prints a JSON summary.
fn build(parsed: &Parsed) -> Result<ExitCode, Failure> {
    let dir = parsed.positional.first().map_or(".", String::as_str);
    let project = workspace::read(dir)?;
    let found = engine::find_with(&project, &parsed.operators, parsed.unsafe_mode);
    let mut sites: Vec<(std::path::PathBuf, usize, usize)> = found
        .mutants
        .iter()
        .map(|m| (m.file.clone(), m.site.start, m.site.end))
        .collect();
    sites.sort();
    sites.dedup();
    let keep = (sites.len() as f64 * parsed.fraction).round() as usize;
    let chosen: std::collections::BTreeSet<_> = (0..keep)
        .map(|i| sites[i * sites.len() / keep.max(1)].clone())
        .collect();
    let live: std::collections::BTreeSet<usize> = found
        .mutants
        .iter()
        .enumerate()
        .filter(|(_, m)| chosen.contains(&(m.file.clone(), m.site.start, m.site.end)))
        .map(|(index, _)| index)
        .collect();
    let started = std::time::Instant::now();
    let built = crate::compile::build(&project, &found, &live, &Default::default(), &|_| {})?;
    let rounds: Vec<serde_json::Value> = built
        .rounds
        .iter()
        .map(|round| {
            serde_json::json!({
                "phase": round.phase,
                "ms": round.took.as_millis() as u64,
                "withdrawn": round.withdrawn,
                "byFunction": round.by_function,
            })
        })
        .collect();
    let binaries: Vec<serde_json::Value> = built
        .binaries
        .iter()
        .map(|binary| {
            serde_json::json!({
                "target": binary.target,
                "path": binary.path,
                "bytes": std::fs::metadata(&binary.path).map(|m| m.len()).unwrap_or(0),
            })
        })
        .collect();
    stdout_line!(
        "{}",
        serde_json::json!({
            "sites": sites.len(),
            "guardedSites": chosen.len(),
            "mutants": live.len(),
            "unviable": built.unviable.len(),
            "unviableErrors": built.unviable.values().collect::<Vec<_>>(),
            "rounds": rounds,
            "ms": started.elapsed().as_millis() as u64,
            "binaries": binaries,
        })
    );
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A count of one reads as one, as people say it: the gate's pass and the baseline's catches.
    #[test]
    fn a_count_of_one_reads_as_one() {
        assert_eq!(catches(1, 1), "catch the only mutant");
        assert_eq!(catches(0, 1), "don't catch the only mutant");
        assert_eq!(catches(6, 6), "catch all 6 mutants");
        assert_eq!(catches(4, 6), "catch 4 of the 6 mutants");
        assert_eq!(catches(0, 2), "catch 0 of the 2 mutants");
        assert_eq!(caught_now(1), "1 of the baseline's mutants is caught now");
        assert_eq!(caught_now(2), "2 of the baseline's mutants are caught now");
        assert_eq!(caught_now(0), "0 of the baseline's mutants are caught now");
    }

    /// A command line's words, `mutastic` left off, as a shell gives them: split at spaces, except
    /// inside double quotes, which are taken away.
    fn words(line: &str) -> Vec<String> {
        let mut words = vec![String::new()];
        let mut quoted = false;
        for character in line.trim_start_matches("mutastic ").chars() {
            match character {
                '"' => quoted = !quoted,
                ' ' if !quoted => words.push(String::new()),
                _ => words.last_mut().unwrap().push(character),
            }
        }
        words
    }

    #[test]
    fn every_example_declared_parses() {
        for command in commands::COMMANDS {
            for example in command.examples {
                assert_eq!(check(&words(example)), Ok(()), "{example}");
            }
            for option in commands::options_for(command.name) {
                let example = option.example_for(command);
                assert!(example.starts_with(&format!("mutastic {}", command.name)));
                assert!(example.contains(&format!("--{}", option.name)), "{example}");
                assert_eq!(check(&words(&example)), Ok(()), "{example}");
            }
        }
    }

    /// An empty id is said to be one, not taken as no `--only` at all, which would run them all.
    #[test]
    fn an_empty_only_id_is_refused() {
        assert_eq!(
            check(&words("run --only=")).unwrap_err(),
            "--only needs a mutant's id: --only=ID, as a card shows it"
        );
        assert_eq!(
            check(&words("run --only=mb2f8ae57,,m1a2b3c4d")).unwrap_err(),
            "--only=mb2f8ae57,,m1a2b3c4d has an empty id between its commas: --only=ID,ID"
        );
    }

    /// `--at` is given again, or split by commas, on `run` and `mutants` and no other command.
    #[test]
    fn run_and_mutants_take_lines_and_no_other_command_does() {
        let given = parse("run", &words("--at=a.rs:1 --at=b.rs:2-3")[..]).unwrap();
        let split = parse("run", &words("--at=a.rs:1,b.rs:2-3")[..]).unwrap();
        assert_eq!(given.at, split.at);
        assert_eq!(given.at.len(), 2);
        assert!(partial(&given));
        assert_eq!(check(&words("mutants --at=src/lib.rs:26")), Ok(()));
        assert_eq!(
            check(&words("verify mb2f8ae57 --at=src/lib.rs:26")).unwrap_err(),
            "verify doesn't take --at: mutastic verify ID... [DIR]"
        );
        assert_eq!(
            check(&words("run --at=src/lib.rs:0")).unwrap_err(),
            "--at=src/lib.rs:0: a file's lines are numbered from 1, as an editor shows them"
        );
    }

    /// `--at` chooses the mutants, as `--only` does: not both, and not with what judges or puts
    /// together a whole run.
    #[test]
    fn lines_arent_given_with_ids_new_misses_or_shares() {
        assert_eq!(
            check(&words("run --at=src/lib.rs:26 --only=mb2f8ae57")).unwrap_err(),
            "--only and --at both choose the mutants that run: give one, the ids or the lines"
        );
        assert_eq!(
            check(&words("run --at=src/lib.rs:26 --fail-on-new")).unwrap_err(),
            "--fail-on-new judges a run of the package on its new misses: not with --at, whose \
             mutants each pass when they're caught"
        );
        let shares = checked(&words("run --at=src/lib.rs:26 --from-shards=shares")).unwrap_err();
        assert_eq!(shares.code, Code::Shards);
        assert!(
            shares
                .message
                .ends_with("not with --at, which the shares run with"),
            "{}",
            shares.message
        );
    }

    #[test]
    fn the_note_on_reports_names_the_option_that_chose() {
        assert_eq!(reports_left_out_with("--only"), REPORTS_LEFT_OUT);
        assert_eq!(
            reports_left_out_with("--at"),
            "note: --report is left out with --at: a run of some mutants writes no reports, as \
             it says nothing about the rest; run without --at to write them"
        );
    }

    /// A proposal says why in a form a person can check: an equivalent or unreachable mutant's
    /// reason has a because, and no proposal keeps the template's parts unfilled.
    #[test]
    fn a_proposal_that_a_mutant_cant_matter_says_why() {
        let argued_reason =
            "Differs only when total is 100. That can't happen because prices end in 9.";
        assert_eq!(argued(argued_reason, "equivalent"), Ok(()));
        assert_eq!(argued("logs only", "telemetry"), Ok(()));
        assert_eq!(argued("logs only", ""), Ok(()));
        let refused = argued("never", "unreachable").unwrap_err().message;
        assert!(
            refused.starts_with("a proposal that a mutant is unreachable needs a reason"),
            "{refused}"
        );
        assert!(refused.ends_with(&format!("--reason=\"{}\"", report::REASON)));
        assert!(argued(report::REASON, "telemetry")
            .unwrap_err()
            .message
            .starts_with("the reason still has the template's <case> or <why>"));
    }

    #[test]
    fn a_command_refuses_an_option_it_does_not_declare() {
        let refused = check(&words("verify mb2f8ae57 --threshold=70")).unwrap_err();
        assert!(
            refused.starts_with("verify doesn't take --threshold"),
            "{refused}"
        );
        let refused = check(&words("gate open --tries=5")).unwrap_err();
        assert!(
            refused.starts_with("gate open doesn't take --tries"),
            "{refused}"
        );
        assert_eq!(
            check(&words("run --thresold=70")).unwrap_err(),
            "run has no option --thresold. There's one called --threshold.\n  \
             mutastic run --threshold=70"
        );
        assert!(check(&words("ignore --prune=yes")).is_err());
    }

    #[test]
    fn each_choice_declared_is_taken_and_no_other() {
        for option in commands::OPTIONS.iter().filter(|o| !o.choices.is_empty()) {
            let command = commands::command(option.commands[0]).unwrap();
            let line = |value: &str| {
                let mut line = option.example_for(command);
                let given = line
                    .split(' ')
                    .find(|word| word.starts_with(&format!("--{}=", option.name)))
                    .unwrap()
                    .to_string();
                line = line.replace(&given, &format!("--{}={value}", option.name));
                words(&line)
            };
            for choice in option.choices {
                assert_eq!(check(&line(choice)), Ok(()), "--{}={choice}", option.name);
            }
            assert!(check(&line("nonsense")).is_err(), "--{}", option.name);
        }
    }

    /// `--operators` given again adds to what it chose, as in the Java edition, and a list that
    /// only leaves operators out leaves them out of the defaults.
    #[test]
    fn operators_given_again_add() {
        let parsed = parse(
            "run",
            &words("--operators=stronger --operators=-mutastic/boundary"),
        )
        .unwrap();
        assert_eq!(
            parsed.operators_given.as_deref(),
            Some("stronger,-mutastic/boundary")
        );
        assert!(!parsed.operators.contains(&crate::sites::BOUNDARY));
        assert!(parsed.operators.len() > crate::sites::DEFAULTS.len() - 1);
        let without = parse("run", &words("--operators=-mutastic/boundary")).unwrap();
        let mut expected = crate::sites::DEFAULTS.to_vec();
        expected.retain(|id| *id != crate::sites::BOUNDARY);
        assert_eq!(without.operators, expected);
    }

    #[test]
    fn the_defaults_are_the_ones_declared() {
        let parsed = parse("run", &[]).unwrap();
        let help = |name: &str| commands::option(name).unwrap().help;
        assert_eq!(parsed.threshold, 80.0);
        assert!(help("threshold").ends_with("; 80 unless set"));
        assert_eq!(parsed.tries, 3);
        assert!(help("tries").ends_with("; 3 unless set"));
        assert_eq!(parsed.unsafe_mode, UnsafeMode::Skip);
        assert!(help("unsafe").ends_with("; skip unless set"));
        let parsed = parse("next", &[]).unwrap();
        assert_eq!(parsed.top, 1);
        assert!(help("top").ends_with(&format!("; 1 unless set, at most {}", report::next::MOST)));
        assert_eq!(check(&words("next --top=25")), Ok(()));
        for wrong in ["0", "26", "five"] {
            assert!(
                check(&words(&format!("next --top={wrong}"))).is_err(),
                "{wrong}"
            );
        }
    }

    /// Every command the output offers for a mutant, and `next`'s for the next item, parses: the
    /// placeholders filled in as a person or an agent fills them, and the ids as a card names
    /// them, one or several.
    #[test]
    fn every_command_offered_for_a_mutant_parses() {
        let filled = |command: String| {
            command
                .replace("<equivalent|unreachable|…>", "unreachable")
                .replace("<id>", "mb2f8ae57")
                .split(" for each of ")
                .next()
                .unwrap()
                .to_string()
        };
        for ids in ["mb2f8ae57", "mb2f8ae57,m95f96847"] {
            for verb in report::MUTANT_COMMANDS {
                let command = filled(report::mutant_command(verb, ids, "fixtures/pricing"));
                // `explain` takes one mutant.
                if *verb == "explain" && ids.contains(',') {
                    continue;
                }
                assert_eq!(check(&words(&command)), Ok(()), "{command}");
            }
        }
        for dir in [".", "fixtures/pricing"] {
            let command = report::next_command(dir);
            assert_eq!(check(&words(&command)), Ok(()), "{command}");
        }
    }

    /// The help lists every option declared, and only those, and its examples parse.
    #[test]
    fn the_usage_lists_the_options_declared() {
        let general = crate::help::general();
        let listed: std::collections::BTreeSet<&str> = general
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
            .filter_map(|word| word.strip_prefix("--"))
            .filter(|name| *name != "help")
            .collect();
        let declared: std::collections::BTreeSet<&str> =
            commands::OPTIONS.iter().map(|option| option.name).collect();
        assert_eq!(listed, declared);
        for example in crate::help::EXAMPLES {
            assert_eq!(check(&words(example)), Ok(()), "{example}");
        }
    }

    /// Every line a mistyped command is answered with parses, as typed: what it offers works.
    #[test]
    fn every_line_offered_for_a_mistyped_one_parses() {
        for typed in [
            "rnu fixtures/pricing",
            "verfy mb2f8ae57",
            "run --formt=agent",
            "run --formt agent",
            "run --format=agnet",
            "run --thresold=70 --format=brief",
            "run --since main",
            "verify mb2f8ae57 --workrs=2",
            "--format=agent run",
            "run --report=jsno",
            "run --operators=strong",
        ] {
            let args = words(typed);
            let refused = match check(&args) {
                Err(refused) => refused,
                Ok(()) => panic!("{typed} parses"),
            };
            let offered = refused.lines().last().unwrap();
            let line = offered
                .strip_prefix("  mutastic ")
                .unwrap_or_else(|| panic!("{typed}: {refused}"));
            assert_eq!(check(&words(line)), Ok(()), "{typed}: {refused}");
            assert!(refused.lines().count() <= 3, "{typed}: {refused}");
        }
    }

    /// A proposal is a decision of its own: not with --accept, --remove or --prune, as in the Java
    /// edition.
    #[test]
    fn a_proposal_is_refused_with_another_decision() {
        for typed in [
            "ignore mb2f8ae57 --propose --accept",
            "ignore mb2f8ae57 --propose --remove",
            "ignore --propose --prune",
        ] {
            assert_eq!(
                check(&words(typed)),
                Err(
                    "--propose records a proposal: not with --accept, --remove or --prune"
                        .to_string()
                ),
                "{typed}"
            );
        }
    }

    /// Decision 1: an option that must take a value takes it after a space too; an optional one,
    /// `--since`, needs its `=`, and an option with nothing after it, or another option, has none.
    #[test]
    fn a_value_after_a_space_is_taken() {
        for typed in [
            "run --format agent fixtures/pricing",
            "next --top 3",
            "run --report json --report sarif:out.sarif",
            "ignore mb2f8ae57 --reason why",
            "run --edition rust",
        ] {
            assert_eq!(check(&words(typed)), Ok(()), "{typed}");
        }
        assert_eq!(
            joined(words("run --format agent --propose x --since main")),
            words("run --format=agent --propose x --since main")
        );
        for typed in [
            "run --since main",
            "run --format",
            "run --format --threshold=70",
        ] {
            assert!(check(&words(typed)).is_err(), "{typed}");
        }
    }

    /// A word no command takes ends in the help that lists what there is: no usage line, never
    /// the whole help.
    #[test]
    fn a_word_it_cant_place_ends_in_the_help_to_read() {
        for typed in ["run --verbose", "build-it", "--verbose"] {
            let refused = check(&words(typed)).unwrap_err();
            assert!(
                !refused.lines().any(|l| l.starts_with("Usage: ")),
                "{typed}: {refused}"
            );
            assert_eq!(refused.lines().count(), 2, "{typed}: {refused}");
            assert!(refused.ends_with(" --help"), "{typed}: {refused}");
        }
    }

    /// `run --help`, `run -h` and `help run` print run's help; nothing runs.
    #[test]
    fn help_after_a_command_is_that_commands() {
        let run = crate::help::of(commands::command("run").unwrap());
        for typed in [
            "run --help",
            "run -h",
            "help run",
            "run . --format=agent --help",
        ] {
            assert_eq!(help(&words(typed)), Ok(Some(run.clone())), "{typed}");
        }
        assert_eq!(help(&words("--help")), Ok(Some(crate::help::general())));
        let open = crate::help::of(commands::command("gate open").unwrap());
        assert_eq!(help(&words("gate open --help")), Ok(Some(open.clone())));
        assert!(help(&words("gate --help"))
            .unwrap()
            .unwrap()
            .starts_with(&open));
        assert_eq!(help(&words("run .")), Ok(None));
        assert_eq!(
            help(&words("help rnu")).unwrap_err().message,
            "mutastic has no command rnu. There's one called run.\n  mutastic help run"
        );
    }
}
