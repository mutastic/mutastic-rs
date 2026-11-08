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
}

/// Builds the copy with the `live` mutants, withdrawing those that don't compile.
pub fn build(
    project: &Project,
    found: &Crate,
    live: &BTreeSet<usize>,
    recorders: &BTreeSet<usize>,
) -> Result<Built, String> {
    let mut live = live.clone();
    let mut rounds = Vec::new();
    let mut unviable = BTreeMap::new();
    'rounds: while rounds.len() < MAX_ROUNDS {
        let (overrides, emitted) = rewrite(project, found, &live, recorders);
        workspace::sync(project, &overrides)?;
        for phase in ["check", "build"] {
            let started = Instant::now();
            let (succeeded, stdout) = cargo(project, phase)?;
            let took = started.elapsed();
            if !succeeded {
                let blame = blame(&stdout, &emitted, project);
                if blame.mutants.is_empty() {
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
) -> (Overrides, BTreeMap<PathBuf, Emitted>) {
    let mut overrides = Overrides::new();
    let mut emitted = BTreeMap::new();
    for file in &found.files {
        let mut out = schema::emit(found, &file.path, &file.text, live, recorders);
        if project.roots.contains(&file.path) {
            out.text.push_str(&schema::runtime(found.mutants.len()));
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
/// panics stay, and the rest of the debug info, which slows builds, goes.
pub fn profile_args() -> Vec<String> {
    vec![
        "--profile".into(),
        "mutastic".into(),
        "--config".into(),
        "profile.mutastic.inherits=\"test\"".into(),
        "--config".into(),
        "profile.mutastic.debug=\"line-tables-only\"".into(),
    ]
}

pub fn environment(command: &mut Command, project: &Project) {
    let wrapper = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("mutastic"));
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
    by_function: usize,
    unattributed: Vec<String>,
}

/// Works out which mutants each error belongs to.
fn blame(stdout: &str, emitted: &BTreeMap<PathBuf, Emitted>, project: &Project) -> Blame {
    let mut found = Blame {
        mutants: BTreeMap::new(),
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
        let (blamed, by_function) = blame_one(&spans, emitted, project);
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

fn blame_one(
    spans: &[Value],
    emitted: &BTreeMap<PathBuf, Emitted>,
    project: &Project,
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
    for spans in [&primary, &located.iter().collect::<Vec<_>>()] {
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
