//! The `mutastic` command, also run as `cargo mutastic`.

use std::ffi::OsString;
use std::io::IsTerminal;
use std::process::ExitCode;

use crate::edition::{self, Edition, Route};
use crate::engine::{self, Options, Status};
use crate::report::files::Kind;
use crate::report::{self, Format, Style};
use crate::sites::UnsafeMode;
use crate::{gate, ignores, java, sites, testrun, workspace, wrapper};

const USAGE: &str = "\
mutastic: find the tests that run your code but don't check it

Usage:
  mutastic run [dir] [options]       Mutate the package in dir (default .) and run its tests
  mutastic verify <id>… [dir]        Pass when a test catches each mutant named, the ids split
                                     by commas or spaces
  mutastic explain <id> [dir]        Everything known of one mutant, with its tests run
  mutastic ignore <id> [dir] --reason=<why>
                                     Record a decision not to test one mutant, in
                                     .mutastic/ignores.yaml; --as=<kind> says what sort; --propose
                                     proposes it for a person to accept (--accept <id>);
                                     --remove <id> takes one back; --prune those whose line changed
  mutastic gate open|check|close [dir]
                                     Gate a coding agent's task on its changed lines' mutants:
                                     open keeps where it starts, check says whether it's done
                                     (--hook=claude answers as Claude Code's hooks, --tries=<n>),
                                     close forgets it; gate <action> --help says more
  mutastic mutants [dir]             List the mutants, without running anything

Options:
  --only=<id>[,<id>…]   Run only these mutants
  --format=<format>     full (default at a terminal), concise, agent or jsonl
  --threshold=<score>   The score a run must reach to pass (default 80)
  --operators=<set>     defaults, stronger or extended, or operator ids separated by commas
  --workers=<n>         Tests run side by side (default half the cores, at most 4)
  --unsafe=<mode>       skip (default): leave unsafe code and functions holding an unsafe block
                        alone; enclosing: mutate those functions too; all: unsafe code as well
  --since[=<ref>]       Run only the mutants on lines changed since the merge-base of HEAD and
                        <ref> (default: the default branch), uncommitted and new files included
  --report=<kind>[:<path>]  Write a report file: json (mutation.json) or html (mutation.html), in
                        target/mutastic unless a path is given; repeat it for more, or none.
                        A full run writes both by default
  --no-suggestions      Don't look for a test that catches each missed mutant, which runs the
                        missed mutant's function outside the tests
  --progress=<mode>     auto (default): a line redrawn in place at a terminal, none for agent
                        and jsonl; bar, counter (a line every so often) or none
  --edition=<edition>   rust or java: which edition runs. Unless given, the build file in dir, or
                        the nearest above it, decides: Cargo.toml for rust; pom.xml, build.gradle
                        or settings.gradle for java, which is fetched the first time it's needed

Exit status: 0 when the run passes, or verify's mutants are caught; 1 when it doesn't; 2 when
mutastic couldn't run.";

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
    let args = match edition::chosen(&args)
        .and_then(|(chosen, args)| front_door(chosen, &args).map(|forwarded| (forwarded, args)))
    {
        Ok((Some(code), _)) => return code,
        Ok((None, args)) => args,
        Err(message) => {
            eprintln!("mutastic: {message}");
            return ExitCode::from(2);
        }
    };
    match dispatch(&args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("mutastic: {message}");
            ExitCode::from(2)
        }
    }
}

/// Hands the command to the Java edition when the project it points at is a Maven or Gradle
/// project, or `--edition=java` says, and returns its exit status. `None` when this edition runs it.
fn front_door(chosen: Option<Edition>, args: &[String]) -> Result<Option<ExitCode>, String> {
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
            eprintln!("{note}");
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
}

fn parse(args: &[String]) -> Result<Parsed, String> {
    let mut parsed = Parsed {
        positional: Vec::new(),
        only: Vec::new(),
        format: None,
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
    };
    for arg in args {
        let Some(option) = arg.strip_prefix("--") else {
            parsed.positional.push(arg.clone());
            continue;
        };
        let (name, value) = option.split_once('=').unwrap_or((option, ""));
        match name {
            "only" => parsed.only.extend(ids(value)),
            "format" => {
                parsed.format = Some(Format::parse(value).ok_or_else(|| {
                    format!("--format: {value} isn't one of full, concise, agent or jsonl")
                })?)
            }
            "threshold" => {
                parsed.threshold = value
                    .parse()
                    .map_err(|_| format!("--threshold: {value} isn't a number"))?
            }
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
                parsed.unsafe_mode = UnsafeMode::parse(value).ok_or_else(|| {
                    format!("--unsafe: {value} isn't one of skip, enclosing or all")
                })?
            }
            "reason" => parsed.reason = Some(value.to_string()),
            "as" => parsed.kind = value.to_string(),
            "propose" => parsed.propose = true,
            "accept" => parsed.accept = true,
            "remove" => parsed.remove = true,
            "prune" => parsed.prune = true,
            "since" => parsed.since = Some(value.to_string()),
            "help" => parsed.help = true,
            "no-suggestions" => parsed.no_suggestions = true,
            "hook" => {
                if value != "claude" {
                    return Err(format!("--hook: {value} isn't claude"));
                }
                parsed.hook = value.to_string();
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
                        format!("--report: unknown report {kind} (one of json, html, none)")
                    })?;
                    reports.push((kind, path));
                }
            }
            "progress" => {
                parsed.progress = match value {
                    "auto" => None,
                    "bar" => Some(Shown::Bar),
                    "counter" => Some(Shown::Counter),
                    "none" => Some(Shown::None),
                    _ => {
                        return Err(format!(
                            "--progress: {value} isn't one of auto, bar, counter or none"
                        ))
                    }
                }
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
            _ => return Err(format!("unknown option --{name}\n\n{USAGE}")),
        }
    }
    Ok(parsed)
}

/// Whether `args` (without the program's name) is a command mutastic takes, with options it knows,
/// without running it: for checking the commands the docs show.
pub fn check(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first() else {
        return Err("no command".into());
    };
    match command.as_str() {
        "run" | "verify" | "explain" | "ignore" | "gate" | "mutants" | "help" | "--help" | "-h"
        | "--version" | "-V" => {
            let parsed = parse(&args[1..])?;
            arguments(command, &parsed)?;
            match &parsed.operators_given {
                Some(given) => operators(given).map(|_| ()),
                None => Ok(()),
            }
        }
        other => Err(format!("unknown command {other}")),
    }
}

/// Whether the words that aren't options are ones `command` takes: a mutant's id where it needs
/// one, then a directory, and nothing after. A word it doesn't take would otherwise be dropped
/// without a word, and the run would be one nobody asked for.
fn arguments(command: &str, parsed: &Parsed) -> Result<(), String> {
    let (usage, takes_id, most) = match command {
        "run" => ("mutastic run [dir] [options]", false, 1),
        "mutants" => ("mutastic mutants [dir]", false, 1),
        "build" => ("mutastic build [dir] --fraction=<share>", false, 1),
        "verify" => (
            "mutastic verify <id>[,<id>…] [<id>…] [dir]",
            true,
            1 + verify_ids(&parsed.positional).1,
        ),
        "explain" => ("mutastic explain <id> [dir]", true, 2),
        "ignore" if parsed.prune => ("mutastic ignore --prune [dir]", false, 1),
        "ignore" => ("mutastic ignore <id> [dir] --reason=<why>", true, 2),
        "gate" => ("mutastic gate open|check|close [dir]", false, 2),
        _ => return Ok(()),
    };
    if takes_id {
        if let Some(given) = parsed.positional.first() {
            let mut named = ids(given).peekable();
            if named.peek().is_none() || !named.all(|id| is_mutant_id(&id)) {
                return Err(needs_id(command, given, usage));
            }
        }
    }
    match parsed.positional.get(most..) {
        Some(extra) if !extra.is_empty() => Err(format!(
            "{command} doesn't take {}: {usage}",
            extra.join(" ")
        )),
        _ => Ok(()),
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

fn operators(value: &str) -> Result<Vec<&'static str>, String> {
    let entries: Vec<String> = value.split(',').map(|e| e.trim().to_string()).collect();
    crate::config::resolve(&entries, "--operators")
}

/// Reads `mutastic.yaml` in the package's directory, if there is one: its rules, its rules files'
/// operators, and the operators it chooses, unless `--operators` chose.
fn configure(command: &str, parsed: &mut Parsed) -> Result<(), String> {
    let at = match command {
        "run" | "mutants" => 0,
        "ignore" if parsed.prune => 0,
        "verify" => verify_ids(&parsed.positional).1,
        _ => 1,
    };
    let dir = std::path::PathBuf::from(parsed.positional.get(at).map_or(".", String::as_str));
    let config = crate::config::Configuration::read(&dir)?;
    let mut rules = Vec::new();
    for plugin in &config.plugins {
        rules.extend(crate::callrules::read(&dir.join(plugin))?);
    }
    crate::callrules::load(rules);
    parsed.rules = config.rules;
    match &parsed.operators_given {
        Some(given) => parsed.operators = operators(given)?,
        None if !config.operators.is_empty() => {
            parsed.operators = crate::config::resolve(&config.operators, crate::config::FILE)?;
        }
        None => {}
    }
    Ok(())
}

fn dispatch(args: &[String]) -> Result<ExitCode, String> {
    let Some(command) = args.first() else {
        println!("{USAGE}");
        return Ok(ExitCode::from(2));
    };
    let mut parsed = parse(&args[1..])?;
    arguments(command, &parsed)?;
    if matches!(
        command.as_str(),
        "run" | "verify" | "explain" | "ignore" | "mutants" | "gate"
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
        // For measuring the schema itself: build with a share of the mutants, and say how it went.
        "build" => build(&parsed),
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        "--version" | "-V" => {
            println!("{}", versions());
            Ok(ExitCode::SUCCESS)
        }
        other => Err(format!("unknown command {other}\n\n{USAGE}")),
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

/// Colour for a stream that's a terminal, or when `FORCE_COLOR` asks; never with `NO_COLOR` or a
/// dumb terminal, unless forced.
fn style(terminal: bool) -> Style {
    let forced = std::env::var_os("FORCE_COLOR").is_some();
    let plain = std::env::var_os("NO_COLOR").is_some()
        || std::env::var("TERM").is_ok_and(|term| term == "dumb");
    Style {
        color: forced || (!plain && terminal),
    }
}

/// How progress is shown while mutants run: `bar`, one line redrawn in place on a terminal;
/// `counter`, that line printed every so often, for logs; or `none`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shown {
    Bar,
    Counter,
    None,
}

/// What the run is doing, on stderr so stdout stays the result: what it's reading, the build, the
/// coverage pass, with `files` each source file's line as its last mutant finishes, and the
/// counter, `[3/11] ✓ 2 caught · ✗ 1 missed · ○ 0 not covered · ⏱ 0 timed out  0:01`, as the
/// Java edition shows it. Nothing when `quiet`, for agents and tools, who only want the result.
struct Progress<'a> {
    package: &'a str,
    quiet: bool,
    files: bool,
    shown: Shown,
    style: Style,
    started: std::time::Instant,
    counter: std::sync::Mutex<Counter>,
}

#[derive(Default)]
struct Counter {
    done: usize,
    total: usize,
    caught: usize,
    missed: usize,
    not_covered: usize,
    timed_out: usize,
    printed: Option<std::time::Instant>,
}

/// The counter line is printed this often when it can't be redrawn in place.
const EVERY: std::time::Duration = std::time::Duration::from_secs(10);

/// Clears the line the bar is on.
const CLEAR: &str = "\r\x1b[2K";

impl<'a> Progress<'a> {
    fn new(package: &'a str, quiet: bool, files: bool, asked: Option<Shown>) -> Progress<'a> {
        let terminal = std::io::stderr().is_terminal();
        let shown = match asked {
            _ if quiet => Shown::None,
            Some(shown) => shown,
            None if terminal => Shown::Bar,
            None => Shown::Counter,
        };
        Progress {
            package,
            quiet,
            files,
            shown,
            style: style(terminal),
            started: std::time::Instant::now(),
            counter: std::sync::Mutex::new(Counter::default()),
        }
    }

    fn line(&self, counter: &Counter) -> String {
        let elapsed = self.started.elapsed().as_secs();
        format!(
            "[{}/{}] ✓ {} caught · ✗ {} missed · ○ {} not covered · ⏱ {} timed out  {}:{:02}",
            counter.done,
            counter.total,
            counter.caught,
            counter.missed,
            counter.not_covered,
            counter.timed_out,
            elapsed / 60,
            elapsed % 60
        )
    }

    fn step(&self, step: engine::Progress) {
        if self.quiet {
            return;
        }
        let Ok(mut counter) = self.counter.lock() else {
            return;
        };
        match step {
            engine::Progress::Building { mutants } => {
                eprintln!("building  {mutants} mutants in one build…")
            }
            engine::Progress::Covered {
                tests,
                rustc,
                took,
                mutants,
                flaky,
            } => {
                counter.total = mutants;
                eprintln!(
                    "coverage  {} on rustc {rustc} in {}{} · {}",
                    count(tests, "test"),
                    report::duration(took),
                    if flaky == 0 {
                        String::new()
                    } else {
                        format!(" ({flaky} flaky, left out)")
                    },
                    count(mutants, "mutant")
                )
            }
            engine::Progress::MutantFinished { status } => {
                counter.done += 1;
                match status {
                    Status::Killed => counter.caught += 1,
                    Status::Survived => counter.missed += 1,
                    Status::NoCoverage => counter.not_covered += 1,
                    Status::Timeout => counter.timed_out += 1,
                    Status::Ignored | Status::RuntimeError => {}
                }
                self.show(&mut counter);
            }
            engine::Progress::FileFinished { found, results } if self.files => {
                let bar = self.shown == Shown::Bar;
                eprint!(
                    "{}{}{}",
                    if bar { CLEAR } else { "" },
                    report::file_finished(self.package, found, results, self.style),
                    if bar {
                        self.line(&counter)
                    } else {
                        String::new()
                    }
                );
            }
            engine::Progress::FileFinished { .. } => {}
        }
    }

    fn show(&self, counter: &mut Counter) {
        match self.shown {
            Shown::Bar => eprint!("{CLEAR}{}", self.line(counter)),
            // With each file's line, a log already shows how far the run is.
            Shown::Counter if !self.files => {
                let due = counter.printed.is_none_or(|at| at.elapsed() >= EVERY);
                if due || counter.done == counter.total {
                    eprintln!("{}", self.line(counter));
                    counter.printed = Some(std::time::Instant::now());
                }
            }
            Shown::Counter | Shown::None => {}
        }
    }

    /// Clears the bar, before the result.
    fn finished(&self) {
        let done = self.counter.lock().map_or(0, |counter| counter.done);
        if self.shown == Shown::Bar && done > 0 {
            eprint!("{CLEAR}");
        }
    }
}

/// `1 test`, `3 tests`.
fn count(number: usize, noun: &str) -> String {
    format!("{number} {noun}{}", if number == 1 { "" } else { "s" })
}

/// The first line, before anything slow.
fn started(quiet: bool) {
    if !quiet {
        eprintln!(
            "mutastic {} · reading the Cargo package…",
            env!("CARGO_PKG_VERSION")
        );
    }
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
    }
}

fn run(parsed: &Parsed) -> Result<ExitCode, String> {
    let dir = parsed.positional.first().map_or(".", String::as_str);
    let format = format(parsed);
    let quiet = matches!(format, Format::Agent | Format::Jsonl);
    started(quiet);
    let project = workspace::read(dir)?;
    let progress = Progress::new(&project.package, quiet, true, parsed.progress);
    let mut options = options(parsed, parsed.only.clone());
    options.suggestions = !parsed.no_suggestions;
    if let Some(since) = &parsed.since {
        options.since = Some(crate::since::changed_since(
            std::path::Path::new(dir),
            since,
        )?);
    }
    let mut report = engine::run(&project, &options, &|step| progress.step(step))?;
    progress.finished();
    // Only a full run writes reports: a run of some mutants would leave out the rest.
    if parsed.only.is_empty() {
        let reports = parsed.reports.clone().unwrap_or_else(|| {
            report::files::DEFAULT
                .iter()
                .map(|(kind, path)| (*kind, path.map(str::to_string)))
                .collect()
        });
        report.reports = report::files::write(&report, &reports)?;
    }
    print!(
        "{}",
        report::render(&report, format, style(std::io::stdout().is_terminal()))
    );
    Ok(if report.passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn verify(parsed: &Parsed) -> Result<ExitCode, String> {
    let (only, taken) = verify_ids(&parsed.positional);
    if only.is_empty() {
        return Err("verify needs a mutant's id: mutastic verify <id> [dir]".into());
    }
    let dir = parsed.positional.get(taken).map_or(".", String::as_str);
    started(false);
    let project = workspace::read(dir)?;
    let progress = Progress::new(&project.package, false, false, parsed.progress);
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
                result.reason.as_deref().unwrap_or("skipped")
            ),
            Status::RuntimeError => format!(
                "crashes, and no test can be said to catch it ({}): it's left out of the score; check the unsafe code it reaches with Miri",
                result.reason.as_deref().unwrap_or("crash")
            ),
        };
        settled &= result.status.detected()
            || matches!(result.status, Status::Ignored | Status::RuntimeError);
        println!("mutastic: {} {fate}", mutant.id);
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
fn explain(parsed: &Parsed) -> Result<ExitCode, String> {
    let Some(id) = parsed.positional.first() else {
        return Err("explain needs a mutant's id: mutastic explain <id> [dir]".into());
    };
    let dir = parsed.positional.get(1).map_or(".", String::as_str);
    started(false);
    let project = workspace::read(dir)?;
    let progress = Progress::new(&project.package, false, false, parsed.progress);
    let mut options = options(parsed, vec![id.clone()]);
    options.suggestions = !parsed.no_suggestions;
    let report = engine::run(&project, &options, &|step| progress.step(step))?;
    progress.finished();
    let last = report::last_run(&project.mutastic_dir().join("mutation.json"), id);
    print!(
        "{}",
        report::explained(&report, last, style(std::io::stdout().is_terminal()))
    );
    Ok(ExitCode::SUCCESS)
}

/// `ignore`: records a decision not to test a mutant in `.mutastic/ignores.yaml`, with the line it's
/// on and the reason; proposes one, in `.mutastic/proposals.yaml`, for a person to accept; accepts
/// a proposal; removes one; or removes those that no longer hold. Runs no tests, and says how to
/// undo what it did, in the Java edition's words.
fn ignore(parsed: &Parsed) -> Result<ExitCode, String> {
    let Some(id) = parsed.positional.first().filter(|_| !parsed.prune).cloned() else {
        if parsed.prune {
            let dir = parsed.positional.first().map_or(".", String::as_str);
            return prune(dir);
        }
        return Err("ignore needs a mutant's id: mutastic ignore <id> [dir] --reason=WHY".into());
    };
    let dir = parsed
        .positional
        .get(1)
        .map_or(".", String::as_str)
        .to_string();
    let here = std::path::Path::new(&dir);
    let command = |words: &[&str]| {
        let words: Vec<String> = words.iter().map(|word| shell(word)).collect();
        format!("mutastic ignore {} {dir}", words.join(" "))
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
        return Err(format!(
            "ignore needs a reason, for whoever reviews it: mutastic ignore {id} --reason=\"<why>\" {dir}"
        ));
    };
    if !parsed.kind.is_empty() && !ignores::KINDS.contains(&parsed.kind.as_str()) {
        return Err(format!(
            "--as: {} isn't one of {}",
            parsed.kind,
            ignores::KINDS.join(", ")
        ));
    }
    let project = workspace::read(&dir)?;
    let found = engine::find_all(&project);
    let Some(index) = found.mutants.iter().position(|m| m.id == id) else {
        return Err(format!(
            "{id} isn't a mutant of {}: run `mutastic run {dir}` to see its mutants",
            project.package
        ));
    };
    let mutant = &found.mutants[index];
    if let Some(skipped) = &mutant.skipped {
        println!("mutastic: {id} doesn't run anyway ({skipped}): nothing to ignore");
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
        println!(
            "mutastic: {id} is proposed for ignoring: {reason}\n  recorded in {}: it counts as missed until a person accepts it\n  to accept: {}\n  to withdraw: {}",
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
    println!(
        "mutastic: {id} is ignored: {reason}\n  recorded in {}: commit it, as a decision to review\n  to undo: {}",
        ignores::FILE,
        command(&["--remove", &id])
    );
    Ok(ExitCode::SUCCESS)
}

/// A proposal accepted: it moves into the ignores, with its reason and sort as proposed.
fn accept(
    id: &str,
    here: &std::path::Path,
    command: &dyn Fn(&[&str]) -> String,
) -> Result<ExitCode, String> {
    let proposals = ignores::proposed(here)?;
    let Some(proposal) = proposals.iter().find(|entry| entry.id == id).cloned() else {
        println!(
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
    println!(
        "mutastic: {id} is ignored, as proposed: {}\n  recorded in {}: commit it, as a decision to review\n  to undo: {}",
        proposal.reason,
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
) -> Result<ExitCode, String> {
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
        println!(
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
        println!(
            "mutastic: {id} is no longer proposed\n  removed from {}\n  to undo: {}",
            ignores::PROPOSALS,
            again(proposal, true)
        );
        return Ok(ExitCode::SUCCESS);
    }
    println!(
        "mutastic: {id} isn't ignored in {}: nothing to remove",
        ignores::FILE
    );
    Ok(ExitCode::SUCCESS)
}

/// `ignore --prune`: takes away every ignore whose line has changed since it was made.
fn prune(dir: &str) -> Result<ExitCode, String> {
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
        println!("mutastic: every ignore still holds: nothing to remove");
        return Ok(ExitCode::SUCCESS);
    }
    ignores::write(here, &kept)?;
    let mut ids: Vec<&str> = stale.iter().map(|entry| entry.id.as_str()).collect();
    ids.sort();
    println!(
        "mutastic: removed {} ignore{} that no longer hold: {}\n  from {}\n  to undo: git checkout -- {}",
        stale.len(),
        if stale.len() == 1 { "" } else { "s" },
        ids.join(", "),
        ignores::FILE,
        ignores::FILE
    );
    Ok(ExitCode::SUCCESS)
}

/// A word as a shell takes it: in double quotes when it has a space or a quote.
fn shell(word: &str) -> String {
    match word.split_once('=') {
        Some((flag, value)) if value.contains([' ', '\'', '"']) => {
            format!("{flag}=\"{}\"", value.replace('"', "\\\""))
        }
        _ => word.to_string(),
    }
}

/// `gate open`, `gate check` or `gate close`, as the Java edition's: see [`gate`].
fn gate_command(parsed: &Parsed) -> Result<ExitCode, String> {
    let Some(action) = parsed.positional.first().cloned() else {
        return Err("gate needs open, check or close: mutastic gate open [dir]".into());
    };
    if parsed.help {
        println!("{}", gate_help(&action)?);
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
        "open" => gate_open(hooked, here, &work, &session),
        "close" => {
            let closed = gate::State::close(&work);
            if !hooked {
                println!(
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
        "check" => gate_check(parsed, &project, here, &work),
        other => Err(format!("gate: {other} isn't one of open, check or close")),
    }
}

fn gate_help(action: &str) -> Result<&'static str, String> {
    match action {
        "open" => Ok("mutastic gate open [dir] [--hook=claude]\n\nStart gating a coding agent's task: keep where it starts. Keeps the commit the task starts from, its ignores, and how many tests run and are ignored, in target/mutastic, so gate check can say whether the task is done. With --hook, once for each of the agent's sessions.\n\nExit status: 0 once it has kept where the task starts; 2 outside a git repository with a commit."),
        "check" => Ok("mutastic gate check [dir] [--hook=claude] [--tries=<n>]\n\nSay whether a gated task is done: its changed lines' mutants caught. Runs the mutants on the lines changed since gate open. The task is done when each is caught or proposed for a person to ignore, no ignore was made by the task, and no test was taken away or ignored; until then it says what's left, as the agent format does. With --hook it answers as the agent's hook, and lets the agent stop, saying what's left, when it changed nothing since it was sent back, or after --tries (default 3).\n\nExit status: 0 when the task is done, 1 when it isn't, 2 when no task is open. With --hook, 0 either way."),
        "close" => Ok("mutastic gate close [dir]\n\nStop gating a task: forget where it started. The next gate open starts a new task.\n\nExit status: 0 whether a task was open or not."),
        other => Err(format!("gate: {other} isn't one of open, check or close")),
    }
}

/// Keeps where the task starts; with a hook, once for each session. A hook's output is the
/// agent's to read: it's told the task is gated, and how to check its work as it goes.
fn gate_open(
    hooked: bool,
    here: &std::path::Path,
    work: &std::path::Path,
    session: &str,
) -> Result<ExitCode, String> {
    let open_for_session = !session.is_empty()
        && gate::State::read(work).is_some_and(|state| state.session == session);
    if hooked && open_for_session {
        return Ok(ExitCode::SUCCESS);
    }
    let Some(base) = crate::since::git(here, &["rev-parse", "HEAD"]) else {
        return Err("the gate needs a git repository with a commit".into());
    };
    let base = base.trim().to_string();
    let state = gate::State {
        session: session.to_string(),
        base: base.clone(),
        ignored: ignored_ids(here)?,
        shape: gate::Shape::of(here),
        ..gate::State::default()
    };
    state.write(work)?;
    println!(
        "mutastic gate: this task is gated from {}: before you finish, check the lines it changes with `mutastic run --since={base} --format=agent`; {}",
        &base[..base.len().min(7)],
        if hooked {
            "the gate checks them when you finish, so you needn't run `mutastic gate check` yourself"
        } else {
            "`mutastic gate check` says when it's done"
        }
    );
    Ok(ExitCode::SUCCESS)
}

fn ignored_ids(here: &std::path::Path) -> Result<std::collections::BTreeSet<String>, String> {
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
) -> Result<ExitCode, String> {
    let hooked = !parsed.hook.is_empty();
    let Some(open) = gate::State::read(work) else {
        if !hooked {
            println!("mutastic gate: none open here: `mutastic gate open` starts one");
        }
        // Nothing gated: a hooked agent may stop.
        return Ok(ExitCode::from(if hooked { 0 } else { 2 }));
    };
    let verdict = gate_verdict(parsed, project, here, &open)?;
    let mut state = open.clone();
    state.red.extend(verdict.red.clone());
    if verdict.passes {
        state.blocks = 0;
        state.tree.clear();
        state.write(work)?;
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
        state.write(work)?;
        return Ok(gate_answer(hooked, &verdict));
    };
    state.blocks = 0;
    state.tree.clear();
    state.write(work)?;
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
    gate::Verdict {
        text: format!(
            "{}\n\nThe task changed what tests that were there when it started expect, in {}: check that's what you asked for",
            verdict.text,
            changed.into_iter().collect::<Vec<_>>().join(", ")
        ),
        ..verdict
    }
}

/// The verdict, as the hook's answer, or printed, with the exit status.
fn gate_answer(hooked: bool, verdict: &gate::Verdict) -> ExitCode {
    if hooked {
        println!("{}", gate::claude_hook(verdict));
        return ExitCode::SUCCESS;
    }
    println!("{}", verdict.text);
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
) -> Result<gate::Verdict, String> {
    // First: taking the test away may leave no line changed.
    let gone = gate::gone(&state.red, here);
    if !gone.is_empty() {
        let one = gone.len() == 1;
        return Ok(gate::Verdict::block(format!(
            "mutastic gate: {} failed at an earlier check, and {} gone now. Put {} back: the gate doesn't pass a task by taking a failing test away. Leave a failing test in place and make the code pass it, unless the spec or plan is what's wrong, in which case say so and stop",
            gone.join(", "),
            if one { "it's" } else { "they're" },
            if one { "it" } else { "them" }
        )));
    }
    let changed = crate::since::changed_since(here, &state.base)?;
    let found = engine::find_all(project);
    let ours = found.files.iter().any(|file| {
        std::fs::canonicalize(project.workspace.join(&file.path))
            .is_ok_and(|path| changed.lines.contains_key(&path))
    });
    if !ours {
        return Ok(gate::Verdict::pass(
            "mutastic gate: passed: the task changed no lines mutastic mutates".into(),
        ));
    }
    let added: Vec<String> = ignored_ids(here)?
        .difference(&state.ignored)
        .cloned()
        .collect();
    if !added.is_empty() {
        return Ok(gate::Verdict::block(format!(
            "mutastic gate: the task ignored {} itself, and whether a mutant is worth a test is a person's call. Take it back with `mutastic ignore --remove ID`, and propose it instead: `mutastic ignore ID --propose --as=KIND --reason=\"…\"`",
            added.join(", ")
        )));
    }
    let (before, now) = (state.shape, gate::Shape::of(here));
    if now.fewer_than(&before) {
        return Ok(gate::Verdict::block(format!(
            "mutastic gate: fewer tests run than when the task started ({}, {} ignored; now {}, {} ignored). Put them back: the gate doesn't pass a change by taking tests away",
            before.tests, before.disabled, now.tests, now.disabled
        )));
    }
    let mut options = options(parsed, Vec::new());
    options.since = Some(changed);
    let progress = Progress::new(&project.package, true, false, Some(Shown::None));
    let report = match engine::run(project, &options, &|step| progress.step(step)) {
        Ok(report) => report,
        Err(error) => return red_verdict(here, state, &error),
    };
    let proposed: std::collections::BTreeSet<String> = ignores::proposed(here)?
        .into_iter()
        .map(|entry| entry.id)
        .collect();
    let missed: Vec<String> = report
        .results
        .iter()
        .filter(|r| matches!(r.status, Status::Survived | Status::NoCoverage))
        .map(|r| report.found.mutants[r.mutant].id.clone())
        .collect();
    let open: Vec<&String> = missed.iter().filter(|id| !proposed.contains(*id)).collect();
    if open.is_empty() {
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
        return Ok(gate::Verdict::pass(format!(
            "mutastic gate: passed: the tests catch {caught} of the {scored} mutants on the lines the task changed{waiting}"
        )));
    }
    let cards: String = report::render(&report, Format::Agent, Style { color: false })
        .lines()
        .skip_while(|line| line.starts_with("mutastic: "))
        .collect::<Vec<_>>()
        .join("\n");
    let one = open.len() == 1;
    Ok(gate::Verdict::block(format!(
        "mutastic gate: {} mutant{} on the lines the task changed {} missed, so it isn't done. For each card: write the test, and run its `done when` before you move on; if a case can't happen, propose ignoring it, for a person to accept.\n\n{cards}",
        open.len(),
        if one { "" } else { "s" },
        if one { "is" } else { "are" }
    )))
}

/// Tests that fail before any mutant, each time they run. One that was there when the task
/// started is most likely broken by the change, and rewriting what it expects would pass a wrong
/// change: the agent is told to find out which is wrong, and to ask. The gate remembers them, so
/// taking one away later holds the gate. The words are the Java edition's.
fn red_verdict(
    here: &std::path::Path,
    state: &gate::State,
    error: &str,
) -> Result<gate::Verdict, String> {
    let Some(failing) = crate::flaky::failing_tests(error) else {
        return Err(error.to_string());
    };
    let before: Vec<&String> = failing
        .iter()
        .filter(|test| {
            let name = test.rsplit("::").next().unwrap_or(test);
            crate::since::git(here, &["grep", "-q", "-w", name, &state.base, "--", "."]).is_some()
        })
        .collect();
    let text = if before.is_empty() {
        format!(
            "mutastic gate: {error}. Make them pass first, unless the spec or plan is what's wrong, in which case say so and stop. Either way, leave a failing test in place: the gate doesn't pass a task by taking it away"
        )
    } else {
        let one = before.len() == 1;
        let names: Vec<&str> = before.iter().map(|test| test.as_str()).collect();
        format!(
            "mutastic gate: {error}.\n\n{} {} there when the task started: if {} passed then, the task's change broke {}. Find out whether the change or the test is wrong. Don't change what a test expects, or take it away, to make it pass unless the task asked for that behaviour, and if you're not sure, ask",
            names.join(", "),
            if one { "was" } else { "were" },
            if one { "it" } else { "they" },
            if one { "it" } else { "them" }
        )
    };
    Ok(gate::Verdict {
        red: gate::remember(&failing, here),
        ..gate::Verdict::block(text)
    })
}

fn caught(result: &engine::MutantResult) -> String {
    let how = result.killed_with.as_deref().unwrap_or("panic");
    if let Some(crash) = how.strip_prefix("crash: ") {
        return format!(
            "is caught only because the test process crashed ({crash}): no test's check caught it"
        );
    }
    let test = result.killed_by.as_deref().unwrap_or("a test");
    match how.strip_prefix("panic: ") {
        Some(message) => format!("is caught by {test}, through a panic: {message}"),
        None => format!("is caught by {test}"),
    }
}

fn mutants(parsed: &Parsed) -> Result<ExitCode, String> {
    let dir = parsed.positional.first().map_or(".", String::as_str);
    let project = workspace::read(dir)?;
    let found = engine::find_with(&project, &parsed.operators, parsed.unsafe_mode);
    for mutant in &found.mutants {
        let function = &found.functions[mutant.function];
        println!(
            "{} {}:{}:{} {} {} in {}{}",
            mutant.id,
            mutant.file.display(),
            mutant.line,
            mutant.column,
            mutant.operator,
            crate::words::plain(
                &crate::words::advise(mutant, &function.name, &Default::default()).change
            ),
            function.path,
            mutant
                .near_unsafe
                .map(|near| format!(" (near unsafe: {near})"))
                .unwrap_or_default()
        );
    }
    println!("{} mutants", found.mutants.len());
    if found.not_mutated_unsafe > 0 {
        println!(
            "not mutated: unsafe ({}): --unsafe=enclosing or --unsafe=all includes them",
            found.not_mutated_unsafe
        );
    }
    for (cfg, count) in &found.not_built {
        println!("not built here: `{cfg}` ({count})");
    }
    Ok(ExitCode::SUCCESS)
}

/// `mutastic build [dir] --fraction=0.25`: writes the schema with that share of the mutants (every
/// site's mutants together, spread evenly), builds it with rollback, and prints a JSON summary.
fn build(parsed: &Parsed) -> Result<ExitCode, String> {
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
    let built = crate::compile::build(&project, &found, &live, &Default::default())?;
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
    println!(
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
