//! A run: find the mutants, build them all at once, record what each test reaches, then run each
//! mutant against only the tests that reach it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::compile::{self, Binary};
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

    /// The status a schema name stands for: `Killed` → [`Status::Killed`].
    pub fn from_schema_name(name: &str) -> Option<Status> {
        [
            Status::Killed,
            Status::Survived,
            Status::Timeout,
            Status::NoCoverage,
            Status::Ignored,
            Status::RuntimeError,
        ]
        .into_iter()
        .find(|status| status.schema_name() == name)
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
    /// A test that would catch it, found by running its function with and without it.
    pub suggestion: Option<crate::suggest::Suggestion>,
}

#[derive(Debug)]
pub struct Report {
    pub project: Project,
    pub found: Crate,
    pub results: Vec<MutantResult>,
    pub tests: usize,
    pub test_runs: usize,
    /// How many times the crate was compiled with mutants in.
    pub rounds: usize,
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
    /// The tests that failed in the coverage pass and passed when run again: left out of the run.
    pub flaky: Vec<crate::flaky::Flaky>,
    /// Which share of a split run this is (`--shard`), and what it ran on.
    pub share: Option<crate::shares::Tag>,
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
    /// Look for a test that catches each missed mutant, by running its function.
    pub suggestions: bool,
    /// `--shard`: only this share of the mutants runs.
    pub shard: Option<crate::shares::Shard>,
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
        /// The tests left out because they failed, then passed when run again.
        flaky: usize,
    },
    /// The results are the shares' (`--from-shards`): none run here.
    FromShares { summary: &'a str, mutants: usize },
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
    let (found, stale_ignores) = find_for_run(project, options)?;
    let selected: BTreeSet<usize> = found
        .mutants
        .iter()
        .enumerate()
        .filter(|(_, m)| options.only.is_empty() || options.only.contains(&m.id))
        .filter(|(_, m)| options.shard.is_none_or(|shard| shard.holds(&m.id)))
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
    let listed = testrun::list(&built.binaries, &cwd)?;
    let setup = started.elapsed();

    let coverage_started = Instant::now();
    let covered = coverage(project, &built.binaries, &listed, options.workers)?;
    let flaky: Vec<crate::flaky::Flaky> = covered
        .flaky
        .iter()
        .map(|&index| {
            flaky_evidence(
                project,
                &found,
                &built.binaries,
                &listed[index],
                &covered.reach[index],
            )
        })
        .collect();
    // A flaky test left in would fail at random against mutants too, and its failures would count
    // as catches it never made.
    let (tests, reach): (Vec<Test>, Vec<Reach>) = listed
        .into_iter()
        .zip(covered.reach)
        .enumerate()
        .filter(|(index, _)| !covered.flaky.contains(index))
        .map(|(_, kept)| kept)
        .unzip();
    let coverage_took = coverage_started.elapsed();
    let rustc = rustc_version();
    progress(Progress::Covered {
        tests: tests.len(),
        rustc: &rustc,
        took: coverage_took,
        mutants: selected.len(),
        flaky: flaky.len(),
    });

    let mutants_started = Instant::now();
    let order: Vec<usize> = selected.iter().copied().collect();
    let first = first_at_site(&found, &live, &built);
    let files = Files::new(&found, &order);
    let mut results = testrun::parallel(&order, options.workers, |&mutant| {
        let result = result(&found, mutant, first[mutant], &built, &tests, &reach, &cwd);
        files.finished(&found, &result, progress);
        result
    });
    if options.suggestions {
        suggest_all(project, &found, &built, &mut results, options.workers);
    }
    // The tests run against mutants; the coverage pass's runs aren't counted, as in the Java
    // edition, so the count compares with running every test on every mutant.
    let test_runs = results.iter().map(|r| r.tests_run).sum::<usize>();
    Ok(Report {
        project: project.clone(),
        found,
        results,
        tests: tests.len(),
        test_runs,
        rounds: built.rounds.len(),
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
        flaky,
        share: None,
    })
}

/// The mutants a run finds, with what the project decided not to test skipped, and the ids of
/// the ignores that no longer hold. An id names its mutant whatever was chosen when it was shown,
/// so `--only` and `verify` look among them all.
pub fn find_for_run(project: &Project, options: &Options) -> Result<(Crate, Vec<String>), String> {
    let mut found = if options.only.is_empty() {
        find_with(project, &options.operators, options.unsafe_mode)
    } else {
        find_all(project)
    };
    let stale_ignores = apply_ignores(project, &mut found)?;
    apply_rules(&options.rules, &project.crate_name, &mut found);
    Ok((found, stale_ignores))
}

/// What a flaky test reaches that varies from run to run: in the functions with mutants it ran,
/// and in its own code.
fn flaky_evidence(
    project: &Project,
    found: &Crate,
    binaries: &[Binary],
    test: &Test,
    reach: &Reach,
) -> crate::flaky::Flaky {
    let mut reached: BTreeSet<usize> = reach
        .hits
        .iter()
        .filter_map(|&hit| found.mutants.iter().find(|m| m.ordinal == hit))
        .map(|mutant| mutant.function)
        .collect();
    reached.extend(
        reach
            .seen
            .keys()
            .filter_map(|&ordinal| found.functions.iter().position(|f| f.ordinal == ordinal)),
    );
    let binary = &binaries[test.binary];
    let sources = crate::flaky::test_sources(
        found,
        &project.workspace.join(&project.package_dir),
        &binary.kind,
        &binary.target,
    );
    crate::flaky::Flaky {
        test: test.display.clone(),
        reaches: crate::flaky::reaches(found, &reached),
        reaches_in_test: crate::flaky::reaches_in_test(&sources, &test.name),
    }
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
/// something else already skips keeps that reason, except that a `mutate:` rule takes back an arid
/// skip, as in the Java edition: the project says that output is a contract. A rule names a
/// function by its path in the crate, as `generated::checksum`, `crate::generated::checksum` or,
/// with the crate's name, `shop::generated::checksum`.
fn apply_rules(rules: &[crate::config::Rule], crate_name: &str, found: &mut Crate) {
    for index in 0..found.mutants.len() {
        let path = &found.functions[found.mutants[index].function].path;
        let named = [
            path.clone(),
            format!("crate::{path}"),
            format!("{crate_name}::{path}"),
        ];
        let covering = |mutate: bool| {
            rules
                .iter()
                .find(|rule| rule.mutate == mutate && named.iter().any(|path| rule.covers(path)))
        };
        let skip = covering(false);
        let mutate = covering(true);
        let mutant = &mut found.mutants[index];
        if mutate.is_some() && mutant.skipped.as_deref().is_some_and(is_arid) {
            mutant.skipped = None;
        }
        if let (Some(rule), None) = (skip, &mutant.skipped) {
            mutant.skipped = Some(rule.skip_reason());
        }
    }
}

/// `arid: only writes output`: a skip a `mutate:` rule may take back.
fn is_arid(reason: &str) -> bool {
    reason.starts_with("arid:")
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
pub(crate) struct Files {
    state: std::sync::Mutex<BTreeMap<PathBuf, (usize, Vec<MutantResult>)>>,
}

impl Files {
    pub(crate) fn new(found: &Crate, order: &[usize]) -> Files {
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
    pub(crate) fn finished(
        &self,
        found: &Crate,
        result: &MutantResult,
        progress: &(dyn Fn(Progress) + Sync),
    ) {
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
pub fn clock() -> Option<String> {
    let output = std::process::Command::new("date")
        .arg("+%H:%M:%S")
        .output()
        .ok()?;
    let time = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (output.status.success() && time.len() == 8).then_some(time)
}

/// What the coverage pass found: each test's reach, and which tests are flaky.
struct Covered {
    reach: Vec<Reach>,
    /// Indexes of the tests that failed, then passed when run again on their own.
    flaky: Vec<usize>,
}

/// Runs each test alone, with the switches recording which sites it reaches and the whole
/// numbers each function was called with. A test that fails runs again on its own, up to
/// [`crate::flaky::RERUNS`] times: one that passes is flaky, and left out of the run, as in the
/// Java edition. One that fails every time stops the run, since a test that fails with no mutant
/// in would look as if it caught every mutant it met.
fn coverage(
    project: &Project,
    binaries: &[Binary],
    tests: &[Test],
    workers: usize,
) -> Result<Covered, String> {
    let dir = project.mutastic_dir().join("coverage");
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let cwd = project.copy_dir().join(&project.package_dir);
    let cover = |index: usize, test: &Test| {
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
    };
    let indexed: Vec<(usize, &Test)> = tests.iter().enumerate().collect();
    let mut reached = testrun::parallel(&indexed, workers, |&(index, test)| cover(index, test));
    let mut flaky = Vec::new();
    let mut failing = Vec::new();
    for (index, test) in tests.iter().enumerate() {
        if reached[index].0.passed {
            continue;
        }
        for _ in 0..crate::flaky::RERUNS {
            reached[index] = cover(index, test);
            if reached[index].0.passed {
                break;
            }
        }
        if reached[index].0.passed {
            flaky.push(index);
        } else {
            failing.push(index);
        }
    }
    if !failing.is_empty() {
        let log = project.mutastic_dir().join("tests.log");
        let output: String = failing
            .iter()
            .map(|&index| {
                let outcome = &reached[index].0;
                format!(
                    "── {} ──\n{}{}\n",
                    tests[index].display, outcome.stdout, outcome.stderr
                )
            })
            .collect();
        std::fs::write(&log, output).map_err(|error| error.to_string())?;
        let named: Vec<(String, bool)> = failing
            .iter()
            .map(|&index| (tests[index].display.clone(), reached[index].0.timed_out))
            .collect();
        let shown = crate::report::files::shown(&project.workspace, &log);
        return Err(crate::flaky::failed_message(
            &named,
            std::path::Path::new(&shown),
        ));
    }
    let reach = reached
        .into_iter()
        .map(|(outcome, mut found)| {
            found.took = outcome.took;
            found
        })
        .collect();
    Ok(Covered { reach, flaky })
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
            suggestion: None,
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
        suggestion: None,
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

/// Looks for a test that catches each missed mutant whose function has a probe, side by side.
fn suggest_all(
    project: &Project,
    found: &Crate,
    built: &compile::Built,
    results: &mut [MutantResult],
    workers: usize,
) {
    let cwd = project.copy_dir().join(&project.package_dir);
    let dir = project.mutastic_dir().join("probes");
    let missed: Vec<usize> = (0..results.len())
        .filter(|&index| results[index].status == Status::Survived)
        .filter(|&index| {
            built
                .probes
                .contains(&found.mutants[results[index].mutant].function)
        })
        .collect();
    if missed.is_empty() || std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let probes = testrun::probes(&built.binaries, &cwd);
    let found_tests = testrun::parallel(&missed, workers, |&index| {
        let result = &results[index];
        let mutant = &found.mutants[result.mutant];
        let function = &found.functions[mutant.function];
        let callable = function.callable.as_ref()?;
        let name = callable.probe_name(function.ordinal);
        let &binary = probes.get(&name)?;
        let constants = crate::suggest::constants(&source_line(found, result.mutant));
        let candidates = crate::suggest::candidates(&callable.types(), &constants, &result.inputs);
        let probe = crate::suggest::Probe {
            binary: &built.binaries[binary],
            cwd: &cwd,
            name: &name,
        };
        crate::suggest::find(&probe, &dir, mutant.ordinal, &candidates)
    });
    for (index, suggestion) in missed.into_iter().zip(found_tests) {
        results[index].suggestion = suggestion;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A `mutate:` rule takes back an arid skip, and only that: a `skip:` rule still skips.
    #[test]
    fn a_mutate_rule_takes_back_an_arid_skip() {
        let dir = std::env::temp_dir().join(format!("mutastic-rules-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let source = "pub fn told(b: i32) { eprintln!(\"{b}\"); }\n\
                      pub fn quiet(b: i32) { eprintln!(\"{b}\"); }\n";
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let mut found = sites::find(
            &dir,
            "shop",
            &[PathBuf::from("src/lib.rs")],
            &[sites::REMOVE_VOID_CALL],
        );
        std::fs::remove_dir_all(&dir).ok();
        let rule = |mutate: bool, target: &str| crate::config::Rule {
            mutate,
            target: target.into(),
            reason: "the log is what the operator reads".into(),
        };
        apply_rules(
            &[rule(true, "shop::told"), rule(false, "quiet")],
            "shop",
            &mut found,
        );
        let skipped: Vec<Option<&str>> =
            found.mutants.iter().map(|m| m.skipped.as_deref()).collect();
        assert_eq!(skipped, [None, Some("arid: only writes output")]);
    }
}
