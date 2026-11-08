//! The `mutastic` command, also run as `cargo mutastic`.

use std::ffi::OsString;
use std::io::IsTerminal;
use std::process::ExitCode;

use crate::engine::{self, Options, Status};
use crate::report::{self, Format, Style};
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
  --operators=<set>     defaults or stronger, or operator ids separated by commas
  --workers=<n>         Tests run side by side (default half the cores, at most 4)

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
        _ => {}
    }
    value
        .split(',')
        .map(|name| {
            sites::STRONGER
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

fn style() -> Style {
    let forced = std::env::var_os("FORCE_COLOR").is_some();
    let plain = std::env::var_os("NO_COLOR").is_some()
        || std::env::var("TERM").is_ok_and(|term| term == "dumb");
    Style {
        color: forced || (!plain && std::io::stdout().is_terminal()),
    }
}

fn options(parsed: &Parsed, only: Vec<String>) -> Options {
    Options {
        only,
        operators: parsed.operators.clone(),
        threshold: parsed.threshold,
        workers: parsed.workers,
    }
}

fn run(parsed: &Parsed) -> Result<ExitCode, String> {
    let dir = parsed.positional.first().map_or(".", String::as_str);
    let project = workspace::read(dir)?;
    let format = format(parsed);
    let quiet = format == Format::Jsonl;
    let report = engine::run(
        &project,
        &options(parsed, parsed.only.clone()),
        &mut |step| {
            if !quiet {
                eprintln!("mutastic: {step}");
            }
        },
    )?;
    print!("{}", report::render(&report, format, style()));
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
    let project = workspace::read(dir)?;
    let only: Vec<String> = ids(named).collect();
    let report = engine::run(&project, &options(parsed, only), &mut |step| {
        eprintln!("mutastic: {step}");
    })?;
    let mut settled = true;
    for result in &report.results {
        let mutant = &report.found.mutants[result.mutant];
        let fate = match result.status {
            Status::Killed => format!(
                "is caught by {}",
                result.killed_by.as_deref().unwrap_or("a test")
            ),
            Status::Timeout => "is caught by a timeout".to_string(),
            Status::Survived => "is still missed".to_string(),
            Status::NoCoverage => "isn't reached by any test".to_string(),
            Status::Ignored => format!(
                "is skipped ({}), so it isn't run and needs no test",
                result.reason.as_deref().unwrap_or("skipped")
            ),
        };
        settled &= result.status.detected() || result.status == Status::Ignored;
        println!("mutastic: {} {fate}", mutant.id);
    }
    Ok(if settled {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn mutants(parsed: &Parsed) -> Result<ExitCode, String> {
    let dir = parsed.positional.first().map_or(".", String::as_str);
    let project = workspace::read(dir)?;
    let found = engine::find(&project, &parsed.operators);
    for mutant in &found.mutants {
        let function = &found.functions[mutant.function];
        println!(
            "{} {}:{}:{} {} {} in {}",
            mutant.id,
            mutant.file.display(),
            mutant.line,
            mutant.column,
            mutant.operator,
            crate::words::plain(
                &crate::words::advise(mutant, &function.name, &Default::default()).change
            ),
            function.path
        );
    }
    println!("{} mutants", found.mutants.len());
    Ok(ExitCode::SUCCESS)
}

/// `mutastic build [dir] --fraction=0.25`: writes the schema with that share of the mutants (every
/// site's mutants together, spread evenly), builds it with rollback, and prints a JSON summary.
fn build(parsed: &Parsed) -> Result<ExitCode, String> {
    let dir = parsed.positional.first().map_or(".", String::as_str);
    let project = workspace::read(dir)?;
    let found = engine::find(&project, &parsed.operators);
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
