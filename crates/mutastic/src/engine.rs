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
    /// When the run started, `15:04:05` on the local clock, when the clock could be read.
    pub started_at: Option<String>,
    /// The report files written, as the summary shows them: `target/mutastic/mutation.json`.
    pub reports: Vec<String>,
    /// The ignores whose line has changed since: they no longer hold, and their mutants ran.
    pub stale_ignores: Vec<String>,
    /// The change the run was scoped to, with `--since`.
    pub scope: Option<Scope>,
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
    /// `--since`: only the mutants on these lines run.
    pub since: Option<crate::since::ChangedLines>,
    /// `mutastic.yaml`'s rules.
    pub rules: Vec<crate::config::Rule>,
}

/// The change a run was scoped to: what it's since, and the crate's lines and files it touched.
#[derive(Clone, Debug)]
pub struct Scope {
    pub since: String,
    pub lines: usize,
    pub files: usize,
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
        &crate::cfg::Cfg::here(project.features.clone()),
    )
}

/// Every mutant, under every operator mutastic has, unsafe code included: what an id can name.
pub fn find_all(project: &Project) -> Crate {
    let all: Vec<&str> = sites::ALL
        .iter()
        .copied()
        .chain(crate::callrules::loaded().iter().map(|rule| rule.id))
        .collect();
    find_with(project, &all, UnsafeMode::All)
}

/// What a run tells the person watching, as it goes.
pub enum Progress<'a> {
    /// The one build of every mutant is starting.
    Building { mutants: usize },
    /// The tests ran once each, recording what they reach; the mutants run next.
    Covered {
        tests: usize,
        rustc: &'a str,
        took: Duration,
        mutants: usize,
    },
    /// One mutant has its result.
    MutantFinished { status: Status },
    /// Every mutant of one source file has its result, in the order found.
    FileFinished {
        found: &'a Crate,
        results: &'a [MutantResult],
    },
}

pub fn run(
    project: &Project,
    options: &Options,
    progress: &(dyn Fn(Progress) + Sync),
) -> Result<Report, String> {
    let started = Instant::now();
    let started_at = clock();
    // An id names its mutant whatever was chosen when it was shown, so `--only` and `verify`
    // look among them all.
    let mut found = if options.only.is_empty() {
        find_with(project, &options.operators, options.unsafe_mode)
    } else {
        find_all(project)
    };
    let stale_ignores = apply_ignores(project, &mut found)?;
    apply_rules(&options.rules, &project.crate_name, &mut found);
    let selected: BTreeSet<usize> = found
        .mutants
        .iter()
        .enumerate()
        .filter(|(_, m)| options.only.is_empty() || options.only.contains(&m.id))
        .filter(|(_, m)| {
            options
                .since
                .as_ref()
                .is_none_or(|changed| changed.touches(&project.workspace.join(&m.file), m.line))
        })
        .map(|(index, _)| index)
        .collect();
    let scope = options.since.as_ref().map(|changed| {
        let ours: Vec<usize> = found
            .files
            .iter()
            .filter_map(|file| {
                let path = std::fs::canonicalize(project.workspace.join(&file.path)).ok()?;
                changed.lines.get(&path).map(|lines| lines.len())
            })
            .collect();
        Scope {
            since: changed.since.clone(),
            lines: ours.iter().sum(),
            files: ours.len(),
        }
    });
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
    progress(Progress::Building {
        mutants: selected.len(),
    });
    // A skipped mutant isn't built or run: it's there for its id and its reason.
    let live: BTreeSet<usize> = selected
        .iter()
        .copied()
        .filter(|&m| found.mutants[m].skipped.is_none())
        .collect();
    let built = compile::build(project, &found, &live, &recorders)?;
    let cwd = project.copy_dir().join(&project.package_dir);
    let tests = testrun::list(&built.binaries, &cwd)?;
    let setup = started.elapsed();

    let coverage_started = Instant::now();
    let reach = coverage(project, &built.binaries, &tests, options.workers)?;
    let coverage_took = coverage_started.elapsed();
    let rustc = rustc_version();
    progress(Progress::Covered {
        tests: tests.len(),
        rustc: &rustc,
        took: coverage_took,
        mutants: selected.len(),
    });

    let mutants_started = Instant::now();
    let order: Vec<usize> = selected.iter().copied().collect();
    let first = first_at_site(&found, &live, &built);
    let files = Files::new(&found, &order);
    let results = testrun::parallel(&order, options.workers, |&mutant| {
        let result = result(&found, mutant, first[mutant], &built, &tests, &reach, &cwd);
        files.finished(&found, &result, progress);
        result
    });
    // The tests run against mutants; the coverage pass's runs aren't counted, as in the Java
    // edition, so the count compares with running every test on every mutant.
    let test_runs = results.iter().map(|r| r.tests_run).sum::<usize>();
    Ok(Report {
        project: project.clone(),
        found,
        results,
        tests: tests.len(),
        test_runs,
        rounds: built.rounds,
        workers: options.workers,
        rustc,
        threshold: options.threshold,
        setup,
        coverage: coverage_took,
        mutants: mutants_started.elapsed(),
        took: started.elapsed(),
        unsafe_mode: options.unsafe_mode,
        profile: built.profile,
        started_at,
        reports: Vec::new(),
        stale_ignores,
        scope,
    })
}

/// Skips each mutant the project decided not to test, in `.mutastic/ignores.yaml`, while its line
/// is as it was when decided; gives the ids of those whose line has changed, which run again. A
/// mutant a filter already skips keeps the filter's reason.
fn apply_ignores(project: &Project, found: &mut Crate) -> Result<Vec<String>, String> {
    let mut stale = Vec::new();
    for ignore in crate::ignores::read(std::path::Path::new(&project.given))? {
        let Some(index) = found.mutants.iter().position(|m| m.id == ignore.id) else {
            continue;
        };
        let line = source_line(found, index);
        let mutant = &mut found.mutants[index];
        if !ignore.holds_for(&line) {
            stale.push(ignore.id);
        } else if mutant.skipped.is_none() {
            mutant.skipped = Some(ignore.skip_reason());
        }
    }
    Ok(stale)
}

/// Skips the mutants in code a `skip:` rule of `mutastic.yaml` covers, with its reason. A mutant
/// something else already skips keeps that reason. A rule names a function by its path in the
/// crate, as `generated::checksum`, `crate::generated::checksum` or, with the crate's name,
/// `shop::generated::checksum`.
fn apply_rules(rules: &[crate::config::Rule], crate_name: &str, found: &mut Crate) {
    for index in 0..found.mutants.len() {
        let path = &found.functions[found.mutants[index].function].path;
        let named = [
            path.clone(),
            format!("crate::{path}"),
            format!("{crate_name}::{path}"),
        ];
        let rule = rules
            .iter()
            .find(|rule| !rule.mutate && named.iter().any(|path| rule.covers(path)));
        if let (Some(rule), None) = (rule, &found.mutants[index].skipped) {
            found.mutants[index].skipped = Some(rule.skip_reason());
        }
    }
}

/// The source line a mutant is on, as written.
pub fn source_line(found: &Crate, index: usize) -> String {
    let mutant = &found.mutants[index];
    found
        .files
        .iter()
        .find(|file| file.path == mutant.file)
        .and_then(|file| file.text.lines().nth(mutant.line - 1))
        .unwrap_or_default()
        .to_string()
}

/// Each source file's mutants still running, and the results of those done, so a file's line can
/// be shown the moment its last mutant finishes.
struct Files {
    state: std::sync::Mutex<BTreeMap<PathBuf, (usize, Vec<MutantResult>)>>,
}

impl Files {
    fn new(found: &Crate, order: &[usize]) -> Files {
        let mut state: BTreeMap<PathBuf, (usize, Vec<MutantResult>)> = BTreeMap::new();
        for &index in order {
            state
                .entry(found.mutants[index].file.clone())
                .or_default()
                .0 += 1;
        }
        Files {
            state: std::sync::Mutex::new(state),
        }
    }

    /// Records `result`, and tells `progress` when it was its file's last. The lock is held while
    /// it's told, so two files' lines never interleave.
    fn finished(&self, found: &Crate, result: &MutantResult, progress: &(dyn Fn(Progress) + Sync)) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let Some((left, done)) = state.get_mut(&found.mutants[result.mutant].file) else {
            return;
        };
        done.push(result.clone());
        *left -= 1;
        progress(Progress::MutantFinished {
            status: result.status,
        });
        if *left == 0 {
            done.sort_by_key(|r| r.mutant);
            progress(Progress::FileFinished {
                found,
                results: done,
            });
        }
    }
}

/// The local time of day, `15:04:05`, from `date`: reading the local clock otherwise takes either
/// unsafe code, which this crate forbids, or a dependency for one line of the summary.
fn clock() -> Option<String> {
    let output = std::process::Command::new("date")
        .arg("+%H:%M:%S")
        .output()
        .ok()?;
    let time = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (output.status.success() && time.len() == 8).then_some(time)
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
    if let Some(reason) = &mutant.skipped {
        return MutantResult {
            mutant: index,
            status: Status::Ignored,
            reason: Some(reason.clone()),
            covered_by: Vec::new(),
            killed_by: None,
            killed_with: None,
            tests_run: 0,
            took: Duration::ZERO,
            inputs: BTreeMap::new(),
        };
    }
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
