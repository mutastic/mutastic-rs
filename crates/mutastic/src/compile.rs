//! One build for every mutant, and the rollback of those that don't compile.
//!
//! Each round writes the schema into the copy, runs `cargo check --tests` and then `cargo build
//! --tests`, and blames each compile error on the mutants it belongs to: first a mutated arm the
//! error lies in (that text exists only because of its mutant), then the innermost guard around
//! it, then the guards inside it (a borrow error spans the construct), and last every mutant in
//! the function it's in. Those mutants are withdrawn as unviable and the round runs again.
//!
//! Flags reach the user's crate only through `RUSTC_WORKSPACE_WRAPPER` (this binary), never
//! `RUSTFLAGS`, which would replace the project's own flags and rebuild every dependency.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::schema::{self, Emitted};
use crate::sites::Crate;
use crate::workspace::{self, Overrides, Project};

/// The rounds a build may take before giving up. Generous: a round is a warm rebuild, and
/// stopping early loses the whole run.
pub const MAX_ROUNDS: usize = 100;

#[derive(Clone, Debug)]
pub struct Round {
    /// `check` or `build`.
    pub phase: &'static str,
    pub took: Duration,
    /// Mutants this round withdrew.
    pub withdrawn: usize,
    /// Errors blamed only on their whole function.
    pub by_function: usize,
}

#[derive(Clone, Debug)]
pub struct Binary {
    /// The target's name: the crate's for its library, the file's for an integration test.
    pub target: String,
    /// `lib`, `bin` or `test`.
    pub kind: String,
    pub path: PathBuf,
}

#[derive(Debug)]
pub struct Built {
    pub binaries: Vec<Binary>,
    pub rounds: Vec<Round>,
    /// Mutants that didn't compile, with the error that said so.
    pub unviable: BTreeMap<usize, String>,
    /// The profile Cargo built the package with, from its own report: `opt_level`,
    /// `debug_assertions`, `overflow_checks`.
    pub profile: Value,
    /// The functions whose probe was built: a probe that didn't compile is left out, not blamed
    /// on a mutant.
    pub probes: BTreeSet<usize>,
}

/// Builds the copy with the `live` mutants, withdrawing those that don't compile, and a recorder
/// and a probe for each function in `recorders` (a probe only where a test could call it).
pub fn build(
    project: &Project,
    found: &Crate,
    live: &BTreeSet<usize>,
    recorders: &BTreeSet<usize>,
) -> Result<Built, String> {
    let mut live = live.clone();
    let mut probes: BTreeSet<usize> = recorders
        .iter()
        .copied()
        .filter(|&function| found.functions[function].callable.is_some())
        .collect();
    let mut rounds = Vec::new();
    let mut unviable = BTreeMap::new();
    'rounds: while rounds.len() < MAX_ROUNDS {
        let (overrides, emitted) = rewrite(project, found, &live, recorders, &probes);
        workspace::sync(project, &overrides)?;
        for phase in ["check", "build"] {
            let started = Instant::now();
            let (succeeded, stdout) = cargo(project, phase)?;
            let took = started.elapsed();
            if !succeeded {
                let blame = blame(&stdout, &emitted, project);
                if blame.mutants.is_empty() && blame.probes.is_empty() {
                    return Err(format!(
                        "the build fails, and not because of a mutant:\n{}",
                        blame.unattributed.join("\n")
                    ));
                }
                rounds.push(Round {
                    phase,
                    took,
                    withdrawn: blame.mutants.len(),
                    by_function: blame.by_function,
                });
                for (mutant, error) in blame.mutants {
                    live.remove(&mutant);
                    unviable.entry(mutant).or_insert(error);
                }
                for function in &blame.probes {
                    probes.remove(function);
                }
                continue 'rounds;
            }
            rounds.push(Round {
                phase,
                took,
                withdrawn: 0,
                by_function: 0,
            });
            if phase == "build" {
                return Ok(Built {
                    binaries: binaries(&stdout, project),
                    rounds,
                    unviable,
                    profile: profile(&stdout, project),
                    probes,
                });
            }
        }
    }
    Err(format!(
        "the build still fails after {MAX_ROUNDS} rounds of withdrawing mutants"
    ))
}

/// The rewritten files: each with live mutants or a recorder, and every crate root, which holds
/// the switches.
pub fn rewrite(
    project: &Project,
    found: &Crate,
    live: &BTreeSet<usize>,
    recorders: &BTreeSet<usize>,
    probes: &BTreeSet<usize>,
) -> (Overrides, BTreeMap<PathBuf, Emitted>) {
    let mut overrides = Overrides::new();
    let mut emitted = BTreeMap::new();
    for file in &found.files {
        let mut out = schema::emit(found, &file.path, &file.text, live, recorders, probes);
        if project.roots.contains(&file.path) {
            out.text
                .push_str(&schema::runtime(found.mutants.len(), &project.edition));
        }
        if out.text != file.text {
            overrides.insert(file.path.clone(), out.text.clone());
        }
        emitted.insert(file.path.clone(), out);
    }
    (overrides, emitted)
}

/// `cargo check` or `cargo test --no-run` in the copy, with JSON messages on stdout.
fn cargo(project: &Project, phase: &str) -> Result<(bool, String), String> {
    let mut command = Command::new(workspace::cargo());
    match phase {
        "check" => command.arg("check"),
        // `cargo build --tests` builds what `cargo test --no-run` does, and takes `--keep-going`.
        _ => command.arg("build"),
    };
    command
        .args(["--tests", "--keep-going", "--message-format=json", "-p"])
        .arg(&project.package)
        .args(profile_args())
        .current_dir(project.copy_dir());
    environment(&mut command, project);
    let output = command
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("couldn't run cargo: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.status.success() && !stdout.contains("\"compiler-message\"") {
        return Err(format!(
            "cargo failed in the copy:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok((output.status.success(), stdout))
}

/// A profile of mutastic's own, from the `test` profile with line tables only: line numbers in
/// panics stay, and the rest of the debug info, which slows builds, goes. Overflow checks and debug
/// assertions are on whatever the project's own profiles say, because they decide what a mutant
/// does: with them, `a - b` going below zero panics, and std checks unsafe preconditions; without,
/// it wraps silently, and scores from two projects' settings couldn't be compared.
pub fn profile_args() -> Vec<String> {
    [
        "inherits=\"test\"",
        "debug=\"line-tables-only\"",
        "overflow-checks=true",
        "debug-assertions=true",
    ]
    .iter()
    .flat_map(|setting| {
        [
            "--config".to_string(),
            format!("profile.mutastic.{setting}"),
        ]
    })
    .chain(["--profile".to_string(), "mutastic".to_string()])
    .collect()
}

pub fn environment(command: &mut Command, project: &Project) {
    // The wrapper is this binary; a library caller (a test) names the binary with MUTASTIC_EXE.
    let wrapper = std::env::var_os("MUTASTIC_EXE")
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
        .unwrap_or_else(|| PathBuf::from("mutastic"));
    command
        .env("CARGO_TARGET_DIR", project.build_dir())
        .env("RUSTC_WORKSPACE_WRAPPER", wrapper)
        .env(crate::wrapper::MARKER, "1")
        .env(
            "CARGO_INCREMENTAL",
            std::env::var("MUTASTIC_INCREMENTAL").unwrap_or_else(|_| "1".into()),
        )
        .env("CARGO_TERM_COLOR", "never")
        .env_remove("MUTASTIC_ACTIVE")
        .env_remove("MUTASTIC_COVERAGE");
}

struct Blame {
    mutants: BTreeMap<usize, String>,
    /// Functions whose probe an error lies in.
    probes: BTreeSet<usize>,
    by_function: usize,
    unattributed: Vec<String>,
}

/// Works out which mutants each error belongs to.
fn blame(stdout: &str, emitted: &BTreeMap<PathBuf, Emitted>, project: &Project) -> Blame {
    let mut found = Blame {
        mutants: BTreeMap::new(),
        probes: BTreeSet::new(),
        by_function: 0,
        unattributed: Vec::new(),
    };
    for line in stdout.lines() {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if message["reason"] != "compiler-message" || message["message"]["level"] != "error" {
            continue;
        }
        let diagnostic = &message["message"];
        let text = diagnostic["rendered"]
            .as_str()
            .or(diagnostic["message"].as_str())
            .unwrap_or_default()
            .to_string();
        let code = diagnostic["code"]["code"].as_str().unwrap_or("error");
        let summary = format!(
            "{code}: {}",
            diagnostic["message"].as_str().unwrap_or_default()
        );
        let spans = diagnostic["spans"].as_array().cloned().unwrap_or_default();
        if spans.is_empty() {
            continue;
        }
        let in_probes = in_probes(&spans, emitted, project);
        if !in_probes.is_empty() {
            found.probes.extend(in_probes);
            continue;
        }
        let (blamed, by_function) = blame_one(&spans, emitted, project, code);
        if blamed.is_empty() {
            found.unattributed.push(text);
        } else {
            if by_function {
                found.by_function += 1;
            }
            for mutant in blamed {
                found
                    .mutants
                    .entry(mutant)
                    .or_insert_with(|| summary.clone());
            }
        }
    }
    found
}

/// The functions whose probe any of an error's spans lies in.
fn in_probes(
    spans: &[Value],
    emitted: &BTreeMap<PathBuf, Emitted>,
    project: &Project,
) -> BTreeSet<usize> {
    spans
        .iter()
        .filter_map(|span| {
            let path = Path::new(span["file_name"].as_str()?);
            let out = emitted.get(path.strip_prefix(project.copy_dir()).unwrap_or(path))?;
            let start = span["byte_start"].as_u64()? as usize;
            let end = span["byte_end"].as_u64()? as usize;
            Some(
                out.probes
                    .iter()
                    .filter(move |(probe, _)| probe.start < end.max(start + 1) && start < probe.end)
                    .map(|(_, function)| *function),
            )
        })
        .flatten()
        .collect()
}

/// Errors whose cause is a span other than the primary one: a use after a move is reported at the
/// use, but the mutant that moved the value is at the "moved here" label.
const CAUSED_ELSEWHERE: &[&str] = &[
    "E0382", "E0499", "E0502", "E0503", "E0505", "E0506", "E0597", "E0716",
];

fn blame_one(
    spans: &[Value],
    emitted: &BTreeMap<PathBuf, Emitted>,
    project: &Project,
    code: &str,
) -> (BTreeSet<usize>, bool) {
    let located: Vec<(&Emitted, usize, usize, bool)> = spans
        .iter()
        .filter_map(|span| {
            let file = span["file_name"].as_str()?;
            let path = Path::new(file);
            let path = path.strip_prefix(project.copy_dir()).unwrap_or(path);
            let out = emitted.get(path)?;
            Some((
                out,
                span["byte_start"].as_u64()? as usize,
                span["byte_end"].as_u64()? as usize,
                span["is_primary"].as_bool().unwrap_or(false),
            ))
        })
        .collect();
    let primary: Vec<_> = located
        .iter()
        .filter(|(_, _, _, primary)| *primary)
        .collect();
    let overlaps = |arm: &std::ops::Range<usize>, start: usize, end: usize| {
        arm.start < end.max(start + 1) && start < arm.end
    };
    // An arm the error lies in, primary spans first, then any.
    let secondary: Vec<_> = located
        .iter()
        .filter(|(_, _, _, primary)| !*primary)
        .collect();
    let all: Vec<_> = located.iter().collect();
    let order = if CAUSED_ELSEWHERE.contains(&code) {
        [&secondary, &all]
    } else {
        [&primary, &all]
    };
    for spans in order {
        let exact: BTreeSet<usize> = spans
            .iter()
            .flat_map(|(out, start, end, _)| {
                out.arms
                    .iter()
                    .filter(|(arm, _)| overlaps(arm, *start, *end))
                    .map(|(_, mutant)| *mutant)
            })
            .collect();
        if !exact.is_empty() {
            return (exact, false);
        }
    }
    for (out, start, end, _) in &primary {
        // The innermost guard around the error.
        if let Some((_, mutants)) = out
            .guards
            .iter()
            .filter(|(guard, _)| guard.start <= *start && *end <= guard.end)
            .min_by_key(|(guard, _)| guard.len())
        {
            return (mutants.iter().copied().collect(), false);
        }
        // The guards inside it.
        let inside: BTreeSet<usize> = out
            .guards
            .iter()
            .filter(|(guard, _)| *start <= guard.start && guard.end <= *end)
            .flat_map(|(_, mutants)| mutants.iter().copied())
            .collect();
        if !inside.is_empty() {
            return (inside, false);
        }
        // Every mutant in the function.
        if let Some((function, _)) = out
            .functions
            .iter()
            .filter(|(function, _)| function.start <= *start && *end <= function.end)
            .min_by_key(|(function, _)| function.len())
        {
            let in_function: BTreeSet<usize> = out
                .guards
                .iter()
                .filter(|(guard, _)| function.start <= guard.start && guard.end <= function.end)
                .flat_map(|(_, mutants)| mutants.iter().copied())
                .collect();
            if !in_function.is_empty() {
                return (in_function, true);
            }
        }
    }
    (BTreeSet::new(), false)
}

/// The profile Cargo reports for the package's own artifacts.
fn profile(stdout: &str, project: &Project) -> Value {
    let manifest = project
        .copy_dir()
        .join(&project.package_dir)
        .join("Cargo.toml");
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|message| message["reason"] == "compiler-artifact")
        .find(|message| {
            message["manifest_path"]
                .as_str()
                .is_some_and(|path| Path::new(path) == manifest)
        })
        .map(|message| message["profile"].clone())
        .unwrap_or(Value::Null)
}

/// The test binaries of the package under test, from the build's messages.
fn binaries(stdout: &str, project: &Project) -> Vec<Binary> {
    let manifest = project
        .copy_dir()
        .join(&project.package_dir)
        .join("Cargo.toml");
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|message| message["reason"] == "compiler-artifact")
        .filter(|message| {
            message["manifest_path"]
                .as_str()
                .is_some_and(|path| Path::new(path) == manifest)
        })
        .filter_map(|message| {
            Some(Binary {
                target: message["target"]["name"].as_str()?.to_string(),
                kind: message["target"]["kind"][0].as_str()?.to_string(),
                path: PathBuf::from(message["executable"].as_str()?),
            })
        })
        .collect()
}
