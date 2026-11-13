//! The `mutastic` command, also run as `cargo mutastic`.

use std::ffi::OsString;
use std::io::IsTerminal;
use std::process::ExitCode;

use crate::edition::{self, Edition, Route};
use crate::engine::{self, Options, Status};
use crate::report::files::Kind;
use crate::report::{self, Format, Style};
use crate::shares::EVENTS;
use crate::sites::UnsafeMode;
use crate::{baseline, commands, gate, ignores, java, sites, testrun, workspace, wrapper};

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
  mutastic baseline [dir]            Record every mutant missed now in .mutastic/baseline.yaml, so
                                     later misses are told from them; --prune drops those the
                                     tests catch now
  mutastic next [dir]                The missed mutant most worth a test first, why, and the test
                                     to write; while the code under test is the same, it runs only
                                     what the last next left uncaught

Options:
  --only=<id>[,<id>…]   Run only these mutants
  --format=<format>     full (default at a terminal), brief, concise, grouped, agent or jsonl
  --threshold=<score>   The score a run must reach to pass (default 80)
  --fail-on-new         Pass when every mutant no test catches is in .mutastic/baseline.yaml,
                        and fail on one that isn't, whatever the score
  --operators=<set>     defaults, stronger or extended, or operator ids separated by commas
  --workers=<n>         Tests run side by side (default half the cores, at most 4)
  --unsafe=<mode>       skip (default): leave unsafe code and functions holding an unsafe block
                        alone; enclosing: mutate those functions too; all: unsafe code as well
  --since[=<ref>]       Run only the mutants on lines changed since the merge-base of HEAD and
                        <ref> (default: the default branch), uncommitted and new files included
  --report=<kind>[:<path>]  Write a report file: json (mutation.json) or html (mutation.html), in
                        target/mutastic unless a path is given; repeat it for more, or none.
                        A full run writes both by default
  --shard=<n>/<m>       Run only the nth of m shares of the mutants, to split one run across
                        machines; it keeps its results in target/mutastic/shares/<n>-of-<m>
  --from-shards=<dir>   Run no mutants: put together the results every share of a split run
                        kept, found anywhere in dir, as the whole run
  --no-suggestions      Don't look for a test that catches each missed mutant, which runs the
                        missed mutant's function outside the tests
  --top=<n>             How many of next's queue to show, at most one a function and two a
                        file (default 1, at most 25)
  --progress=<mode>     auto (default): a line redrawn in place at a terminal, none for agent
                        and jsonl; bar, counter (a line every so often) or none
  --vocabulary=<words>  plain (default) says caught and missed; classic says killed and survived
  --color=<when>        auto (default): colour on a terminal that wants it; always or never
  --hyperlinks=<when>   auto (default): file locations as links on a terminal; always or never
  --edition=<edition>   rust or java: which edition runs. Unless given, the build file in dir, or
                        the nearest above it, decides: Cargo.toml for rust; pom.xml, build.gradle
                        or settings.gradle for java, which is fetched the first time it's needed

Exit status: 0 when the run passes, verify's mutants are caught, or next has nothing queued; 1
when it doesn't; 2 when mutastic couldn't run.";

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
            stderr_line!("mutastic: {message}");
            return ExitCode::from(2);
        }
    };
    match dispatch(&args) {
        Ok(code) => code,
        Err(message) => {
            stderr_line!("mutastic: {message}");
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
fn parse(command: &str, args: &[String]) -> Result<Parsed, String> {
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
        shard: None,
        from_shards: None,
        top: 1,
        fail_on_new: false,
    };
    for arg in args {
        let Some(option) = arg.strip_prefix("--") else {
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
            if commands::option(name).is_none() {
                return Err(format!("unknown option --{name}\n\n{USAGE}"));
            }
            commands::accepts(declared, name, value)?;
        }
        let value = value.unwrap_or("");
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
            "shard" => parsed.shard = Some(crate::shares::Shard::parse(value)?),
            "from-shards" if value.is_empty() => {
                return Err("--from-shards needs the shares' directory: --from-shards=DIR".into())
            }
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
                parsed.vocabulary = Some(report::Vocabulary::parse(value).ok_or_else(|| {
                    format!("--vocabulary: {value} isn't one of plain or classic")
                })?)
            }
            "color" => parsed.color = choice("--color", value)?,
            "hyperlinks" => parsed.hyperlinks = choice("--hyperlinks", value)?,
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
        "run" | "verify" | "explain" | "ignore" | "gate" | "mutants" | "next" | "baseline"
        | "help" | "--help" | "-h" | "--version" | "-V" => {
            let parsed = parse(command, &args[1..])?;
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
    let synopsis = |name: &str| commands::command(name).map_or("", |command| command.synopsis);
    let (usage, takes_id, most) = match command {
        "run" | "mutants" | "next" | "baseline" => (synopsis(command), false, 1),
        "build" => ("mutastic build [dir] --fraction=<share>", false, 1),
        "verify" => (
            synopsis(command),
            true,
            1 + verify_ids(&parsed.positional).1,
        ),
        "explain" => (synopsis(command), true, 2),
        "ignore" if parsed.prune => ("mutastic ignore --prune [dir]", false, 1),
        "ignore" => (synopsis(command), true, 2),
        "gate" => ("mutastic gate open|check|close [dir]", false, 2),
        _ => return Ok(()),
    };
    if command == "run" {
        sharing(parsed)?;
        if parsed.fail_on_new && !parsed.only.is_empty() {
            return Err(
                "--fail-on-new judges a run of the package on its new misses: not with \
                        --only, whose mutants each pass when they're caught"
                    .into(),
            );
        }
    }
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

/// A run is a share or made of them, not both; made of them, it runs no mutants itself, so the
/// options for what runs are the shares'.
fn sharing(parsed: &Parsed) -> Result<(), String> {
    if parsed.from_shards.is_none() {
        return Ok(());
    }
    let given = [
        ("--shard", parsed.shard.is_some()),
        ("--only", !parsed.only.is_empty()),
        ("--since", parsed.since.is_some()),
    ];
    match given.iter().find(|(_, given)| *given) {
        Some((option, _)) => Err(format!(
            "--from-shards puts the shares' results together as the whole run, and runs no mutant \
             itself: not with {option}, which the shares run with"
        )),
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

fn operators(value: &str) -> Result<Vec<&'static str>, String> {
    let entries: Vec<String> = value.split(',').map(|e| e.trim().to_string()).collect();
    crate::config::resolve(&entries, "--operators")
}

/// Reads the package's nearest `mutastic.yaml`, if there is one: its rules, its rules files'
/// operators, and the operators it chooses, unless `--operators` chose.
fn configure(command: &str, parsed: &mut Parsed) -> Result<(), String> {
    let at = match command {
        "run" | "mutants" | "next" | "baseline" => 0,
        "ignore" if parsed.prune => 0,
        "verify" => verify_ids(&parsed.positional).1,
        _ => 1,
    };
    let given = parsed.positional.get(at).map_or(".", String::as_str);
    let config = match configured_dir(given) {
        Some(dir) => crate::config::Configuration::read(&dir)?,
        None => crate::config::Configuration::default(),
    };
    let mut rules = Vec::new();
    for plugin in &config.plugins {
        rules.extend(crate::callrules::read(plugin)?);
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

fn dispatch(args: &[String]) -> Result<ExitCode, String> {
    let Some(command) = args.first() else {
        println!("{USAGE}");
        return Ok(ExitCode::from(2));
    };
    let mut parsed = parse(command, &args[1..])?;
    arguments(command, &parsed)?;
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
    fn new(
        package: &'a str,
        quiet: bool,
        files: bool,
        asked: Option<Shown>,
        parsed: &Parsed,
    ) -> Progress<'a> {
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
            style: style(parsed, terminal),
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
                stderr_line!("building  {mutants} mutants in one build…")
            }
            engine::Progress::Covered {
                tests,
                rustc,
                took,
                mutants,
                flaky,
            } => {
                counter.total = mutants;
                stderr_line!(
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
            engine::Progress::FromShares { summary, mutants } => {
                counter.total = mutants;
                stderr_line!("{summary}");
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
                stderr_text!(
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
            Shown::Bar => stderr_text!("{CLEAR}{}", self.line(counter)),
            // With each file's line, a log already shows how far the run is.
            Shown::Counter if !self.files => {
                let due = counter.printed.is_none_or(|at| at.elapsed() >= EVERY);
                if due || counter.done == counter.total {
                    stderr_line!("{}", self.line(counter));
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
            stderr_text!("{CLEAR}");
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
        stderr_line!(
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
        shard: None,
    }
}

fn run(parsed: &Parsed) -> Result<ExitCode, String> {
    no_baseline_to_fail_on(parsed)?;
    if let Some(shares) = &parsed.from_shards {
        return from_shards(parsed, shares);
    }
    let dir = parsed.positional.first().map_or(".", String::as_str);
    let format = format(parsed);
    let quiet = matches!(format, Format::Agent | Format::Jsonl);
    started(quiet);
    let project = workspace::read(dir)?;
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
    let progress = Progress::new(&project.package, quiet, true, parsed.progress, parsed);
    let mut options = options(parsed, parsed.only.clone());
    options.suggestions = !parsed.no_suggestions;
    options.shard = parsed.shard;
    if let Some(since) = &parsed.since {
        options.since = Some(crate::since::changed_since(
            std::path::Path::new(dir),
            since,
        )?);
    }
    let mut report = engine::run(&project, &options, &|step| progress.step(step))?;
    report.share = share;
    progress.finished();
    finish(parsed, report, format, folder.as_deref())
}

/// `--fail-on-new` with no baseline to tell new misses from: every miss would be new, or none, and
/// either would be a guess. Said before the package is read.
fn no_baseline_to_fail_on(parsed: &Parsed) -> Result<(), String> {
    let dir = parsed.positional.first().map_or(".", String::as_str);
    if !parsed.fail_on_new || baseline::exists(std::path::Path::new(dir)) {
        return Ok(());
    }
    let shown = std::fs::canonicalize(dir)
        .map_or_else(|_| dir.to_string(), |path| path.display().to_string());
    Err(format!(
        "--fail-on-new needs a baseline to tell new misses from, and {shown} has no {}: \
         `mutastic baseline` writes one, to commit",
        baseline::FILE
    ))
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
fn from_shards(parsed: &Parsed, shares: &str) -> Result<ExitCode, String> {
    let shares =
        crate::shares::Shares::read(std::path::Path::new(shares), env!("CARGO_PKG_VERSION"))?;
    let dir = parsed.positional.first().map_or(".", String::as_str);
    let format = format(parsed);
    let quiet = matches!(format, Format::Agent | Format::Jsonl);
    started(quiet);
    let project = workspace::read(dir)?;
    let progress = Progress::new(&project.package, quiet, true, parsed.progress, parsed);
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
) -> Result<ExitCode, String> {
    let folder = folder.map_or_else(
        || report.project.mutastic_dir(),
        std::path::Path::to_path_buf,
    );
    // Only a run of every mutant compares with the last one and is kept for the next: a run of one
    // says nothing about the others, and a change's score says nothing about the whole package's.
    if parsed.only.is_empty() && report.scope.is_none() {
        report.comparison = recorded(&report, &folder)?;
    }
    // Only a full run writes reports: a run of some mutants would leave out the rest.
    if parsed.only.is_empty() {
        let reports = parsed.reports.clone().unwrap_or_else(|| {
            report::files::DEFAULT
                .iter()
                .map(|(kind, path)| (*kind, path.map(str::to_string)))
                .collect()
        });
        report.reports = report::files::write(&report, &reports, &folder)?;
    }
    // A share's events, kept for `--from-shards`: as `--format=jsonl` writes them, whatever the
    // format shown.
    if report.share.is_some() {
        let events = folder.join(EVENTS);
        let given = std::path::Path::new(&report.project.given);
        // Written empty first, so its path is shown as the reports' are.
        crate::shares::keep(&events, "")?;
        report.reports.push(report::files::shown(given, &events));
        crate::shares::keep(
            &events,
            &report::render(&report, Format::Jsonl, Style::default()),
        )?;
    }
    report.fail_on_new = judged_on_new(parsed);
    print!(
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
        println!(
            "note: {caught} of the baseline's mutants are caught now: `mutastic baseline --prune` drops them"
        );
    }
}

/// `baseline`: every mutant no test catches, written for the package to commit; or with `--prune`,
/// the baseline's mutants the tests catch now dropped. Never rewritten whole once there, as taking
/// in new misses would hide them.
fn baseline_command(parsed: &Parsed) -> Result<ExitCode, String> {
    let dir = parsed.positional.first().map_or(".", String::as_str);
    let here = std::path::Path::new(dir);
    if parsed.prune != baseline::exists(here) {
        if parsed.prune {
            println!("mutastic: there's no baseline to prune: `mutastic baseline` writes one");
        } else {
            println!(
                "mutastic: there's a baseline already, in {}. `mutastic baseline --prune` drops its \
                 mutants the tests catch now; it's never rewritten to take in new misses, which \
                 would hide them",
                baseline::FILE
            );
        }
        return Ok(ExitCode::FAILURE);
    }
    let quiet = matches!(format(parsed), Format::Agent | Format::Jsonl);
    started(quiet);
    let project = workspace::read(dir)?;
    let progress = Progress::new(&project.package, quiet, false, parsed.progress, parsed);
    let report = engine::run(&project, &options(parsed, Vec::new()), &|step| {
        progress.step(step)
    })?;
    progress.finished();
    if parsed.prune {
        let caught = baseline::caught_now(&report, &baseline::ids(here));
        let dropped = baseline::prune(here, &caught)?;
        println!(
            "mutastic: {dropped} of the baseline's mutants are caught now, and are dropped from {}",
            baseline::FILE
        );
    } else {
        let entries = baseline::entries(&report);
        baseline::write(here, &entries)?;
        println!(
            "mutastic: {} mutants no test catches are in {}: commit it. A miss that isn't in it is \
             new: the gate holds only new misses, and next offers them first",
            entries.len(),
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
fn next(parsed: &Parsed) -> Result<ExitCode, String> {
    let dir = parsed.positional.first().map_or(".", String::as_str);
    let format = format(parsed);
    let quiet = matches!(format, Format::Agent | Format::Jsonl);
    started(quiet);
    let project = workspace::read(dir)?;
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
            Some(next_pass(parsed, &project, only)?)
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
        crate::nextrun::NextRun { key, uncaught }.write(&folder)?;
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
    print!("{text}");
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
) -> Result<engine::Report, String> {
    let format = format(parsed);
    let quiet = matches!(format, Format::Agent | Format::Jsonl);
    let progress = Progress::new(&project.package, quiet, false, parsed.progress, parsed);
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
    if whole && report.scope.is_none() {
        let folder = project.mutastic_dir();
        report.comparison = recorded(&report, &folder)?;
        let reports: Vec<(Kind, Option<String>)> = report::files::DEFAULT
            .iter()
            .map(|(kind, path)| (*kind, path.map(str::to_string)))
            .collect();
        report.reports = report::files::write(&report, &reports, &folder)?;
    }
    Ok(report)
}

/// Keeps `report` as the last run in `folder`, with its comparison with the one before: only an
/// earlier run of the same package, in the same place, under the same operators, says anything
/// about it. A share's folder is its own, so a whole run is never compared with part of one.
fn recorded(
    report: &engine::Report,
    folder: &std::path::Path,
) -> Result<Option<crate::record::Comparison>, String> {
    let code = crate::shares::code(&report.project)?;
    let record = crate::record::RunRecord::of(report, &code, &crate::record::tool());
    let records = crate::records::RunRecords::new(folder);
    let comparison = records
        .comparable_to(&record)
        .map(|previous| record.against(&previous));
    records.keep(&record).map_err(|error| {
        format!(
            "couldn't keep the run's record in {}: {error}",
            folder.display()
        )
    })?;
    Ok(comparison)
}

fn verify(parsed: &Parsed) -> Result<ExitCode, String> {
    let (only, taken) = verify_ids(&parsed.positional);
    if only.is_empty() {
        return Err("verify needs a mutant's id: mutastic verify <id> [dir]".into());
    }
    let dir = parsed.positional.get(taken).map_or(".", String::as_str);
    started(false);
    let project = workspace::read(dir)?;
    let progress = Progress::new(&project.package, false, false, parsed.progress, parsed);
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
    if let Some(other) = parsed
        .format
        .filter(|format| !matches!(format, Format::Full | Format::Jsonl))
    {
        return Err(format!(
            "explain takes --format=full or --format=jsonl, not --format={}",
            other.name()
        ));
    }
    let dir = parsed.positional.get(1).map_or(".", String::as_str);
    started(false);
    let project = workspace::read(dir)?;
    let progress = Progress::new(&project.package, false, false, parsed.progress, parsed);
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
    print!("{shown}");
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
    if parsed.propose {
        argued(&reason, &parsed.kind)?;
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

/// A proposal's reason, which a person checks in place of a test: filled in, and for a mutant said
/// to be equivalent or unreachable, saying why its case can't happen, as the Java edition asks.
fn argued(reason: &str, kind: &str) -> Result<(), String> {
    if reason.contains("<case>") || reason.contains("<why>") {
        return Err(
            "the reason still has the template's <case> or <why>: fill them in, as \
             --reason=\"Differs only when count is negative. That can't happen because count is a \
             length.\""
                .into(),
        );
    }
    let because = reason
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word.eq_ignore_ascii_case("because"));
    if ["equivalent", "unreachable"].contains(&kind) && !because {
        return Err(format!(
            "a proposal that a mutant is {kind} needs a reason a person can check: the one case \
             where it differs, and why that can't happen, with a because: --reason=\"{}\"",
            crate::report::REASON
        ));
    }
    Ok(())
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

fn gate_help(action: &str) -> Result<String, String> {
    commands::command(&format!("gate {action}"))
        .map(commands::help)
        .ok_or_else(|| format!("gate: {action} isn't one of open, check or close"))
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
    let older = baseline::ids(here);
    let baselined: Vec<String> = older
        .difference(&baseline::ids_at(here, &state.base))
        .cloned()
        .collect();
    if !baselined.is_empty() {
        let one = baselined.len() == 1;
        return Ok(gate::Verdict::block(format!(
            "mutastic gate: the task added {} to {} itself, and whether a miss is the project's backlog is a person's call. Take {} out, and write the test, or propose ignoring it: `mutastic ignore ID --propose --as=KIND --reason=\"…\"`",
            baselined.join(", "),
            baseline::FILE,
            if one { "it" } else { "them" }
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
    let progress = Progress::new(&project.package, true, false, Some(Shown::None), parsed);
    let mut report = match engine::run(project, &options, &|step| progress.step(step)) {
        Ok(report) => report,
        Err(error) => return red_verdict(here, state, &error),
    };
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
        return Ok(gate::Verdict::pass(format!(
            "mutastic gate: passed: the tests catch {caught} of the {scored} mutants on the lines the task changed{waiting}{backlog}"
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
        let refused = argued("never", "unreachable").unwrap_err();
        assert!(
            refused.starts_with("a proposal that a mutant is unreachable needs a reason"),
            "{refused}"
        );
        assert!(refused.ends_with(&format!("--reason=\"{}\"", report::REASON)));
        assert!(argued(report::REASON, "telemetry")
            .unwrap_err()
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
        assert!(check(&words("run --thresold=70"))
            .unwrap_err()
            .starts_with("unknown option --thresold"));
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

    /// The help lists every option declared, and only those.
    #[test]
    fn the_usage_lists_the_options_declared() {
        let listed: std::collections::BTreeSet<&str> = USAGE
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
            .filter_map(|word| word.strip_prefix("--"))
            .filter(|name| *name != "help")
            .collect();
        let declared: std::collections::BTreeSet<&str> =
            commands::OPTIONS.iter().map(|option| option.name).collect();
        assert_eq!(listed, declared);
    }
}
