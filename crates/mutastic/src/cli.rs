//! The `mutastic` command, also run as `cargo mutastic`.

use std::ffi::OsString;
use std::io::IsTerminal;
use std::process::ExitCode;

use crate::engine::{self, Options, Status};
use crate::report::files::Kind;
use crate::report::{self, Format, Style};
use crate::sites::UnsafeMode;
use crate::{ignores, sites, testrun, workspace, wrapper};

const USAGE: &str = "\
mutastic: find the tests that run your code but don't check it

Usage:
  mutastic run [dir] [options]       Mutate the package in dir (default .) and run its tests
  mutastic verify <id>[,<id>…] [dir] Pass when a test catches each mutant named
  mutastic explain <id> [dir]        Everything known of one mutant, with its tests run
  mutastic ignore <id> [dir] --reason=<why>
                                     Record a decision not to test one mutant, in
                                     .mutastic/ignores.yaml; --as=<kind> says what sort; --propose
                                     proposes it for a person to accept (--accept <id>);
                                     --remove <id> takes one back; --prune those whose line changed
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
  --progress=<mode>     auto (default): a line redrawn in place at a terminal, none for agent
                        and jsonl; bar, counter (a line every so often) or none

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
    match dispatch(&args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("mutastic: {message}");
            ExitCode::from(2)
        }
    }
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
        "run" | "verify" | "explain" | "ignore" | "mutants" | "help" | "--help" | "-h"
        | "--version" | "-V" => {
            let parsed = parse(&args[1..])?;
            match &parsed.operators_given {
                Some(given) => operators(given).map(|_| ()),
                None => Ok(()),
            }
        }
        other => Err(format!("unknown command {other}")),
    }
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
        "mutants" => mutants(&parsed),
        // For measuring the schema itself: build with a share of the mutants, and say how it went.
        "build" => build(&parsed),
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        "--version" | "-V" => {
            println!("mutastic {} (Rust edition)", env!("CARGO_PKG_VERSION"));
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
            } => {
                counter.total = mutants;
                eprintln!(
                    "coverage  {} on rustc {rustc} in {} · {}",
                    count(tests, "test"),
                    report::duration(took),
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
    let Some(named) = parsed.positional.first() else {
        return Err("verify needs a mutant's id: mutastic verify <id> [dir]".into());
    };
    let dir = parsed.positional.get(1).map_or(".", String::as_str);
    started(false);
    let project = workspace::read(dir)?;
    let only: Vec<String> = ids(named).collect();
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
    let report = engine::run(&project, &options(parsed, vec![id.clone()]), &|step| {
        progress.step(step)
    })?;
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
