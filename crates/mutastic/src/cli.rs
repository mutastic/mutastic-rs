//! The `mutastic` command, also run as `cargo mutastic`.

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
use crate::{
    baseline, commands, gate, ignores, java, sites, testrun, untrusted, workspace, wrapper,
};

pub fn main(args: Vec<OsString>) -> ExitCode {
    if wrapper::active(&args) {
        return wrapper::run(&args);
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
    let args = joined(args);
    hook_panics(&args);
    let args = match edition::chosen(&args)
        .and_then(|(chosen, args)| front_door(chosen, &args).map(|forwarded| (forwarded, args)))
    {
        Ok((Some(code), _)) => return code,
        Ok((None, args)) => args,
        Err(failure) => return failed(&args, &failure),
    };
    match dispatch(&args) {
        Ok(code) => code,
        Err(failure) => failed(&args, &failure),
    }
}

/// The status mutastic exits with when it couldn't run: 2 for every failure until 2.0, when it
/// moves to the Java edition's 2, 3 and 4 (decision 2).
const FAILED: u8 = 2;

/// A panic is a bug in mutastic: it ends as any failure does, said on stderr and, in the agent and
/// jsonl formats, as a `run-failed` event with code `internal` (decision 2), not as Rust's own
/// message and exit 101, which a tool reading the stream can't tell from a crash. A panic in a
/// worker ends the run there, rather than leave the others to finish without it.
fn hook_panics(args: &[String]) {
    let args = args.to_vec();
    std::panic::set_hook(Box::new(move |panic| {
        failed(&args, &crate::failure::internal(panic));
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
    // A build's or a test's output comes with it: escaped, a line at a time.
    stderr_line!("mutastic: {}", untrusted::escape_lines(&failure.message));
    if streams_events(args) {
        stdout_line!(
            "{}",
            crate::failure::event(failure, &directory_typed(args), FAILED)
        );
    }
    ExitCode::from(FAILED)
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
/// project, or `--edition=java` says, and returns its exit status. `None` when this edition runs it.
fn front_door(chosen: Option<Edition>, args: &[String]) -> Result<Option<ExitCode>, Failure> {
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
    if route.edition == Edition::Rust {
        return Ok(None);
    }
    Ok(Some(java::forward(&java::engine()?, args)))
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

struct Parsed {
    positional: Vec<String>,
    only: Vec<String>,
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
    /// `--shard`: the share of the mutants this run runs.
    shard: Option<crate::shares::Shard>,
    /// `--from-shards`: the directory of the shares this run is made of.
    from_shards: Option<String>,
    /// `--top`: how many of `next`'s queue to show.
    top: usize,
    /// `--fail-on-new`: the run passes on the misses the baseline doesn't hold, not its score.
    fail_on_new: bool,
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
        rules: Vec::new(),
        help: false,
        hook: String::new(),
        tries: 3,
        no_suggestions: false,
        force_coverage: false,
        shard: None,
        from_shards: None,
        top: 1,
        fail_on_new: false,
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
        "format" => {
            parsed.format = Some(Format::parse(value).ok_or_else(|| {
                format!(
                    "--format: {value} isn't one of full, brief, concise, grouped, agent or jsonl"
                )
            })?)
        }
        "threshold" => parsed.threshold = f64::from(whole("--threshold", value, 0, 100)?),
        "workers" => {
            parsed.workers = value
                .parse()
                .ok()
                .filter(|&n: &usize| n > 0)
                .ok_or_else(|| format!("--workers: {value} isn't a positive number"))?
        }
        "operators" => {
            // A name only a rules file has is checked once the rules files are read.
            if let Ok(chosen) = operators(value) {
                parsed.operators = chosen;
            }
            parsed.operators_given = Some(value.to_string());
        }
        "unsafe" => {
            parsed.unsafe_mode = UnsafeMode::parse(value)
                .ok_or_else(|| format!("--unsafe: {value} isn't one of skip, enclosing or all"))?
        }
        "reason" => parsed.reason = Some(value.to_string()),
        "as" => {
            if !ignores::KINDS.contains(&value) {
                return Err(format!(
                    "--as: {value} isn't one of {}",
                    ignores::KINDS.join(", ")
                ));
            }
            parsed.kind = value.to_string()
        }
        "propose" => parsed.propose = true,
        "accept" => parsed.accept = true,
        "remove" => parsed.remove = true,
        "prune" => parsed.prune = true,
        "fail-on-new" => parsed.fail_on_new = true,
        "since" => parsed.since = Some(value.to_string()),
        "help" => parsed.help = true,
        "no-suggestions" => parsed.no_suggestions = true,
        "force-coverage" => parsed.force_coverage = true,
        "from-shards" => parsed.from_shards = Some(value.to_string()),
        "hook" => {
            if value != "claude" {
                return Err(format!("--hook: {value} isn't claude"));
            }
            parsed.hook = value.to_string();
        }
        "top" => {
            parsed.top = value
                .parse()
                .ok()
                .filter(|n: &usize| (1..=report::next::MOST).contains(n))
                .ok_or_else(|| {
                    format!(
                        "--top: {value} isn't a number from 1 to {}",
                        report::next::MOST
                    )
                })?
        }
        "tries" => {
            parsed.tries = value
                .parse()
                .ok()
                .filter(|n: &usize| (1..=100).contains(n))
                .ok_or_else(|| format!("--tries: {value} isn't a number from 1 to 100"))?
        }
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
            parsed.vocabulary =
                Some(report::Vocabulary::parse(value).ok_or_else(|| {
                    format!("--vocabulary: {value} isn't one of plain or classic")
                })?)
        }
        "color" => parsed.color = choice("--color", value)?,
        "hyperlinks" => parsed.hyperlinks = choice("--hyperlinks", value)?,
        "progress" => {
            parsed.progress = Shown::parse(value).ok_or_else(|| {
                format!("--progress: {value} isn't one of auto, bar, counter or none")
            })?
        }
        // Which edition runs is decided before this one reads its options.
        "edition" => {
            Edition::parse(value)
                .ok_or_else(|| format!("--edition: {value} isn't one of java or rust"))?;
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
    if command == "run" {
        sharing(parsed)?;
        if parsed.fail_on_new && !parsed.only.is_empty() {
            return Err(Code::OptionNotTaken.with(
                "--fail-on-new judges a run of the package on its new misses: not with --only, \
                 whose mutants each pass when they're caught",
            ));
        }
    }
    if takes_id {
        if let Some(given) = parsed.positional.first() {
            let mut named = ids(given).peekable();
            if named.peek().is_none() || !named.all(|id| is_mutant_id(&id)) {
                return Err(Code::NeedsId.with(needs_id(command, given, usage)));
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

/// A run is a share or made of them, not both; made of them, it runs no mutants itself, so the
/// options for what runs are the shares'.
fn sharing(parsed: &Parsed) -> Result<(), Failure> {
    if parsed.from_shards.is_none() {
        return Ok(());
    }
    let given = [
        ("--shard", parsed.shard.is_some()),
        ("--only", !parsed.only.is_empty()),
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

/// What's wrong with the word where `command` needs a mutant's id, with an example: one that keeps
/// the directory, when that's what was given.
fn needs_id(command: &str, given: &str, usage: &str) -> String {
    let directory = std::path::Path::new(given).is_dir();
    let what = if directory {
        format!("{given} is a directory")
    } else {
        format!("{given} isn't one")
    };
    let dir = if directory {
        format!(" {given}")
    } else {
        String::new()
    };
    let reason = if command == "ignore" {
        " --reason=\"…\""
    } else {
        ""
    };
    format!(
        "{command} needs a mutant's id, m and eight hex digits as a card shows it, and {what}: \
         {usage}, as in mutastic {command} m1a2b3c4d{dir}{reason}"
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
    parsed.rules = config.rules;
    match &parsed.operators_given {
        Some(given) => parsed.operators = operators(given)?,
        None if !config.operators.is_empty() => {
            parsed.operators =
                crate::config::resolve_from(&config.operators, &config.operators_from)?;
        }
        None => {}
    }
    Ok(())
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
    let given = parsed.positional.get(slot).map_or(".", String::as_str);
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
    if located.dir != given {
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
        _ => Err(format!(
            "{option}: {value} isn't one of auto, always or never"
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
    Style {
        color,
        links,
        words,
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
        operators: parsed.operators.clone(),
        threshold: parsed.threshold,
        workers: parsed.workers,
        unsafe_mode: parsed.unsafe_mode,
        since: None,
        rules: parsed.rules.clone(),
        suggestions: false,
        shard: None,
        force_coverage: parsed.force_coverage,
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
                crate::shares::code(&project)?,
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
    if format == Format::Jsonl {
        progress.stream(crate::progress::Events {
            unsafe_mode: parsed.unsafe_mode,
            share: share.as_ref().map(crate::shares::Tag::json),
        });
    }
    let mut report = engine::run(&project, &options, &|step| progress.step(step))?;
    report.share = share;
    progress.finished();
    report.streamed = progress.streamed();
    finish(parsed, report, format, folder.as_deref())
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
    let code = crate::shares::code(&project)?;
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
    if parsed.only.is_empty() && report.scope.is_none() && kept {
        report.comparison = recorded(&report, &folder)?;
    }
    // Only a full run writes reports: a run of some mutants would leave out the rest.
    if parsed.only.is_empty() && (kept || parsed.reports.is_some()) {
        let reports = parsed.reports.clone().unwrap_or_else(|| {
            report::files::DEFAULT
                .iter()
                .map(|(kind, path)| (*kind, path.map(str::to_string)))
                .collect()
        });
        report.reports = report::files::write(&report, &reports, &folder)
            .map_err(|error| Code::Other.with(error))?;
    }
    // A share's events, kept for `--from-shards`: as `--format=jsonl` writes them, whatever the
    // format shown.
    if report.share.is_some() {
        let events = folder.join(EVENTS);
        let given = std::path::Path::new(&report.project.given);
        // Written empty first, so its path is shown as the reports' are.
        crate::shares::keep(&events, "")?;
        report.reports.push(report::files::shown(given, &events));
        // Whole, `run-started` included, even when it went out live.
        let streamed = std::mem::replace(&mut report.streamed, false);
        crate::shares::keep(
            &events,
            &report::render(&report, Format::Jsonl, Style::default()),
        )?;
        report.streamed = streamed;
    }
    report.fail_on_new = judged_on_new(parsed);
    stdout_text!(
        "{}",
        report::render(
            &report,
            format,
            style(parsed, std::io::stdout().is_terminal())
        )
    );
    if format != Format::Jsonl && parsed.only.is_empty() && report.scope.is_none() {
        baseline_note(&report);
    }
    if !report.failing_alone().is_empty() {
        // A test fails on the code with no mutant in, as one that fails before any mutant does.
        return Ok(ExitCode::from(2));
    }
    Ok(if report.passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// After a full run, how many of the baseline's mutants it found caught: they can be pruned.
fn baseline_note(report: &engine::Report) {
    let older = baseline::ids(std::path::Path::new(&report.project.given));
    let caught = baseline::caught_now(report, &older).len();
    if !older.is_empty() && caught > 0 {
        stdout_line!(
            "note: {caught} of the baseline's mutants are caught now: `mutastic baseline --prune` drops them"
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
    let progress = start(parsed, false);
    let project = workspace::read(dir)?;
    progress.package(&project.package);
    let report = engine::run(&project, &options(parsed, Vec::new()), &|step| {
        progress.step(step)
    })?;
    progress.finished();
    if parsed.prune {
        let caught = baseline::caught_now(&report, &baseline::ids(here));
        let dropped = baseline::prune(here, &caught).map_err(|error| Code::Other.with(error))?;
        stdout_line!(
            "mutastic: {dropped} of the baseline's mutants are caught now, and are dropped from {}",
            baseline::FILE
        );
    } else {
        let entries = baseline::entries(&report);
        baseline::write(here, &entries).map_err(|error| Code::Other.with(error))?;
        stdout_line!(
            "mutastic: {} no test catches {} in {}: commit it. A miss that isn't in it is new: the \
             gate holds only new misses, and next offers them first",
            crate::progress::count(entries.len(), "mutant"),
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
        parsed.since.as_deref().unwrap_or("-"),
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
            .map_err(|error| Code::Other.with(error))?;
    }
    let proposed: std::collections::BTreeSet<String> =
        ignores::proposed(std::path::Path::new(dir))?
            .into_iter()
            .map(|entry| entry.id)
            .collect();
    let older = baseline::ids(std::path::Path::new(dir));
    let asked = report::next::Asked {
        proposed: &proposed,
        baseline: &older,
        project: &project.package,
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
    let mut report = engine::run(project, &options, &|step| progress.step(step))?;
    progress.finished();
    if whole && report.scope.is_none() && report.built {
        let folder = project.mutastic_dir();
        report.comparison = recorded(&report, &folder)?;
        let reports: Vec<(Kind, Option<String>)> = report::files::DEFAULT
            .iter()
            .map(|(kind, path)| (*kind, path.map(str::to_string)))
            .collect();
        report.reports = report::files::write(&report, &reports, &folder)
            .map_err(|error| Code::Other.with(error))?;
    }
    Ok(report)
}

/// Keeps `report` as the last run in `folder`, with its comparison with the one before: only an
/// earlier run of the same package, in the same place, under the same operators, says anything
/// about it. A share's folder is its own, so a whole run is never compared with part of one.
fn recorded(
    report: &engine::Report,
    folder: &std::path::Path,
) -> Result<Option<crate::record::Comparison>, Failure> {
    let code = crate::shares::code(&report.project)?;
    let record = crate::record::RunRecord::of(report, &code, &crate::record::tool());
    let records = crate::records::RunRecords::new(folder);
    let comparison = records
        .comparable_to(&record)
        .map(|previous| record.against(&previous));
    records.keep(&record).map_err(|error| {
        Code::Other.with(format!(
            "couldn't keep the run's record in {}: {error}",
            folder.display()
        ))
    })?;
    Ok(comparison)
}

fn verify(parsed: &Parsed) -> Result<ExitCode, Failure> {
    let (only, taken) = verify_ids(&parsed.positional);
    if only.is_empty() {
        return Err(Code::NeedsId.with("verify needs a mutant's id: mutastic verify ID... [DIR]"));
    }
    let dir = parsed.positional.get(taken).map_or(".", String::as_str);
    let progress = start(parsed, false);
    let project = workspace::read(dir)?;
    progress.package(&project.package);
    let report = engine::run(&project, &options(parsed, only), &|step| {
        progress.step(step)
    })?;
    progress.finished();
    let mut settled = true;
    for result in &report.results {
        let mutant = &report.found.mutants[result.mutant];
        let fate = match result.status {
            Status::Killed => caught(result),
            Status::Timeout => "is caught by a timeout".to_string(),
            Status::Survived => "is still missed".to_string(),
            Status::NoCoverage => "isn't reached by any test".to_string(),
            Status::Ignored => format!(
                "is skipped ({}), so it isn't run and needs no test",
                untrusted::value(result.reason.as_deref().unwrap_or("skipped"))
            ),
            Status::RuntimeError => format!(
                "crashes, and no test can be said to catch it ({}): it's left out of the score; check the unsafe code it reaches with Miri",
                untrusted::value(result.reason.as_deref().unwrap_or("crash"))
            ),
        };
        settled &= result.status.settled();
        stdout_line!("mutastic: {} {fate}", mutant.id);
    }
    if !report.failing_alone().is_empty() {
        stdout_text!("{}", report::failing_alone(&report));
        return Ok(ExitCode::from(2));
    }
    Ok(if settled {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// "is caught by tests::big_orders", and how, when it wasn't by a test's own check.
/// `explain <id>`: runs the mutant, then shows its card, or what became of it, and what's known of
/// it, with how the last full run went. It explains; it doesn't judge, so it exits 0.
fn explain(parsed: &Parsed) -> Result<ExitCode, Failure> {
    let Some(id) = parsed.positional.first() else {
        return Err(Code::NeedsId.with("explain needs a mutant's id: mutastic explain ID [DIR]"));
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
    let report = engine::run(&project, &options, &|step| progress.step(step))?;
    progress.finished();
    let last = report::last_run(&project.mutastic_dir().join("mutation.json"), id);
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
        return Err(
            Code::NeedsId.with("ignore needs a mutant's id: mutastic ignore ID [DIR] --reason=WHY")
        );
    };
    let dir = parsed
        .positional
        .get(1)
        .map_or(".", String::as_str)
        .to_string();
    let here = std::path::Path::new(&dir);
    let command = |words: &[&str]| {
        let words: Vec<String> = words.iter().map(|word| shell(word)).collect();
        format!(
            "mutastic ignore {} {}",
            words.join(" "),
            crate::report::shell_word(&dir)
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
            "ignore needs a reason, for whoever reviews it: mutastic ignore {id} --reason=\"<why>\" {dir}"
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
            ignores::PROPOSALS,
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
        ignores::FILE,
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
            "mutastic: {id} isn't proposed in {}: nothing to accept",
            ignores::PROPOSALS
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
        ignores::FILE,
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
            ignores::FILE,
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
            ignores::PROPOSALS,
            again(proposal, true)
        );
        return Ok(ExitCode::SUCCESS);
    }
    stderr_line!(
        "mutastic: {id} isn't ignored in {}: nothing to remove",
        ignores::FILE
    );
    Ok(ExitCode::FAILURE)
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
        "mutastic: removed {} ignore{} that no longer hold: {}\n  from {}\n  to undo: git checkout -- {}",
        stale.len(),
        if stale.len() == 1 { "" } else { "s" },
        ids.join(", "),
        ignores::FILE,
        ignores::FILE
    );
    Ok(ExitCode::SUCCESS)
}

/// A word of a command mutastic offers, as a shell takes it: as it is when it's plain, else in
/// single quotes, where nothing in it is run, and with what can't be seen escaped. As the Java
/// edition's `Invocation.quoted`.
fn shell(word: &str) -> String {
    crate::report::shell_word(word)
}

/// `gate open`, `gate check` or `gate close`, as the Java edition's: see [`gate`].
fn gate_command(parsed: &Parsed) -> Result<ExitCode, Failure> {
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
        "open" => gate_open(hooked, &project, here, &session),
        "close" => {
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
        _ => gate_check(parsed, &project, here, &work),
    }
}

fn gate_help(action: &str) -> Result<String, Failure> {
    commands::command(&format!("gate {action}"))
        .map(crate::help::of)
        .ok_or_else(|| Code::UnknownCommand.with(no_gate_action(action)))
}

/// `gate`'s action: open, check or close. Checked before the project is looked for, so a mistyped
/// one is said as that wherever it's run.
fn gate_action(parsed: &Parsed) -> Result<&str, Failure> {
    match parsed.positional.first().map(String::as_str) {
        None => {
            Err(Code::UnknownCommand
                .with("gate needs open, check or close: mutastic gate open [DIR]"))
        }
        Some(action @ ("open" | "check" | "close")) => Ok(action),
        Some(other) => Err(Code::UnknownCommand.with(no_gate_action(other))),
    }
}

/// `gate`'s action isn't one it has.
fn no_gate_action(action: &str) -> String {
    let actions = ["open", "check", "close"];
    format!(
        "gate: {action} isn't one of open, check or close.{}",
        crate::similar::offered(action, actions)
    )
}

/// Keeps where the task starts; with a hook, once for each session, and at each later prompt takes
/// in the ignores the person made between turns. A hook's output is the agent's to read: it's told
/// the task is gated, and how to check its work as it goes.
fn gate_open(
    hooked: bool,
    project: &workspace::Project,
    here: &std::path::Path,
    session: &str,
) -> Result<ExitCode, Failure> {
    let work = &project.mutastic_dir();
    let open =
        gate::State::read(work).filter(|state| !session.is_empty() && state.session == session);
    if let Some(open) = open.filter(|_| hooked) {
        // A later prompt: what the person ignored between turns is the task's to start from.
        if open.stopped.is_some() {
            open.prompted(&ignored_ids(here)?)
                .write(work)
                .map_err(|error| Code::Other.with(error))?;
        }
        return Ok(ExitCode::SUCCESS);
    }
    let Some(base) = crate::since::git(here, &["rev-parse", "HEAD"]) else {
        return Err(Code::Git.with("the gate needs a git repository with a commit"));
    };
    let base = base.trim().to_string();
    // The tests as libtest lists them, each case its own; from the source where the package
    // doesn't build yet, and fixing that is the task.
    let shape = gate::listed(project).map_or_else(
        |_| gate::Shape::of(here),
        |names| gate::Shape::listed(&names),
    );
    let state = gate::State {
        session: session.to_string(),
        base: base.clone(),
        ignored: ignored_ids(here)?,
        shape,
        ..gate::State::default()
    };
    state.write(work).map_err(|error| Code::Other.with(error))?;
    stdout_line!(
        "mutastic gate: this task is gated from {}: before you finish, check the lines it changes with `mutastic run --since={base} --format=agent`; {}. {}",
        &base[..base.len().min(7)],
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
) -> Result<ExitCode, Failure> {
    let hooked = !parsed.hook.is_empty();
    let Some(open) = gate::State::read(work) else {
        if !hooked {
            stderr_line!("mutastic gate: none open here: `mutastic gate open` starts one");
        }
        // Nothing gated: a hooked agent may stop.
        return Ok(ExitCode::from(if hooked { 0 } else { 2 }));
    };
    let verdict = gate_verdict(parsed, project, here, &open)?;
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
        state.write(work).map_err(|error| Code::Other.with(error))?;
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
        state.write(work).map_err(|error| Code::Other.with(error))?;
        return Ok(gate_answer(hooked, &verdict));
    };
    state.blocks = 0;
    state.tree.clear();
    state.stopped = stopped;
    state.write(work).map_err(|error| Code::Other.with(error))?;
    let answer = for_the_person(here, &open, gate::Verdict::pass(text));
    Ok(gate_answer(hooked, &answer))
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
    let older = baseline::ids(here);
    let baselined: Vec<String> = older
        .difference(&baseline::ids_at(here, &state.base))
        .cloned()
        .collect();
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
    if !ours {
        // A change to a test, a build file or a test's data may still break a test, so they run.
        return match gate_run(parsed, project, changed) {
            Ok(_) => Ok(gate::Verdict::pass(
                "mutastic gate: passed: the tests pass, and the task changed no lines mutastic mutates"
                    .into(),
            )),
            Err(error) => red_verdict(here, state, &error),
        };
    }
    let mut report = match gate_run(parsed, project, changed) {
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
                "mutastic gate: passed: the tests pass, and the lines the task changed hold no mutant to run"
                    .into(),
            ));
        }
        let scored = crate::progress::count(scored, "mutant");
        return Ok(gate::Verdict::pass(format!(
            "mutastic gate: passed: the tests catch {caught} of the {scored} on the lines the task changed{waiting}{backlog}"
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
        gate::held_by(&open_missed, &open_uncovered, &waiting)
    )))
}

/// The gate's run of the lines the task changed. It always builds and runs the tests, as
/// `--force-coverage` does: a change with no mutant on its lines (a constant, an import, a log line)
/// can still break a test, and the gate mustn't pass it.
fn gate_run(
    parsed: &Parsed,
    project: &workspace::Project,
    changed: crate::since::ChangedLines,
) -> Result<engine::Report, Failure> {
    let mut options = options(parsed, Vec::new());
    options.since = Some(changed);
    options.force_coverage = true;
    let progress = Progress::start(Shown::None, Style::default(), (false, parsed.workers), None);
    engine::run(project, &options, &|step| progress.step(step))
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
    let named: Vec<String> = listed
        .split(", ")
        .map(|test| {
            let name = test.trim_end_matches(" (it timed out)");
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
fn ids_shown(ids: &[String]) -> String {
    ids.iter()
        .map(|id| untrusted::value(id))
        .collect::<Vec<_>>()
        .join(", ")
}

fn caught(result: &engine::MutantResult) -> String {
    let how = result.killed_with.as_deref().unwrap_or("panic");
    if let Some(crash) = how.strip_prefix("crash: ") {
        return format!(
            "is caught only because the test process crashed ({}): no test's check caught it",
            untrusted::value(crash)
        );
    }
    let test = untrusted::value(result.killed_by.as_deref().unwrap_or("a test"));
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
    let found = engine::find_with(&project, &parsed.operators, parsed.unsafe_mode);
    for mutant in &found.mutants {
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
    stdout_line!("{}", crate::progress::count(found.mutants.len(), "mutant"));
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

    /// A word no command takes gets one line of usage, never the whole help.
    #[test]
    fn a_word_it_cant_place_gets_one_line_of_usage() {
        for typed in ["run --verbose", "build-it", "--verbose"] {
            let refused = check(&words(typed)).unwrap_err();
            let usage: Vec<&str> = refused
                .lines()
                .filter(|l| l.starts_with("Usage: "))
                .collect();
            assert_eq!(usage.len(), 1, "{typed}: {refused}");
            assert!(refused.lines().count() <= 3, "{typed}: {refused}");
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
