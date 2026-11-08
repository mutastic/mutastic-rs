//! The `mutastic` command, also run as `cargo mutastic`.

use std::ffi::OsString;
use std::io::IsTerminal;
use std::process::ExitCode;

use crate::engine::{self, Options, Status};
use crate::report::{self, Format, Style};
use crate::sites::UnsafeMode;
use crate::{sites, testrun, workspace, wrapper};

const USAGE: &str = "\
mutastic: find the tests that run your code but don't check it

Usage:
  mutastic run [dir] [options]       Mutate the package in dir (default .) and run its tests
  mutastic verify <id>[,<id>…] [dir] Pass when a test catches each mutant named
  mutastic mutants [dir]             List the mutants, without running anything

Options:
  --only=<id>[,<id>…]   Run only these mutants
  --format=<format>     full (default at a terminal), concise, agent or jsonl
  --threshold=<score>   The score a run must reach to pass (default 80)
  --operators=<set>     defaults, stronger or extended, or operator ids separated by commas
  --workers=<n>         Tests run side by side (default half the cores, at most 4)
  --unsafe=<mode>       skip (default): leave unsafe code and functions holding an unsafe block
                        alone; enclosing: mutate those functions too; all: unsafe code as well

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
            "operators" => parsed.operators = operators(value)?,
            "unsafe" => {
                parsed.unsafe_mode = UnsafeMode::parse(value).ok_or_else(|| {
                    format!("--unsafe: {value} isn't one of skip, enclosing or all")
                })?
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
        "run" | "verify" | "mutants" | "help" | "--help" | "-h" | "--version" | "-V" => {
            parse(&args[1..]).map(|_| ())
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
    match value {
        "defaults" => return Ok(sites::DEFAULTS.to_vec()),
        "stronger" => return Ok(sites::STRONGER.to_vec()),
        "extended" => return Ok(sites::EXTENDED.to_vec()),
        _ => {}
    }
    value
        .split(',')
        .map(|name| {
            sites::ALL
                .iter()
                .copied()
                .find(|known| *known == name)
                .ok_or_else(|| format!("--operators: {name} isn't an operator mutastic has"))
        })
        .collect()
}

fn dispatch(args: &[String]) -> Result<ExitCode, String> {
    let Some(command) = args.first() else {
        println!("{USAGE}");
        return Ok(ExitCode::from(2));
    };
    let parsed = parse(&args[1..])?;
    match command.as_str() {
        "run" => run(&parsed),
        "verify" => verify(&parsed),
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

/// What the run is doing, on stderr so stdout stays the result: what it's reading, the build, the
/// coverage pass, and with `files` each source file's line as its last mutant finishes. Nothing
/// when `quiet`, for agents and tools, who only want the result.
fn progress(package: &str, quiet: bool, files: bool) -> impl Fn(engine::Progress) + Sync + '_ {
    let style = style(std::io::stderr().is_terminal());
    move |step| {
        if quiet {
            return;
        }
        match step {
            engine::Progress::Building { mutants } => {
                eprintln!("building  {mutants} mutants in one build…")
            }
            engine::Progress::Covered {
                tests,
                rustc,
                took,
                mutants,
            } => eprintln!(
                "coverage  {} on rustc {rustc} in {} · {}",
                count(tests, "test"),
                report::duration(took),
                count(mutants, "mutant")
            ),
            engine::Progress::FileFinished { found, results } if files => {
                eprint!("{}", report::file_finished(package, found, results, style))
            }
            engine::Progress::FileFinished { .. } => {}
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
    }
}

fn run(parsed: &Parsed) -> Result<ExitCode, String> {
    let dir = parsed.positional.first().map_or(".", String::as_str);
    let format = format(parsed);
    let quiet = matches!(format, Format::Agent | Format::Jsonl);
    started(quiet);
    let project = workspace::read(dir)?;
    let report = engine::run(
        &project,
        &options(parsed, parsed.only.clone()),
        &progress(&project.package, quiet, true),
    )?;
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
    let report = engine::run(
        &project,
        &options(parsed, only),
        &progress(&project.package, false, false),
    )?;
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
