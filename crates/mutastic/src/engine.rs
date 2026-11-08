//! A run: find the mutants, build them all at once, record what each test reaches, then run each
//! mutant against only the tests that reach it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::compile::{self, Binary, Round};
use crate::sites::{self, Crate, UnsafeMode};
use crate::testrun::{self, Outcome, Test};
use crate::workspace::Project;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Killed,
    Survived,
    Timeout,
    NoCoverage,
    Ignored,
    /// Crashed where the crash may be undefined behaviour, or where std's own debug check of an
    /// unsafe precondition stopped it: no test can be said to have caught it. Out of the score.
    RuntimeError,
}

impl Status {
    /// The mutation-testing report schema's name.
    pub fn schema_name(self) -> &'static str {
        match self {
            Status::Killed => "Killed",
            Status::Survived => "Survived",
            Status::Timeout => "Timeout",
            Status::NoCoverage => "NoCoverage",
            Status::Ignored => "Ignored",
            Status::RuntimeError => "RuntimeError",
        }
    }

    /// The events format's name for a count of this status in `run-finished`, from 0.6.0:
    /// `killed`, `noCoverage`.
    pub fn event_name(self) -> &'static str {
        match self {
            Status::Killed => "killed",
            Status::Survived => "survived",
            Status::Timeout => "timeout",
            Status::NoCoverage => "noCoverage",
            Status::Ignored => "ignored",
            Status::RuntimeError => "runtimeError",
        }
    }

    pub fn detected(self) -> bool {
        matches!(self, Status::Killed | Status::Timeout)
    }
}

#[derive(Clone, Debug)]
pub struct MutantResult {
    /// Index into `Crate::mutants`.
    pub mutant: usize,
    pub status: Status,
    /// Why it was skipped, `unviable: …`.
    pub reason: Option<String>,
    pub covered_by: Vec<String>,
    pub killed_by: Option<String>,
    pub killed_with: Option<String>,
    pub tests_run: usize,
    pub took: Duration,
    /// The whole numbers its function was called with, by parameter.
    pub inputs: BTreeMap<String, Vec<i128>>,
}

#[derive(Debug)]
pub struct Report {
    pub project: Project,
    pub found: Crate,
    pub results: Vec<MutantResult>,
    pub tests: usize,
    pub test_runs: usize,
    pub rounds: Vec<Round>,
    pub workers: usize,
    pub rustc: String,
    pub threshold: f64,
    pub setup: Duration,
    pub coverage: Duration,
    pub mutants: Duration,
    pub took: Duration,
    pub unsafe_mode: UnsafeMode,
    /// The settings the user's crate was built with, from Cargo: `opt_level`, `debug_assertions`,
    /// `overflow_checks`.
    pub profile: serde_json::Value,
}

impl Report {
    pub fn count(&self, status: Status) -> usize {
        self.results.iter().filter(|r| r.status == status).count()
    }

    /// Mutants out of the score: skipped, or crashed where a crash proves nothing.
    pub fn unscored(&self) -> usize {
        self.count(Status::Ignored) + self.count(Status::RuntimeError)
    }

    /// (caught + timed out) / (all − unscored), or `None` with nothing to score.
    pub fn score(&self) -> Option<f64> {
        let scored = self.results.len() - self.unscored();
        let detected = self.count(Status::Killed) + self.count(Status::Timeout);
        (scored > 0).then(|| 100.0 * detected as f64 / scored as f64)
    }

    /// The score of the code the tests reach.
    pub fn covered_score(&self) -> Option<f64> {
        let scored = self.results.len() - self.unscored() - self.count(Status::NoCoverage);
        let detected = self.count(Status::Killed) + self.count(Status::Timeout);
        (scored > 0).then(|| 100.0 * detected as f64 / scored as f64)
    }

    pub fn passed(&self) -> bool {
        self.score().is_none_or(|score| score >= self.threshold)
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    pub only: Vec<String>,
    pub operators: Vec<&'static str>,
    pub threshold: f64,
    pub workers: usize,
    pub unsafe_mode: UnsafeMode,
}

/// What the coverage pass found for one test.
#[derive(Clone, Debug, Default)]
struct Reach {
    took: Duration,
    hits: BTreeSet<u32>,
    /// Function ordinal → parameter → values.
    seen: BTreeMap<u32, BTreeMap<String, BTreeSet<i128>>>,
}

pub fn find(project: &Project, operators: &[&str]) -> Crate {
    find_with(project, operators, UnsafeMode::Skip)
}

pub fn find_with(project: &Project, operators: &[&str], mode: UnsafeMode) -> Crate {
    sites::find_with(
        &project.workspace,
        &project.crate_name,
        &project.roots,
        operators,
        mode,
    )
}

/// Every mutant, under every operator mutastic has, unsafe code included: what an id can name.
pub fn find_all(project: &Project) -> Crate {
    find_with(project, sites::ALL, UnsafeMode::All)
}

pub fn run(
    project: &Project,
    options: &Options,
    progress: &mut dyn FnMut(&str),
) -> Result<Report, String> {
    let started = Instant::now();
    // An id names its mutant whatever was chosen when it was shown, so `--only` and `verify`
    // look among them all.
    let found = if options.only.is_empty() {
        find_with(project, &options.operators, options.unsafe_mode)
    } else {
        find_all(project)
    };
    let selected: BTreeSet<usize> = found
        .mutants
        .iter()
        .enumerate()
        .filter(|(_, m)| options.only.is_empty() || options.only.contains(&m.id))
        .map(|(index, _)| index)
        .collect();
    if !options.only.is_empty() {
        for id in &options.only {
            if !found.mutants.iter().any(|m| &m.id == id) {
                return Err(format!(
                    "{id} isn't a mutant of {}: run `mutastic run {}` to see its mutants",
                    project.package, project.given
                ));
            }
        }
    }
    let recorders: BTreeSet<usize> = selected
        .iter()
        .map(|&m| found.mutants[m].function)
        .collect();
    progress(&format!("building {} mutants in one build", selected.len()));
    let built = compile::build(project, &found, &selected, &recorders)?;
    let cwd = project.copy_dir().join(&project.package_dir);
    let tests = testrun::list(&built.binaries, &cwd)?;
    let setup = started.elapsed();

    progress(&format!("recording what {} tests reach", tests.len()));
    let coverage_started = Instant::now();
    let reach = coverage(project, &built.binaries, &tests, options.workers)?;
    let coverage_took = coverage_started.elapsed();

    progress("running mutants");
    let mutants_started = Instant::now();
    let order: Vec<usize> = selected.iter().copied().collect();
    let first = first_at_site(&found, &selected, &built);
    let results = testrun::parallel(&order, options.workers, |&mutant| {
        result(&found, mutant, first[mutant], &built, &tests, &reach, &cwd)
    });
    let test_runs = tests.len() + results.iter().map(|r| r.tests_run).sum::<usize>();
    Ok(Report {
        project: project.clone(),
        found,
        results,
        tests: tests.len(),
        test_runs,
        rounds: built.rounds,
        workers: options.workers,
        rustc: rustc_version(),
        threshold: options.threshold,
        setup,
        coverage: coverage_took,
        mutants: mutants_started.elapsed(),
        took: started.elapsed(),
        unsafe_mode: options.unsafe_mode,
        profile: built.profile,
    })
}

/// Runs each test alone, with the switches recording which sites it reaches and the whole
/// numbers each function was called with. Every test must pass here: a test that fails alone
/// would look as if it caught every mutant it met.
fn coverage(
    project: &Project,
    binaries: &[Binary],
    tests: &[Test],
    workers: usize,
) -> Result<Vec<Reach>, String> {
    let dir = project.mutastic_dir().join("coverage");
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let cwd = project.copy_dir().join(&project.package_dir);
    let indexed: Vec<(usize, &Test)> = tests.iter().enumerate().collect();
    let reached = testrun::parallel(&indexed, workers, |(index, test)| {
        let file: PathBuf = dir.join(format!("{index}.cov"));
        let _ = std::fs::remove_file(&file);
        let outcome = testrun::run(
            &binaries[test.binary],
            &cwd,
            &[&test.name],
            &[("MUTASTIC_COVERAGE", file.display().to_string())],
            Duration::from_secs(300),
        );
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        (outcome, read_reach(&text))
    });
    let mut reach = Vec::new();
    for ((outcome, mut found), test) in reached.into_iter().zip(tests) {
        if !outcome.passed {
            return Err(format!(
                "{} fails without any mutant{}: fix it, or mark it #[ignore], before running mutastic",
                test.display,
                if outcome.timed_out { " (it timed out)" } else { "" }
            ));
        }
        found.took = outcome.took;
        reach.push(found);
    }
    Ok(reach)
}

fn read_reach(text: &str) -> Reach {
    let mut reach = Reach::default();
    for line in text.lines() {
        let mut words = line.split(' ');
        match words.next() {
            Some("h") => {
                if let Some(id) = words.next().and_then(|id| id.parse().ok()) {
                    reach.hits.insert(id);
                }
            }
            Some("s") => {
                let Some(function) = words.next().and_then(|f| f.parse().ok()) else {
                    continue;
                };
                let values = reach.seen.entry(function).or_default();
                for pair in words {
                    if let Some((name, value)) = pair.split_once('=') {
                        if let Ok(value) = value.parse() {
                            values.entry(name.to_string()).or_default().insert(value);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    reach
}

/// For each mutant, the first live mutant at its site: what the coverage pass records.
fn first_at_site(found: &Crate, selected: &BTreeSet<usize>, built: &compile::Built) -> Vec<u32> {
    let mut first: BTreeMap<(&std::path::Path, usize, usize), u32> = BTreeMap::new();
    for &index in selected {
        if built.unviable.contains_key(&index) {
            continue;
        }
        let mutant = &found.mutants[index];
        let key = (mutant.file.as_path(), mutant.site.start, mutant.site.end);
        let entry = first.entry(key).or_insert(mutant.ordinal);
        *entry = (*entry).min(mutant.ordinal);
    }
    found
        .mutants
        .iter()
        .map(|m| {
            first
                .get(&(m.file.as_path(), m.site.start, m.site.end))
                .copied()
                .unwrap_or(m.ordinal)
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn result(
    found: &Crate,
    index: usize,
    site: u32,
    built: &compile::Built,
    tests: &[Test],
    reach: &[Reach],
    cwd: &std::path::Path,
) -> MutantResult {
    let mutant = &found.mutants[index];
    let function = &found.functions[mutant.function];
    let covering: Vec<usize> = (0..tests.len())
        .filter(|&t| reach[t].hits.contains(&site))
        .collect();
    let mut inputs: BTreeMap<String, BTreeSet<i128>> = BTreeMap::new();
    for &t in &covering {
        if let Some(seen) = reach[t].seen.get(&function.ordinal) {
            for (name, values) in seen {
                inputs.entry(name.clone()).or_default().extend(values);
            }
        }
    }
    let mut result = MutantResult {
        mutant: index,
        status: Status::Survived,
        reason: None,
        covered_by: covering.iter().map(|&t| tests[t].display.clone()).collect(),
        killed_by: None,
        killed_with: None,
        tests_run: 0,
        took: Duration::ZERO,
        inputs: function
            .numbers
            .iter()
            .filter_map(|(name, _)| {
                inputs
                    .get(name)
                    .map(|values| (name.clone(), values.iter().copied().collect()))
            })
            .collect(),
    };
    result.covered_by.sort();
    if let Some(error) = built.unviable.get(&index) {
        result.status = Status::Ignored;
        result.reason = Some(format!("unviable: doesn't compile ({error})"));
        result.covered_by.clear();
        return result;
    }
    if covering.is_empty() {
        result.status = Status::NoCoverage;
        return result;
    }
    // Fastest binary first; within it, the covering tests run together.
    let mut by_binary: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &t in &covering {
        by_binary.entry(tests[t].binary).or_default().push(t);
    }
    let mut groups: Vec<(Duration, usize, Vec<usize>)> = by_binary
        .into_iter()
        .map(|(binary, group)| (group.iter().map(|&t| reach[t].took).sum(), binary, group))
        .collect();
    groups.sort();
    let started = Instant::now();
    for (baseline, binary, group) in groups {
        let mut names: Vec<&str> = group.iter().map(|&t| tests[t].name.as_str()).collect();
        // Near unsafe code, one test at a time: a mutant that corrupts memory mustn't make a
        // different test fail and take the credit.
        if mutant.near_unsafe.is_some() {
            names.push("--test-threads=1");
        }
        let timeout = (baseline * 5 + Duration::from_secs(1)).max(Duration::from_secs(10));
        let active = [("MUTASTIC_ACTIVE", mutant.ordinal.to_string())];
        let outcome: Outcome = testrun::run(&built.binaries[binary], cwd, &names, &active, timeout);
        result.tests_run += group.len();
        if outcome.timed_out {
            result.status = Status::Timeout;
            break;
        }
        if outcome.passed {
            continue;
        }
        if outcome.failed.is_none() {
            // No test failed: the process died. Run again, one test at a time and uncaptured,
            // to see how.
            names.push("--nocapture");
            if mutant.near_unsafe.is_none() {
                names.push("--test-threads=1");
            }
            let again = testrun::run(&built.binaries[binary], cwd, &names, &active, timeout);
            crashed(&mut result, mutant.near_unsafe, &outcome, &again);
            break;
        }
        result.status = Status::Killed;
        result.killed_with = Some(match testrun::language_panic(&outcome.stdout) {
            Some(message) => format!("panic: {message}"),
            None => "panic".into(),
        });
        result.killed_by = outcome.failed.as_ref().map(|name| {
            group
                .iter()
                .find(|&&t| &tests[t].name == name)
                .map(|&t| tests[t].display.clone())
                .unwrap_or_else(|| name.clone())
        });
        break;
    }
    result.took = started.elapsed();
    result
}

/// What a crash means. A safe mutant that crashes the process was caught, but by the crash, not a
/// test's check, so no test is named. Near unsafe code a crash may be undefined behaviour, and a
/// crash in std's own check of an unsafe precondition is std's catch: neither is in the score.
fn crashed(result: &mut MutantResult, near_unsafe: Option<&str>, first: &Outcome, again: &Outcome) {
    let signal = first
        .signal
        .or(again.signal)
        .map_or("an abnormal exit".to_string(), testrun::signal_name);
    let output = format!("{}\n{}", again.stdout, again.stderr);
    if output.contains("unsafe precondition(s) violated") {
        result.status = Status::RuntimeError;
        result.reason = Some(format!(
            "crash: std's debug check of an unsafe precondition stopped it ({signal}), not a test"
        ));
    } else if let Some(near) = near_unsafe {
        result.status = Status::RuntimeError;
        result.reason = Some(format!(
            "crash: {signal} {near}, which may be undefined behaviour"
        ));
    } else if output.contains("panic in a function that cannot unwind") {
        result.status = Status::Killed;
        result.killed_with = Some(format!("crash: aborted at an FFI boundary ({signal})"));
    } else {
        result.status = Status::Killed;
        result.killed_with = Some(format!("crash: {signal}"));
    }
}

pub fn rustc_version() -> String {
    std::process::Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .arg("--version")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .and_then(|text| text.split_whitespace().nth(1).map(str::to_string))
        .unwrap_or_else(|| "unknown".into())
}
