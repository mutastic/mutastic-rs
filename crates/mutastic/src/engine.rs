//! A run: find the mutants, build them all at once, record what each test reaches, then run each
//! mutant against only the tests that reach it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::compile::{self, Binary, Round};
use crate::sites::{self, Crate};
use crate::testrun::{self, Outcome, Test};
use crate::workspace::Project;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Killed,
    Survived,
    Timeout,
    NoCoverage,
    Ignored,
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
}

impl Report {
    pub fn count(&self, status: Status) -> usize {
        self.results.iter().filter(|r| r.status == status).count()
    }

    /// (caught + timed out) / (all − skipped), or `None` with nothing to score.
    pub fn score(&self) -> Option<f64> {
        let scored = self.results.len() - self.count(Status::Ignored);
        let detected = self.count(Status::Killed) + self.count(Status::Timeout);
        (scored > 0).then(|| 100.0 * detected as f64 / scored as f64)
    }

    /// The score of the code the tests reach.
    pub fn covered_score(&self) -> Option<f64> {
        let scored =
            self.results.len() - self.count(Status::Ignored) - self.count(Status::NoCoverage);
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
    sites::find(
        &project.workspace,
        &project.crate_name,
        &project.roots,
        operators,
    )
}

pub fn run(
    project: &Project,
    options: &Options,
    progress: &mut dyn FnMut(&str),
) -> Result<Report, String> {
    let started = Instant::now();
    let found = find(project, &options.operators);
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
    let results = testrun::parallel(&order, options.workers, |&mutant| {
        result(&found, mutant, &built, &tests, &reach, &cwd)
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

fn result(
    found: &Crate,
    index: usize,
    built: &compile::Built,
    tests: &[Test],
    reach: &[Reach],
    cwd: &std::path::Path,
) -> MutantResult {
    let mutant = &found.mutants[index];
    let function = &found.functions[mutant.function];
    let covering: Vec<usize> = (0..tests.len())
        .filter(|&t| reach[t].hits.contains(&mutant.ordinal))
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
        let names: Vec<&str> = group.iter().map(|&t| tests[t].name.as_str()).collect();
        let timeout = (baseline * 5 + Duration::from_secs(1)).max(Duration::from_secs(10));
        let outcome: Outcome = testrun::run(
            &built.binaries[binary],
            cwd,
            &names,
            &[("MUTASTIC_ACTIVE", mutant.ordinal.to_string())],
            timeout,
        );
        result.tests_run += group.len();
        if outcome.timed_out {
            result.status = Status::Timeout;
            break;
        }
        if !outcome.passed {
            result.status = Status::Killed;
            result.killed_with = outcome.failure.clone();
            result.killed_by = outcome.failed.as_ref().map(|name| {
                group
                    .iter()
                    .find(|&&t| &tests[t].name == name)
                    .map(|&t| tests[t].display.clone())
                    .unwrap_or_else(|| name.clone())
            });
            break;
        }
    }
    result.took = started.elapsed();
    result
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
