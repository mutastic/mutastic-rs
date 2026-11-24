//! A run: find the mutants, build them all at once, record what each test reaches, then run each
//! mutant against only the tests that reach it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::compile::{self, Binary};
use crate::failure::{Code, Failure};
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

    /// The events format's name for a count of this status in `run-finished`:
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

    /// Whether a mutant asked for by id passes, as `verify` judges it: caught, skipped, or crashed
    /// where a crash proves nothing, which leaves nothing for a test to catch.
    pub fn settled(self) -> bool {
        self.detected() || matches!(self, Status::Ignored | Status::RuntimeError)
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
    /// With `--kill-matrix`, every covering test that catches it, `killed_by` first, then the
    /// rest in the order they're listed; empty otherwise, and when no test did.
    pub killed_by_all: Vec<String>,
    /// With `--confirm-catches`, a test that failed against the mutant, then passed against it
    /// when run again on its own: its catch didn't count, and the other covering tests decided.
    pub unconfirmed: Option<String>,
    /// Where the catching test's failure came from: `assertion`, `test`, `code` or `unknown`
    /// ([`crate::origin`]).
    pub failure_origin: Option<String>,
    /// A test that failed against the mutant, then failed on its own with no mutant in too: its
    /// catch didn't count, and the mutant ran again without it.
    pub false_catch_by: Option<String>,
    pub tests_run: usize,
    pub took: Duration,
    /// The whole numbers its function was called with, by parameter.
    pub inputs: BTreeMap<String, Vec<i128>>,
    /// A test that would catch it, found by running its function with and without it.
    pub suggestion: Option<crate::suggest::Suggestion>,
    /// Whether the test that caught it failed on an assertion of its own (`assert!`,
    /// `assert_eq!`), not on a panic in the code it ran: what an audit of the arid rules counts.
    pub by_assertion: bool,
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
    /// Reading, building and listing the tests, the build included.
    pub setup: Duration,
    /// The build alone, of `setup`; zero when it wasn't timed here.
    pub build: Duration,
    pub coverage: Duration,
    pub mutants: Duration,
    /// Checking each catch on the code as it is, after the mutants, when there was a miss.
    pub checking: Option<Duration>,
    /// Writing the report files, after the run, when there were any to write: `took` counts it.
    pub reporting: Option<Duration>,
    /// Whether `run-started` and each `mutant-finished` went out live, with the run's progress,
    /// rather than with the rest.
    pub streamed: bool,
    pub took: Duration,
    /// How long the machine slept during the run, which `took` leaves out, when it's worth saying
    /// (see [`crate::slept`]); else zero.
    pub slept: Duration,
    pub unsafe_mode: UnsafeMode,
    /// The settings the user's crate was built with, from Cargo: `opt_level`, `debug_assertions`,
    /// `overflow_checks`.
    pub profile: serde_json::Value,
    /// When the run started, `15:04:05` on the local clock, when the clock could be read.
    pub started_at: Option<String>,
    /// When the run started, for `mutation.json`.
    pub started: std::time::SystemTime,
    /// The report files written, as `run-finished` lists them: `target/mutastic/mutation.json`,
    /// from the directory mutastic was pointed at.
    pub reports: Vec<String>,
    /// The same files, each whole: the summary shows them from where mutastic runs, and links
    /// them.
    pub written: Vec<std::path::PathBuf>,
    /// Beside the HTML report this run wrote, the folder of a page for each mutant it didn't catch,
    /// which opens the report on that mutant: what the human output's ids link to.
    pub pages: Option<std::path::PathBuf>,
    /// The ignores whose line has changed since: they no longer hold, and their mutants ran.
    pub stale_ignores: Vec<String>,
    /// The change the run was scoped to, with `--since`.
    pub scope: Option<Scope>,
    /// The tests that failed in the coverage pass and passed when run again, then those that
    /// caught a mutant and failed on their own with no mutant in: left out of the run.
    pub flaky: Vec<crate::flaky::Flaky>,
    /// Each mutant that ran again without a test that fails on its own, by its index in
    /// `found.mutants`: its first result, which its `mutant-finished` gives, before the
    /// `mutant-revised` with the result in `results`.
    pub revised: BTreeMap<usize, MutantResult>,
    /// Which share of a split run this is (`--shard`), and what it ran on.
    pub share: Option<crate::shares::Tag>,
    /// The operators the run used, by id: a score is a share of what they make.
    pub operators: Vec<&'static str>,
    /// This run against the last full run under the same operators, when there was one.
    pub comparison: Option<crate::record::Comparison>,
    /// With `--fail-on-new`, the baseline the run is judged by: it passes on the misses the
    /// baseline doesn't hold, not on its score.
    pub fail_on_new: Option<crate::baseline::FailOnNew>,
    /// Whether the build and the coverage pass ran: not when no mutant was there to run, unless
    /// `--force-coverage` asked for them anyway.
    pub built: bool,
    /// Whether the run was of some mutants, asked for by id (`--only`) or by their lines (`--at`):
    /// each one's fate is the verdict, not a score, as the Java edition judges it.
    pub only: bool,
    /// With `--at`, the lines asked for: the count line names them, and `run-started` and
    /// `run-finished` carry them.
    pub at: Vec<crate::at::Lines>,
    /// In an audit of the arid rules (`MUTASTIC_ARID=audit`), each arid mutant's result when run
    /// anyway, in the order found; none otherwise. Never in `results`, the counts or the score.
    pub audited: Vec<MutantResult>,
    /// The tests the run was told to leave out, mutastic.yaml's `exclude-tests:` then
    /// `--exclude-tests`, as given: the summary names them, each with its reason.
    pub left_out: Vec<LeftOut>,
    /// Each site the coverage pass measured, with the time of the tests that reach it, flaky tests
    /// left out: what `test-times.tsv` keeps. Empty when nothing was built.
    pub measured: BTreeMap<String, Duration>,
}

/// A test a run leaves out, by its name as given, and why: the reason under mutastic.yaml's
/// `exclude-tests:`, or none for one `--exclude-tests` adds for one run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftOut {
    pub test: String,
    pub reason: Option<String>,
}

impl LeftOut {
    /// mutastic.yaml's tests, then each of the command line's not among them.
    pub fn listed(configured: &[crate::config::ExcludedTest], given: &[String]) -> Vec<LeftOut> {
        let mut all: Vec<LeftOut> = configured
            .iter()
            .map(|test| LeftOut {
                test: test.test.clone(),
                reason: Some(test.reason.clone()),
            })
            .collect();
        for test in given {
            if !all.iter().any(|kept| &kept.test == test) {
                all.push(LeftOut {
                    test: test.clone(),
                    reason: None,
                });
            }
        }
        all
    }

    /// Each test's name, in order.
    pub fn names(left_out: &[LeftOut]) -> Vec<String> {
        left_out.iter().map(|test| test.test.clone()).collect()
    }
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

    /// The tests that caught mutants and fail on their own with no mutant in.
    pub fn failing_alone(&self) -> Vec<&crate::flaky::Flaky> {
        self.flaky
            .iter()
            .filter(|flaky| flaky.fails_alone)
            .collect()
    }

    /// Whether the run passes: never with a test that caught mutants and fails on its own, as its
    /// catches can't be trusted; with `--fail-on-new`, when no mutant no test catches is new;
    /// otherwise when the score reaches the threshold.
    pub fn passed(&self) -> bool {
        if !self.failing_alone().is_empty() {
            return false;
        }
        if self.only {
            return self.results.iter().all(|result| result.status.settled());
        }
        match &self.fail_on_new {
            Some(judged) => judged.passes(self),
            None => self.score().is_none_or(|score| score >= self.threshold),
        }
    }
}

impl Options {
    /// Whether the run is of some mutants: asked for by id, or by their lines.
    pub fn partial(&self) -> bool {
        !self.only.is_empty() || self.among.is_some()
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    pub only: Vec<String>,
    /// `--at`: only these of the mutants the run finds run, those on its lines.
    pub among: Option<BTreeSet<String>>,
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
    /// `--force-coverage`: build and run the coverage pass even when no mutant is there to run.
    pub force_coverage: bool,
    /// `--confirm-catches` and `--kill-matrix`: how a caught mutant's tests run.
    pub catches: Catches,
    /// mutastic.yaml's `exclude-tests:` and `--exclude-tests`: tests the run leaves out, each a
    /// test's name as a run shows it or a module of tests. What only they reach counts as not
    /// covered.
    pub exclude_tests: Vec<LeftOut>,
    /// `MUTASTIC_ARID=audit`: run the mutants mutastic's arid rules skip anyway, and say what
    /// happened to them, out of the score.
    pub audit_arid: bool,
}

/// How a caught mutant's tests run, as the Java edition's `Catches`: each catch run once more to
/// confirm it (`--confirm-catches`), and every covering test run, not only up to the first catch
/// (`--kill-matrix`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Catches {
    pub confirm: bool,
    pub every: bool,
}

/// Whether `--exclude-tests` leaves a test out: one of `left_out` is its name as a run shows it
/// (`pricing::tests::big_orders`), or a module above it (`pricing::tests`), which leaves out every
/// test in it, as a class named to the Java edition leaves out its methods.
pub fn leaves_out(left_out: &[LeftOut], test: &str) -> bool {
    left_out.iter().any(|name| {
        test.strip_prefix(name.test.as_str())
            .is_some_and(|rest| rest.is_empty() || rest.starts_with("::"))
    })
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
    /// Cargo is at work: its step (`check` or `build`), the round, the crates finished in it, and
    /// its latest `Checking`/`Compiling` line.
    Compiling {
        step: &'a str,
        round: usize,
        crates: usize,
        last: &'a str,
    },
    /// The build is done: each round's step, the mutants withdrawn as they didn't compile, and
    /// the profile Cargo built with.
    Built {
        mutants: usize,
        took: Duration,
        rounds: &'a [&'a str],
        withdrawn: usize,
        profile: &'a serde_json::Value,
    },
    /// A worker of the coverage pass starts a test.
    TestStarted { worker: usize, name: &'a str },
    /// The coverage pass has run `done` of its `total` tests.
    Tested { done: usize, total: usize },
    /// The tests ran once each, recording what they reach; the mutants run next.
    Covered {
        tests: usize,
        rustc: &'a str,
        took: Duration,
        mutants: usize,
        /// The tests left out because they failed, then passed when run again.
        flaky: usize,
        /// What the mutants should take, all told: each the time its covering tests took.
        cost: Duration,
    },
    /// The results are the shares' (`--from-shards`): none run here.
    FromShares { summary: &'a str, mutants: usize },
    /// No mutant is there to run, so nothing is built and no test runs.
    NothingToRun,
    /// A worker starts a mutant, expected to take `expected`, as its covering tests did; it's
    /// slow past `slow`.
    MutantStarted {
        worker: usize,
        found: &'a Crate,
        mutant: usize,
        expected: Duration,
        slow: Duration,
        /// The tests that cover it.
        tests: usize,
    },
    /// One mutant has its first result, the result itself, in the package and among the mutants
    /// it's of; `cost` is what it was expected to take. Its `mutant-finished` goes out live.
    MutantFinished {
        result: &'a MutantResult,
        found: &'a Crate,
        project: &'a Project,
        worker: usize,
        cost: Duration,
    },
    /// Every mutant of one source file has its result, in the order found, `took` after its first
    /// started.
    FileFinished {
        found: &'a Crate,
        results: &'a [MutantResult],
        took: Duration,
    },
    /// Every mutant has its result.
    Ran { took: Duration },
    /// Every mutant has its result, and each catch is checked on the code as it is: `done` of
    /// `total`, when it's counted.
    Checking { done: usize, total: usize },
    /// The check is over: of the `missed` mutants, a test to suggest was found for `found`.
    Checked {
        missed: usize,
        took: Duration,
        found: usize,
    },
}

pub fn run(
    project: &Project,
    options: &Options,
    progress: &(dyn Fn(Progress) + Sync),
) -> Result<Report, Failure> {
    let started = Instant::now();
    let started_at = clock();
    let started_wall = std::time::SystemTime::now();
    let (found, stale_ignores) = find_for_run(project, options)?;
    let chosen: BTreeSet<&str> = found
        .mutants
        .iter()
        .filter(|m| options.only.is_empty() || options.only.contains(&m.id))
        .filter(|m| options.among.as_ref().is_none_or(|ids| ids.contains(&m.id)))
        .filter(|m| {
            options
                .since
                .as_ref()
                .is_none_or(|changed| changed.touches(&project.workspace.join(&m.file), m.line))
        })
        // A twin doesn't run: the mutant it's the same as runs for it.
        .flat_map(|m| std::iter::once(m.id.as_str()).chain(crate::sites::twins::partner_of(m)))
        .collect();
    let selected: BTreeSet<usize> = found
        .mutants
        .iter()
        .enumerate()
        .filter(|(_, m)| chosen.contains(m.id.as_str()))
        .filter(|(_, m)| options.shard.is_none_or(|shard| shard.holds(&m.id)))
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
                return Err(not_a_mutant(id, &project.package, &project.given, &found));
            }
        }
    }
    // A skipped mutant isn't built or run: it's there for its id and its reason.
    let live: BTreeSet<usize> = selected
        .iter()
        .copied()
        .filter(|&m| found.mutants[m].skipped.is_none())
        .collect();
    // In an audit, an arid mutant is built and run too, as if it weren't skipped, on the side.
    let audited: BTreeSet<usize> = selected
        .iter()
        .copied()
        .filter(|&m| options.audit_arid && is_arid(&found.mutants[m]))
        .collect();
    let unskipped = (!audited.is_empty()).then(|| {
        let mut unskipped = found.clone();
        for &m in &audited {
            unskipped.mutants[m].skipped = None;
        }
        unskipped
    });
    let built_set: BTreeSet<usize> = live.union(&audited).copied().collect();
    // With no mutant to run, a build and a pass of every test would take minutes to measure
    // nothing.
    if built_set.is_empty() && !options.force_coverage {
        progress(Progress::NothingToRun);
        let unbuilt = Unbuilt {
            started,
            started_at,
            started_wall,
            stale_ignores,
            scope,
        };
        return Ok(nothing_to_run(project, options, found, &selected, unbuilt));
    }
    let recorders: BTreeSet<usize> = selected
        .iter()
        .map(|&m| found.mutants[m].function)
        .collect();
    progress(Progress::Building {
        mutants: selected.len(),
    });
    let building = Instant::now();
    let built = compile::build(project, &found, &built_set, &recorders, progress)?;
    let build_took = building.elapsed();
    let steps: Vec<&str> = built.rounds.iter().map(|round| round.phase).collect();
    progress(Progress::Built {
        mutants: selected.len(),
        took: build_took,
        rounds: &steps,
        withdrawn: built.unviable.len(),
        profile: &built.profile,
    });
    let cwd = project.copy_dir().join(&project.package_dir);
    let mut listed =
        testrun::list(&built.binaries, &cwd).map_err(|error| Code::Other.with(error))?;
    if listed.is_empty() {
        let ignored = testrun::Names::of(&built.binaries, &cwd)
            .map_err(|error| Code::Other.with(error))?
            .ignored
            .len();
        if ignored > 0 {
            return Err(no_tests_ran(project, ignored));
        }
    }
    // Left out before the coverage pass, so a test that can't run here never runs: what only it
    // reaches is not covered.
    listed.retain(|test| !leaves_out(&options.exclude_tests, &test.display));
    let setup = started.elapsed();

    let coverage_started = Instant::now();
    let covered = coverage(project, &built.binaries, &listed, options.workers, progress)?;
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
    let (tests, reach) = steady(listed, covered);
    // Only a run of every mutant: tests that reach none of some of them can be the honest answer.
    let whole = !options.partial()
        && options.since.is_none()
        && options.shard.is_none_or(|shard| shard.of == 1);
    let with_mutants: BTreeSet<usize> = live
        .iter()
        .filter(|mutant| !built.unviable.contains_key(mutant))
        .map(|&mutant| found.mutants[mutant].function)
        .collect();
    let recorded_nothing = reach.iter().all(|reached| reached.hits.is_empty());
    if whole && !tests.is_empty() && !with_mutants.is_empty() && recorded_nothing {
        return Err(nothing_covered(project, tests.len(), with_mutants.len()));
    }
    let coverage_took = coverage_started.elapsed();
    let rustc = rustc_version();
    let order: Vec<usize> = selected.iter().copied().collect();
    let first = first_at_site(&found, &built_set, &built);
    let sites_measured = measured(&found, &built_set, &built, &tests, &reach, false);
    let costs: Vec<Cost> = order
        .iter()
        .map(|&mutant| cost(&found, mutant, first[mutant], &built, &tests, &reach))
        .collect();
    progress(Progress::Covered {
        tests: tests.len(),
        rustc: &rustc,
        took: coverage_took,
        mutants: selected.len(),
        flaky: flaky.len(),
        cost: costs.iter().map(|cost| cost.expected).sum(),
    });

    let mutants_started = Instant::now();
    let suggester = Suggester::new(project, &found, &built, options);
    let files = Files::new(&found, &order);
    let indexed: Vec<(usize, &Cost)> = order.iter().copied().zip(&costs).collect();
    // No more workers than mutants, as the Java edition starts them: `--only` on one is one.
    let workers = options.workers.min(order.len().max(1));
    let suite = Suite {
        built: &built,
        tests: &tests,
        reach: &reach,
        cwd: &cwd,
        catches: options.catches,
    };
    let ran = testrun::parallel_on(&indexed, workers, |worker, &(mutant, cost)| {
        if cost.expected > Duration::ZERO {
            progress(Progress::MutantStarted {
                worker,
                found: &found,
                mutant,
                expected: cost.expected,
                slow: cost.slow,
                tests: cost.tests,
            });
        }
        files.started(&found, mutant);
        let result = Suggester::with_suggestion(
            suggester.as_ref(),
            result(&found, mutant, first[mutant], &suite),
        );
        files.finished(
            (&found, project),
            &result,
            (worker, cost.expected),
            progress,
        );
        let audit = unskipped
            .as_ref()
            .filter(|_| audited.contains(&mutant))
            .map(|unskipped| self::result(unskipped, mutant, first[mutant], &suite));
        (result, audit)
    });
    let (mut results, audited): (Vec<MutantResult>, Vec<Option<MutantResult>>) =
        ran.into_iter().unzip();
    let audited: Vec<MutantResult> = audited.into_iter().flatten().collect();
    let mutants_took = mutants_started.elapsed();
    progress(Progress::Ran { took: mutants_took });
    // Each catch checked on the code as it is, once the mutants have run: the checking phase.
    let checking_started = Instant::now();
    progress(Progress::Checking { done: 0, total: 0 });
    let runs = BuildRuns {
        suggester: suggester.as_ref(),
        found: &found,
        first: &first,
        built: &built,
        tests: &tests,
        reach: &reach,
        cwd: &cwd,
        workers: options.workers,
        catches: options.catches,
    };
    let guarded = crate::guard::check(&mut results, &runs);
    let mut flaky = flaky;
    flaky.extend(guarded.failing_alone.iter().filter_map(|failing| {
        let index = tests.iter().position(|test| &test.display == failing)?;
        let mut evidence = flaky_evidence(
            project,
            &found,
            &built.binaries,
            &tests[index],
            &reach[index],
        );
        evidence.fails_alone = true;
        Some(evidence)
    }));
    let revised: BTreeMap<usize, MutantResult> = guarded
        .first
        .into_iter()
        .map(|(index, first)| (results[index].mutant, first))
        .collect();
    let missed = results
        .iter()
        .filter(|r| r.status == Status::Survived)
        .count();
    let checking_took = checking_started.elapsed();
    progress(Progress::Checked {
        missed,
        took: checking_took,
        found: results
            .iter()
            .filter(|r| r.status == Status::Survived && r.suggestion.is_some())
            .count(),
    });
    // As the Java edition's summary has it: the time after the mutants, when there was a miss to
    // say it about.
    let checking = (missed > 0).then_some(checking_took);
    // The tests run against mutants; the coverage pass's runs aren't counted, as in the Java
    // edition, so the count compares with running every test on every mutant.
    let test_runs = results
        .iter()
        .chain(revised.values())
        .map(|r| r.tests_run)
        .sum::<usize>();
    let took = started.elapsed();
    Ok(Report {
        project: project.clone(),
        found,
        results,
        tests: tests.len(),
        test_runs,
        rounds: built.rounds.len(),
        workers,
        rustc,
        threshold: options.threshold,
        setup,
        build: build_took,
        coverage: coverage_took,
        mutants: mutants_took,
        checking,
        reporting: None,
        streamed: false,
        took,
        slept: crate::slept::since(started_wall, took),
        unsafe_mode: options.unsafe_mode,
        profile: built.profile,
        started_at,
        started: started_wall,
        reports: Vec::new(),
        written: Vec::new(),
        pages: None,
        stale_ignores,
        scope,
        flaky,
        revised,
        share: None,
        operators: options.operators.clone(),
        comparison: None,
        fail_on_new: None,
        built: true,
        only: options.partial(),
        at: Vec::new(),
        audited,
        left_out: options.exclude_tests.clone(),
        measured: sites_measured,
    })
}

/// The tests the coverage pass found steady, with what each reaches. A flaky test left in would
/// fail at random against mutants too, and its failures would count as catches it never made.
fn steady(listed: Vec<Test>, covered: Covered) -> (Vec<Test>, Vec<Reach>) {
    listed
        .into_iter()
        .zip(covered.reach)
        .enumerate()
        .filter(|(index, _)| !covered.flaky.contains(index))
        .map(|(_, kept)| kept)
        .unzip()
}

/// `mutants --estimate`: builds every mutant `options.operators` make, runs the coverage pass
/// once, and runs no mutant. Gives each site it measured, with what a mutant there is expected to
/// take ([`measured`]).
pub fn estimate(
    project: &Project,
    options: &Options,
    progress: &(dyn Fn(Progress) + Sync),
) -> Result<BTreeMap<String, Duration>, Failure> {
    let (found, _) = find_for_run(project, options)?;
    // A skipped mutant isn't built or run, so it costs nothing and its site needn't be measured.
    let live: BTreeSet<usize> = (0..found.mutants.len())
        .filter(|&m| found.mutants[m].skipped.is_none())
        .collect();
    if live.is_empty() {
        progress(Progress::NothingToRun);
        return Ok(BTreeMap::new());
    }
    progress(Progress::Building {
        mutants: live.len(),
    });
    let building = Instant::now();
    // No recorder or probe: the pass measures only where each test goes, not what it passes.
    let built = compile::build(project, &found, &live, &BTreeSet::new(), progress)?;
    let steps: Vec<&str> = built.rounds.iter().map(|round| round.phase).collect();
    progress(Progress::Built {
        mutants: live.len(),
        took: building.elapsed(),
        rounds: &steps,
        withdrawn: built.unviable.len(),
        profile: &built.profile,
    });
    let cwd = project.copy_dir().join(&project.package_dir);
    let listed = testrun::list(&built.binaries, &cwd).map_err(|error| Code::Other.with(error))?;
    let covering = Instant::now();
    let covered = coverage(project, &built.binaries, &listed, options.workers, progress)?;
    let flaky = covered.flaky.len();
    let (tests, reach) = steady(listed, covered);
    let sites = measured(&found, &live, &built, &tests, &reach, true);
    let first = first_at_site(&found, &live, &built);
    progress(Progress::Covered {
        tests: tests.len(),
        rustc: &rustc_version(),
        took: covering.elapsed(),
        mutants: live.len(),
        flaky,
        cost: live
            .iter()
            .map(|&m| cost(&found, m, first[m], &built, &tests, &reach).expected)
            .sum(),
    });
    Ok(sites)
}

/// Whether one of mutastic's arid rules, or an ignore as arid, skipped the mutant: what an audit
/// runs anyway, as the Java edition's does.
fn is_arid(mutant: &crate::sites::Mutant) -> bool {
    mutant
        .skipped
        .as_deref()
        .is_some_and(|reason| reason.starts_with("arid:"))
}

/// What a run knows before its build: when it started, the ignores that no longer hold, and the
/// change it's scoped to.
struct Unbuilt {
    started: Instant,
    started_at: Option<String>,
    started_wall: std::time::SystemTime,
    stale_ignores: Vec<String>,
    scope: Option<Scope>,
}

/// The report of a run with no mutant to run: the skipped mutants it selected, with their ids and
/// reasons, and no build, test or score.
fn nothing_to_run(
    project: &Project,
    options: &Options,
    found: Crate,
    selected: &BTreeSet<usize>,
    unbuilt: Unbuilt,
) -> Report {
    let results = selected
        .iter()
        .filter_map(|&index| {
            let reason = found.mutants[index].skipped.clone()?;
            Some(MutantResult {
                mutant: index,
                status: Status::Ignored,
                reason: Some(reason),
                covered_by: Vec::new(),
                killed_by: None,
                killed_with: None,
                killed_by_all: Vec::new(),
                unconfirmed: None,
                failure_origin: None,
                false_catch_by: None,
                tests_run: 0,
                took: Duration::ZERO,
                inputs: BTreeMap::new(),
                suggestion: None,
                by_assertion: false,
            })
        })
        .collect();
    let setup = unbuilt.started.elapsed();
    Report {
        project: project.clone(),
        found,
        results,
        tests: 0,
        test_runs: 0,
        rounds: 0,
        workers: options.workers,
        rustc: rustc_version(),
        threshold: options.threshold,
        setup,
        build: Duration::ZERO,
        coverage: Duration::ZERO,
        mutants: Duration::ZERO,
        checking: None,
        reporting: None,
        streamed: false,
        took: setup,
        slept: crate::slept::since(unbuilt.started_wall, setup),
        unsafe_mode: options.unsafe_mode,
        // Nothing was built, so Cargo reported nothing: only what mutastic's own profile forces.
        profile: serde_json::json!({ "overflow_checks": true, "debug_assertions": true }),
        started_at: unbuilt.started_at,
        started: unbuilt.started_wall,
        reports: Vec::new(),
        written: Vec::new(),
        pages: None,
        stale_ignores: unbuilt.stale_ignores,
        scope: unbuilt.scope,
        flaky: Vec::new(),
        revised: BTreeMap::new(),
        share: None,
        operators: options.operators.clone(),
        comparison: None,
        fail_on_new: None,
        built: false,
        only: options.partial(),
        at: Vec::new(),
        audited: Vec::new(),
        left_out: options.exclude_tests.clone(),
        measured: BTreeMap::new(),
    }
}

/// The mutants a run finds, with what the project decided not to test skipped, and the ids of
/// the ignores that no longer hold. An id names its mutant whatever was chosen when it was shown,
/// so `--only` and `verify` look among them all.
pub fn find_for_run(project: &Project, options: &Options) -> Result<(Crate, Vec<String>), Failure> {
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
        fails_alone: false,
        reaches: crate::flaky::reaches(found, &reached),
        reaches_in_test: crate::flaky::reaches_in_test(&sources, &test.name),
    }
}

/// Skips each mutant the project decided not to test, in `.mutastic/ignores.yaml`, while its line
/// is as it was when decided; gives the ids of those whose line has changed, which run again. A
/// mutant a filter already skips keeps the filter's reason.
fn apply_ignores(project: &Project, found: &mut Crate) -> Result<Vec<String>, Failure> {
    let mut stale = Vec::new();
    for ignore in crate::ignores::read(std::path::Path::new(&project.given))? {
        let Some(index) = found.mutants.iter().position(|m| m.id == ignore.id) else {
            continue;
        };
        let line = source_line(found, index);
        if !ignore.holds_for(&line) {
            stale.push(ignore.id);
            continue;
        }
        // An ignore of a twin, kept maybe from before mutastic knew the two are the same, is the
        // decision of the mutant that runs for it.
        let partner = crate::sites::twins::partner_of(&found.mutants[index])
            .and_then(|partner| found.mutants.iter().position(|m| m.id == partner));
        let mutant = &mut found.mutants[partner.unwrap_or(index)];
        if mutant.skipped.is_none() {
            mutant.skipped = Some(ignore.skip_reason());
        }
    }
    Ok(stale)
}

/// Skips the mutants in code a `skip:` rule of `mutastic.yaml` covers, with its reason. A mutant
/// something else already skips keeps that reason, except that a `mutate:` rule takes back an arid
/// skip, as in the Java edition: the project says that output is a contract. With an `arid:` list,
/// it takes back only the skips of the arid rules it names. A rule names a
/// function by its path in the crate, as `generated::checksum`, `crate::generated::checksum` or,
/// with the crate's name, `shop::generated::checksum`.
fn apply_rules(rules: &[crate::config::Rule], crate_name: &str, found: &mut Crate) {
    own_output(rules, found);
    for index in 0..found.mutants.len() {
        let path = &found.functions[found.mutants[index].function].path;
        let named = [
            path.clone(),
            format!("crate::{path}"),
            format!("{crate_name}::{path}"),
        ];
        let covers = |rule: &&crate::config::Rule| named.iter().any(|path| rule.covers(path));
        let skip = rules.iter().filter(|rule| rule.skips()).find(covers);
        let mutant = &mut found.mutants[index];
        if let Some(reason) = mutant.skipped.as_deref() {
            if rules
                .iter()
                .filter(covers)
                .any(|rule| rule.brings_back(reason))
            {
                mutant.skipped = None;
            }
        }
        if let (Some(rule), None) = (skip, &mutant.skipped) {
            mutant.skipped = Some(rule.skip_reason());
        }
    }
}

/// A statement removed whose call an `output:` rule names, `audit::record(&order);` or
/// `metrics::counter!("orders").increment(1);`, skipped as arid with the rule's reason, as an
/// output macro's is with mutastic's own.
fn own_output(rules: &[crate::config::Rule], found: &mut Crate) {
    if !rules.iter().any(|rule| rule.output) {
        return;
    }
    for mutant in &mut found.mutants {
        if mutant.operator != sites::REMOVE_VOID_CALL || mutant.skipped.is_some() {
            continue;
        }
        let Some(file) = found.files.iter().find(|file| file.path == mutant.file) else {
            continue;
        };
        let path = called_path(file.text.get(mutant.site.clone()).unwrap_or_default());
        if let Some(rule) = rules.iter().find(|rule| rule.names_call(path)) {
            mutant.skipped = Some(rule.output_reason(path));
        }
    }
}

/// The path a statement starts by calling, `audit::record` of `audit::record(&order);` and
/// `metrics::counter` of `metrics::counter!("orders").increment(1);`: empty when it starts with
/// anything else, a method called on a value or an assignment.
fn called_path(statement: &str) -> &str {
    let statement = statement.trim_start();
    let end = statement
        .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':'))
        .unwrap_or(statement.len());
    let path = statement[..end].trim_end_matches(':');
    let rest = statement[end..].trim_start();
    if rest.starts_with('(') || rest.starts_with('!') {
        path
    } else {
        ""
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
pub(crate) struct Files {
    state: std::sync::Mutex<BTreeMap<PathBuf, FileState>>,
}

/// A file's mutants still to finish, the results of those done, and when its first started.
#[derive(Default)]
pub(crate) struct FileState {
    left: usize,
    done: Vec<MutantResult>,
    first: Option<Instant>,
}

impl Files {
    pub(crate) fn new(found: &Crate, order: &[usize]) -> Files {
        let mut state: BTreeMap<PathBuf, FileState> = BTreeMap::new();
        for &index in order {
            state
                .entry(found.mutants[index].file.clone())
                .or_default()
                .left += 1;
        }
        Files {
            state: std::sync::Mutex::new(state),
        }
    }

    /// Notes when a file's first mutant starts, so its line says how long the file took, not the
    /// sum of its mutants' times across the workers.
    pub(crate) fn started(&self, found: &Crate, mutant: usize) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(file) = state.get_mut(&found.mutants[mutant].file) {
                file.first.get_or_insert_with(Instant::now);
            }
        }
    }

    /// Records `result`, and tells `progress` when it was its file's last. The lock is held while
    /// it's told, so two files' lines never interleave.
    pub(crate) fn finished(
        &self,
        (found, project): (&Crate, &Project),
        result: &MutantResult,
        (worker, cost): (usize, Duration),
        progress: &(dyn Fn(Progress) + Sync),
    ) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let Some(file) = state.get_mut(&found.mutants[result.mutant].file) else {
            return;
        };
        file.done.push(result.clone());
        file.left -= 1;
        progress(Progress::MutantFinished {
            result,
            found,
            project,
            worker,
            cost,
        });
        if file.left == 0 {
            file.done.sort_by_key(|r| r.mutant);
            let summed = || file.done.iter().map(|r| r.took).sum();
            progress(Progress::FileFinished {
                found,
                results: &file.done,
                took: file.first.map_or_else(summed, |first| first.elapsed()),
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
    progress: &(dyn Fn(Progress) + Sync),
) -> Result<Covered, Failure> {
    let dir = project.mutastic_dir().join("coverage");
    std::fs::create_dir_all(&dir).map_err(|error| {
        crate::failure::disk(format!("couldn't make {}: {error}", dir.display()))
    })?;
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
    let tested = std::sync::atomic::AtomicUsize::new(0);
    progress(Progress::Tested {
        done: 0,
        total: tests.len(),
    });
    let mut reached = testrun::parallel_on(&indexed, workers, |worker, &(index, test)| {
        progress(Progress::TestStarted {
            worker,
            name: &test.display,
        });
        let covered = cover(index, test);
        progress(Progress::Tested {
            done: tested.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1,
            total: tests.len(),
        });
        covered
    });
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
        std::fs::write(&log, output).map_err(|error| {
            crate::failure::disk(format!("couldn't write {}: {error}", log.display()))
        })?;
        let named: Vec<crate::flaky::Red> = failing
            .iter()
            .map(|&index| {
                let test = &tests[index];
                let outcome = &reached[index].0;
                let output = format!("{}\n{}", outcome.stdout, outcome.stderr);
                let seeded = crate::flaky::seeded(&output);
                let command = rerun(project, &binaries[test.binary], &test.name);
                crate::flaky::Red {
                    display: test.display.clone(),
                    timed_out: outcome.timed_out,
                    stopped: crate::flaky::stopped(outcome),
                    panic: crate::flaky::panic_message(&output),
                    rerun: if seeded {
                        format!("PROPTEST_RNG_SEED={} {command}", testrun::PROPTEST_SEED)
                    } else {
                        command
                    },
                    seeded,
                    case: crate::cases::of(project, &test.display),
                }
            })
            .collect();
        // From where the person is, as they'd open it.
        let here = std::env::current_dir().unwrap_or_else(|_| project.workspace.clone());
        let shown = crate::report::files::shown(&here, &log);
        return Err(
            crate::flaky::code(&named).with(crate::flaky::failed_message(
                &named,
                std::path::Path::new(&shown),
            )),
        );
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

/// The package has tests, and every one is `#[ignore]`d, so none ran: as the Java edition stops a
/// run whose module has test classes none of which ran. Scored, that would be every mutant not
/// covered, a 0% that reads as tests that don't reach the code. A package with no tests at all
/// isn't an error: its mutants are not covered, and the summary says so.
fn no_tests_ran(project: &Project, ignored: usize) -> Failure {
    Code::NoTests.with(format!(
        "no test ran in {}, so there's nothing to judge a mutant by. mutastic runs the tests `cargo \
         test` runs, and {}: take the #[ignore] off one that calls this code, or write one",
        project.package,
        if ignored == 1 {
            "its only test is #[ignore]d".to_string()
        } else {
            format!("all {ignored} of its tests are #[ignore]d")
        }
    ))
}

/// The tests ran and passed, and the coverage pass recorded none of the code with mutants, in a
/// run of every mutant: as the Java edition stops such a run, since scored it would be every
/// mutant not covered, which measures nothing.
fn nothing_covered(project: &Project, tests: usize, functions: usize) -> Failure {
    Code::NothingCovered.with(format!(
        "{} ran and passed in {}, but {} the {}, so a score would measure nothing: every mutant \
         would read as reached by no test. Write a #[test] that calls this code",
        if tests == 1 {
            "1 test".to_string()
        } else {
            format!("{tests} tests")
        },
        project.package,
        if tests == 1 {
            "it reached none of"
        } else {
            "none of them reached any of"
        },
        if functions == 1 {
            "only function with mutants".to_string()
        } else {
            format!("{functions} functions with mutants")
        }
    ))
}

/// The command that runs one test alone, as Cargo runs it: its target, then its exact name.
fn rerun(project: &Project, binary: &crate::compile::Binary, name: &str) -> String {
    let target = match binary.kind.as_str() {
        "lib" => "--lib".to_string(),
        kind => format!("--{kind} {}", binary.target),
    };
    project.cargo_command("test", &format!("{target} -- --exact {name}"))
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

/// What a mutant should take, and when it's slow: the time its covering tests took in the coverage
/// pass, and three times that or half its timeout, whichever comes first. One no test reaches, or
/// that isn't run, costs nothing.
pub(crate) struct Cost {
    expected: Duration,
    slow: Duration,
    tests: usize,
}

fn cost(
    found: &Crate,
    index: usize,
    site: u32,
    built: &compile::Built,
    tests: &[Test],
    reach: &[Reach],
) -> Cost {
    let none = Cost {
        expected: Duration::ZERO,
        slow: Duration::ZERO,
        tests: 0,
    };
    if found.mutants[index].skipped.is_some() || built.unviable.contains_key(&index) {
        return none;
    }
    let (by_binary, covering) = covering_time(site, tests, reach);
    let expected: Duration = by_binary.values().sum();
    let timeout: Duration = by_binary.values().map(|&baseline| timeout(baseline)).sum();
    if by_binary.is_empty() {
        return none;
    }
    Cost {
        expected,
        slow: (expected * 3).min(timeout / 2),
        tests: covering,
    }
}

/// What the tests that reach `site` took in the coverage pass, by binary, and how many there are.
fn covering_time(site: u32, tests: &[Test], reach: &[Reach]) -> (BTreeMap<usize, Duration>, usize) {
    let mut by_binary: BTreeMap<usize, Duration> = BTreeMap::new();
    let mut covering = 0;
    for (t, test) in tests.iter().enumerate() {
        if reach[t].hits.contains(&site) {
            *by_binary.entry(test.binary).or_default() += reach[t].took;
            covering += 1;
        }
    }
    (by_binary, covering)
}

/// Each site the coverage pass measured ([`crate::test_times::site`]), with what a mutant there
/// is expected to take: the time of the tests that reach it, as [`cost`] has it, zero when none
/// does. Only a site with a switch built is measured; with `every`, the build had every mutant
/// there is, so a site whose every mutant didn't compile is measured too, at zero, as no mutant
/// there runs.
fn measured(
    found: &Crate,
    built_set: &BTreeSet<usize>,
    built: &compile::Built,
    tests: &[Test],
    reach: &[Reach],
    every: bool,
) -> BTreeMap<String, Duration> {
    let first = first_at_site(found, built_set, built);
    let mut sites = BTreeMap::new();
    for &index in built_set {
        let mutant = &found.mutants[index];
        let site = crate::test_times::site(mutant);
        if !built.unviable.contains_key(&index) {
            let (by_binary, _) = covering_time(first[index], tests, reach);
            sites.insert(site, by_binary.values().sum());
        } else if every {
            sites.entry(site).or_insert(Duration::ZERO);
        }
    }
    sites
}

/// How long a binary's covering tests may run against a mutant: five times what they took, and a
/// second, or ten seconds.
fn timeout(baseline: Duration) -> Duration {
    (baseline * 5 + Duration::from_secs(1)).max(Duration::from_secs(10))
}

/// What a mutant runs against: the build, the tests the coverage pass kept with what each reaches,
/// where they run, and how a catch is taken.
struct Suite<'a> {
    built: &'a compile::Built,
    tests: &'a [Test],
    reach: &'a [Reach],
    cwd: &'a std::path::Path,
    catches: Catches,
}

fn result(found: &Crate, index: usize, site: u32, suite: &Suite) -> MutantResult {
    let Suite {
        built,
        tests,
        reach,
        cwd,
        catches,
    } = *suite;
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
            killed_by_all: Vec::new(),
            unconfirmed: None,
            failure_origin: None,
            false_catch_by: None,
            tests_run: 0,
            took: Duration::ZERO,
            inputs: BTreeMap::new(),
            suggestion: None,
            by_assertion: false,
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
        killed_by_all: Vec::new(),
        unconfirmed: None,
        failure_origin: None,
        false_catch_by: None,
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
        by_assertion: false,
    };
    listed(&function.module, &mut result.covered_by);
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
    // A binary at a time, the library's first, then by target, so the test that catches a mutant
    // is the same in every run; within one, the covering tests run together.
    let mut by_binary: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &t in &covering {
        by_binary.entry(tests[t].binary).or_default().push(t);
    }
    let mut groups: Vec<(Duration, usize, Vec<usize>)> = by_binary
        .into_iter()
        .map(|(binary, group)| (group.iter().map(|&t| reach[t].took).sum(), binary, group))
        .collect();
    groups.sort_by_key(|(_, binary, _)| {
        let built = &built.binaries[*binary];
        (
            built.kind != "lib",
            built.target.clone(),
            built.kind.clone(),
        )
    });
    let started = Instant::now();
    let active = [("MUTASTIC_ACTIVE", mutant.ordinal.to_string())];
    let named = |group: &[usize]| {
        let mut names: Vec<&str> = group.iter().map(|&t| tests[t].name.as_str()).collect();
        // Near unsafe code, one test at a time: a mutant that corrupts memory mustn't make a
        // different test fail and take the credit.
        if mutant.near_unsafe.is_some() {
            names.push("--test-threads=1");
        }
        names
    };
    // Of a group's tests, those among `failures`, as they're listed, not as they finished: the
    // threads finish in a different order each run.
    let failing = |group: &[usize], failures: &[String]| -> Vec<&Test> {
        group
            .iter()
            .map(|&t| &tests[t])
            .filter(|test| failures.contains(&test.name))
            .collect()
    };
    let mut groups = groups.into_iter();
    for (baseline, binary, group) in groups.by_ref() {
        let mut names = named(&group);
        let timeout = timeout(baseline);
        let run =
            |names: &[&str]| testrun::run(&built.binaries[binary], cwd, names, &active, timeout);
        let mut outcome: Outcome = run(&names);
        result.tests_run += group.len();
        // Code a test ran exited, status 0: `cargo test` passes, so that test didn't catch it, and
        // the tests that never ran after it run without it.
        while outcome.exited == Some(0) {
            let Some(exit) = exiting(&run, &names) else {
                break;
            };
            names.retain(|name| *name != exit.as_str());
            outcome = if names.iter().any(|name| !name.starts_with("--")) {
                run(&names)
            } else {
                Outcome::passed()
            };
        }
        if outcome.timed_out {
            result.status = Status::Timeout;
            break;
        }
        if outcome.passed {
            continue;
        }
        if let Some(code) = outcome.exited {
            // Code a test ran exited with a status that fails `cargo test`: that test saw the
            // mutant end the process. Not a crash, and its test is the one that was running.
            if let Some(exit) = exiting(&run, &names) {
                result.status = Status::Killed;
                result.killed_with = Some(format!("exit: {code}"));
                result.killed_by = group
                    .iter()
                    .find(|&&t| tests[t].name == exit)
                    .map(|&t| tests[t].display.clone())
                    .or(Some(exit));
                break;
            }
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
        let failed = failing(&group, &outcome.failures);
        // The first that failed catches it; with catches confirmed, the first whose failure
        // repeats when it runs again on its own, and that run's output is its alone.
        let mut first = 0;
        let mut alone = None;
        if catches.confirm && !failed.is_empty() {
            match repeated(&run, &failed, &mut result.unconfirmed) {
                Some((at, again)) => {
                    first = at;
                    alone = Some(again).filter(|again| again.failed.is_some());
                }
                // No catch repeated, and the group's other tests passed: the next group decides.
                None => continue,
            }
        }
        let output = alone
            .as_ref()
            .map_or(&outcome.stdout, |again| &again.stdout);
        result.status = Status::Killed;
        result.by_assertion = testrun::assertion_failed(output);
        result.killed_with = Some(match testrun::language_panic(output) {
            Some(message) => format!("panic: {message}"),
            None => "panic".into(),
        });
        let catcher = failed.get(first).copied();
        let name = catcher.map(|test| &test.name).or(outcome.failed.as_ref());
        // A panic names its file from the package, or from the workspace above it.
        let roots: Vec<&std::path::Path> = cwd.ancestors().collect();
        result.failure_origin =
            name.map(|name| crate::origin::of(output, name, &roots).to_string());
        result.killed_by = catcher
            .map(|test| test.display.clone())
            .or_else(|| outcome.failed.clone());
        if catches.every {
            result.killed_by_all = failed[first.min(failed.len())..]
                .iter()
                .map(|test| test.display.clone())
                .collect();
        }
        break;
    }
    // Every test that catches it, not only the first: the binaries after the one that caught it
    // run too, until one doesn't finish. Only a test's catch is added to: a crash names none.
    let by_a_test = result.status == Status::Killed && result.killed_by.is_some();
    if catches.every && by_a_test {
        if result.killed_by_all.is_empty() {
            result.killed_by_all.extend(result.killed_by.clone());
        }
        for (baseline, binary, group) in groups {
            let binary = &built.binaries[binary];
            let outcome = testrun::run(binary, cwd, &named(&group), &active, timeout(baseline));
            let failed = failing(&group, &outcome.failures);
            if failed.is_empty() && !outcome.passed {
                break;
            }
            result
                .killed_by_all
                .extend(failed.iter().map(|test| test.display.clone()));
        }
    }
    result.took = started.elapsed();
    result
}

/// The first of `failed` whose failure repeats when it runs again on its own against the mutant
/// (`--confirm-catches`): its place among them, and that run. A test that passes this time didn't
/// catch the mutant: it's `unconfirmed`, the last such kept, as the Java edition keeps it, and the
/// next one that failed is tried. A run that fails any other way, or runs out of time, confirms.
fn repeated(
    run: &impl Fn(&[&str]) -> Outcome,
    failed: &[&Test],
    unconfirmed: &mut Option<String>,
) -> Option<(usize, Outcome)> {
    for (at, test) in failed.iter().enumerate() {
        let again = run(&[test.name.as_str()]);
        if !again.passed {
            return Some((at, again));
        }
        *unconfirmed = Some(test.display.clone());
    }
    None
}

/// The false-catch guard's runs ([`crate::guard`]), on the run's build and the tests the coverage
/// pass kept.
struct BuildRuns<'a> {
    found: &'a Crate,
    /// What looks for a test to suggest for a mutant the reruns find missed.
    suggester: Option<&'a Suggester<'a>>,
    /// For each mutant, the first live mutant at its site: what the coverage pass recorded.
    first: &'a [u32],
    built: &'a compile::Built,
    tests: &'a [Test],
    reach: &'a [Reach],
    cwd: &'a std::path::Path,
    workers: usize,
    catches: Catches,
}

impl BuildRuns<'_> {
    fn test(&self, display: &str) -> Option<usize> {
        self.tests.iter().position(|test| test.display == display)
    }
}

impl crate::guard::Runs for BuildRuns<'_> {
    fn time_of(&self, test: &str) -> Duration {
        self.test(test)
            .map_or(Duration::ZERO, |index| self.reach[index].took)
    }

    /// On its own, with no mutant active, under the time limit a mutant's run of it has. A process
    /// that couldn't start, or ran out of time, isn't evidence; one that failed or crashed is.
    fn fails_alone(&self, display: &str) -> bool {
        let Some(index) = self.test(display) else {
            return false;
        };
        let test = &self.tests[index];
        let limit = timeout(self.reach[index].took);
        let outcome = testrun::run(
            &self.built.binaries[test.binary],
            self.cwd,
            &[&test.name],
            &[],
            limit,
        );
        let ran = outcome.failed.is_some() || outcome.failure.as_deref() == Some("crash");
        !outcome.passed && !outcome.timed_out && ran
    }

    fn run_again(
        &self,
        results: &[MutantResult],
        indexes: &[usize],
        left_out: &BTreeSet<String>,
    ) -> Vec<MutantResult> {
        let (tests, reach): (Vec<Test>, Vec<Reach>) = self
            .tests
            .iter()
            .zip(self.reach)
            .filter(|(test, _)| !left_out.contains(&test.display))
            .map(|(test, reach)| (test.clone(), reach.clone()))
            .unzip();
        let mutants: Vec<usize> = indexes.iter().map(|&at| results[at].mutant).collect();
        let suite = Suite {
            built: self.built,
            tests: &tests,
            reach: &reach,
            cwd: self.cwd,
            catches: self.catches,
        };
        testrun::parallel(&mutants, self.workers, |&mutant| {
            let again = result(self.found, mutant, self.first[mutant], &suite);
            Suggester::with_suggestion(self.suggester, again)
        })
    }
}

/// What looks for a test that catches a missed mutant whose function has a probe, as the mutant
/// finishes: by running the probe on candidate inputs, as the code is and with the mutant in.
struct Suggester<'a> {
    found: &'a Crate,
    built: &'a compile::Built,
    /// The probes in the build, by libtest's name, with the binary each is in.
    probes: BTreeMap<String, usize>,
    /// Where the probes write what they found.
    dir: PathBuf,
    cwd: PathBuf,
}

impl<'a> Suggester<'a> {
    /// `None` when the run doesn't look, or no function has a probe.
    fn new(
        project: &Project,
        found: &'a Crate,
        built: &'a compile::Built,
        options: &Options,
    ) -> Option<Suggester<'a>> {
        if !options.suggestions || built.probes.is_empty() {
            return None;
        }
        let cwd = project.copy_dir().join(&project.package_dir);
        let dir = project.mutastic_dir().join("probes");
        std::fs::create_dir_all(&dir).ok()?;
        Some(Suggester {
            found,
            built,
            probes: testrun::probes(&built.binaries, &cwd),
            dir,
            cwd,
        })
    }

    /// A test that would catch `result`'s mutant, when it's missed and its function has a probe;
    /// and for a mutant no test reaches, the call to start its function's first test with, found
    /// the same way, as the Java edition finds a starting point.
    fn suggest(&self, result: &MutantResult) -> Option<crate::suggest::Suggestion> {
        if !matches!(result.status, Status::Survived | Status::NoCoverage) {
            return None;
        }
        let found = self.found;
        let mutant = &found.mutants[result.mutant];
        if !self.built.probes.contains(&mutant.function) {
            return None;
        }
        let function = &found.functions[mutant.function];
        // No starting point is offered for the crate's own function, so none is looked for.
        if result.status == Status::NoCoverage && !function.public {
            return None;
        }
        let callable = function.callable.as_ref()?;
        let name = callable.probe_name(function.ordinal);
        let &binary = self.probes.get(&name)?;
        let line = source_line(found, result.mutant);
        let text = found
            .files
            .iter()
            .find(|file| file.path == mutant.file)
            .map_or("", |file| file.text.as_str());
        let mut constants = crate::suggest::constants(&line);
        constants.extend(crate::suggest::named_constants(&line, text));
        // First with the receiver made as a covering test makes it, when one does.
        let made = crate::suggest::built(callable, &result.covered_by);
        let mut candidates = made.as_ref().map_or_else(Vec::new, |made| {
            crate::suggest::candidates_made(
                &callable.types(),
                &constants,
                &result.inputs,
                &made.values,
            )
        });
        candidates.extend(crate::suggest::candidates(
            &callable.types(),
            &constants,
            &result.inputs,
        ));
        let probe = crate::suggest::Probe {
            binary: &self.built.binaries[binary],
            cwd: &self.cwd,
            name: &name,
        };
        crate::suggest::find(&probe, &self.dir, mutant.ordinal, &candidates).map(|mut found| {
            found.built_by = made
                .filter(|made| found.values.starts_with(&made.values))
                .map(|made| made.test);
            found
        })
    }

    /// `result` with the test that would catch it, when there's one to find.
    fn with_suggestion(suggester: Option<&Self>, mut result: MutantResult) -> MutantResult {
        if let Some(suggester) = suggester {
            result.suggestion = suggester.suggest(&result);
        }
        result
    }
}

/// The test whose code exits the process, found by running `names` again one at a time: `None`
/// if, run that way, none does.
fn exiting(run: &impl Fn(&[&str]) -> Outcome, names: &[&str]) -> Option<String> {
    let mut one_at_a_time = names.to_vec();
    if !one_at_a_time.contains(&"--test-threads=1") {
        one_at_a_time.push("--test-threads=1");
    }
    let again = run(&one_at_a_time);
    again.exited.and_then(|_| testrun::running(&again.stdout))
}

/// What a crash means. A safe mutant that crashes the process was caught, but by the crash, not a
/// test's check, so no test is named. Near unsafe code a crash may be undefined behaviour, and a
/// crash in std's own check of an unsafe precondition is std's catch: neither is in the score.
/// Whether `result` is a catch by a test that a signal from outside the run ended: a hang-up,
/// Ctrl-C or a kill (SIGHUP, SIGINT, SIGTERM), which `timeout` and Ctrl-C send the run's tests
/// with the run. That's the run being stopped, not the mutant caught, so what's kept of a stopped
/// run leaves it out; the run's own result, if it gets one, judges it as ever.
pub fn stopped_from_outside(result: &MutantResult) -> bool {
    result
        .killed_with
        .as_deref()
        .is_some_and(ended_from_outside)
}

/// Whether a catch's `killed_with`, `crash: signal 15`, is a signal from outside the run's.
fn ended_from_outside(with: &str) -> bool {
    with.starts_with("crash:")
        && ["signal 1", "signal 2", "signal 15"].iter().any(|signal| {
            with.match_indices(signal)
                .any(|(at, _)| !with[at + signal.len()..].starts_with(|c: char| c.is_ascii_digit()))
        })
}

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

/// The covering tests in the order the cards list them, as the Java edition lists them: the
/// mutant's own test module's first (`pricing::tests::…` for a function in `pricing`), then by
/// name.
fn listed(module: &str, tests: &mut [String]) {
    let own = if module.is_empty() {
        "tests::".to_string()
    } else {
        format!("{module}::tests::")
    };
    tests.sort_by(|a, b| (!a.starts_with(&own), a).cmp(&(!b.starts_with(&own), b)));
}

/// `id` names no mutant of the package: one a digit or a swap away, if there is one, with the
/// line corrected to it; else the command that lists them all, without running anything.
pub fn not_a_mutant(id: &str, package: &str, dir: &str, found: &Crate) -> Failure {
    let ids = found.mutants.iter().map(|mutant| mutant.id.as_str());
    match crate::similar::closest_within(id, ids, 1) {
        Some(close) => Code::UnknownId
            .with(format!(
                "{id} isn't a mutant of {package}. There's one called {close}."
            ))
            .correcting(id, close),
        None => Code::UnknownId.with(format!(
            "{id} isn't a mutant of {package}. To see its mutants:\n  mutastic mutants {}",
            crate::report::shell_word(dir)
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test a hang-up, Ctrl-C or a kill ended stopped with the run; any other crash, and a signal
    /// whose number only starts the same, is the mutant's.
    #[test]
    fn a_test_stopped_from_outside_is_told_from_a_crash() {
        for stopped in ["crash: signal 15", "crash: signal 2", "crash: signal 1"] {
            assert!(ended_from_outside(stopped), "{stopped}");
        }
        for crash in [
            "crash: SIGSEGV",
            "crash: signal 16",
            "crash: signal 13",
            "exit: 15",
            "panic",
        ] {
            assert!(!ended_from_outside(crash), "{crash}");
        }
    }

    /// `--exclude-tests` takes a test by the name a run shows, or a module of tests, whole: never
    /// part of a name.
    #[test]
    fn a_test_is_left_out_by_its_name_or_a_module_above_it() {
        let left_out = LeftOut::listed(
            &[crate::config::ExcludedTest {
                test: "pricing::tests".to_string(),
                reason: "shares a port".to_string(),
            }],
            &[
                "orders::rounds_up".to_string(),
                "pricing::tests".to_string(),
            ],
        );
        assert_eq!(
            LeftOut::names(&left_out),
            ["pricing::tests", "orders::rounds_up"],
            "the file's reason is kept for a test given twice"
        );
        assert_eq!(left_out[0].reason.as_deref(), Some("shares a port"));
        assert_eq!(left_out[1].reason, None);
        assert!(leaves_out(&left_out, "pricing::tests::big_orders"));
        assert!(leaves_out(&left_out, "pricing::tests::edge::at_a_hundred"));
        assert!(leaves_out(&left_out, "orders::rounds_up"));
        assert!(!leaves_out(&left_out, "pricing::tests_of_shipping::free"));
        assert!(!leaves_out(&left_out, "orders::rounds_up_twice"));
        assert!(!leaves_out(&left_out, "orders::rounds_down"));
        assert!(!leaves_out(&left_out, "tests::big_orders"));
        assert!(!leaves_out(&[], "pricing::tests::big_orders"));
    }

    #[test]
    fn covering_tests_are_listed_own_module_first_then_by_name() {
        let mut tests = vec![
            "window::tests::b".to_string(),
            "pricing::tests::z".to_string(),
            "integration::a".to_string(),
            "pricing::tests::a".to_string(),
        ];
        listed("pricing", &mut tests);
        assert_eq!(
            tests,
            [
                "pricing::tests::a",
                "pricing::tests::z",
                "integration::a",
                "window::tests::b"
            ]
        );
        let mut root = vec!["other::tests::a".to_string(), "tests::b".to_string()];
        listed("", &mut root);
        assert_eq!(root, ["tests::b", "other::tests::a"]);
    }

    /// A `mutate:` rule takes back an arid skip, and only that: a `skip:` rule still skips.
    #[test]
    fn a_mutate_rule_takes_back_an_arid_skip() {
        let dir = std::env::temp_dir().join(format!("mutastic-rules-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let source = "pub fn told(b: i32) { eprintln!(\"{b}\"); }\n\
                      pub fn quiet(b: i32) { eprintln!(\"{b}\"); }\n";
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let found = sites::find(
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
            ..crate::config::Rule::default()
        };
        let only = |target: &str, id: &str| crate::config::Rule {
            arid: vec![id.to_string()],
            ..rule(true, target)
        };
        let skipped = |rules: &[crate::config::Rule]| {
            let mut found = found.clone();
            apply_rules(rules, "shop", &mut found);
            found
                .mutants
                .iter()
                .map(|m| m.skipped.clone())
                .collect::<Vec<_>>()
        };
        let output = Some("arid: only writes output".to_string());
        assert_eq!(
            skipped(&[rule(true, "shop::told"), rule(false, "quiet")]),
            [None, output.clone()]
        );
        // An `arid:` list takes back only its rules' skips.
        assert_eq!(
            skipped(&[
                only("told", "mutastic:output"),
                only("quiet", "mutastic:null-checks")
            ]),
            [None, output.clone()]
        );
        // Two rules over the same code take back what either names.
        assert_eq!(
            skipped(&[
                only("quiet", "mutastic:null-checks"),
                only("quiet", "mutastic:output")
            ]),
            [output, None]
        );
    }

    /// A value a `let` works out only for a log line comes back with `mutastic:output`, the rule it
    /// belongs to, and with no list; another rule's list leaves it skipped.
    #[test]
    fn a_mutate_rule_takes_back_a_value_only_logged() {
        let dir =
            std::env::temp_dir().join(format!("mutastic-logged-rules-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let source =
            "pub fn timed(a: i64, b: i64) {\n    let d = a - b;\n    log::info!(\"{d}\");\n}\n";
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let found = sites::find(&dir, "shop", &[PathBuf::from("src/lib.rs")], &[sites::MATH]);
        std::fs::remove_dir_all(&dir).ok();
        let rule = |arid: &[&str]| crate::config::Rule {
            mutate: true,
            target: "shop::timed".into(),
            reason: "the log is what the operator reads".into(),
            arid: arid.iter().map(|id| id.to_string()).collect(),
            ..crate::config::Rule::default()
        };
        let skipped = |rules: &[crate::config::Rule]| {
            let mut found = found.clone();
            apply_rules(rules, "shop", &mut found);
            found
                .mutants
                .iter()
                .map(|m| m.skipped.clone())
                .collect::<Vec<_>>()
        };
        let logged = || vec![Some(crate::arid::ONLY_FEEDS_A_LOG_LINE.to_string())];
        assert_eq!(skipped(&[]), logged());
        assert_eq!(skipped(&[rule(&[])]), [None]);
        assert_eq!(skipped(&[rule(&["mutastic:output"])]), [None]);
        assert_eq!(skipped(&[rule(&["mutastic:null-checks"])]), logged());
    }

    /// An `output:` rule makes the statements that call what it names arid, with its reason, by
    /// the path as written; other calls stay counted, and a `mutate:` rule takes them back.
    #[test]
    fn an_output_rule_skips_the_calls_it_names() {
        let dir = std::env::temp_dir().join(format!("mutastic-own-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let source = "mod audit { pub fn record(_b: i32) {} pub fn keep(_b: i32) {} }\n\
                      use audit::record;\n\
                      pub fn told(b: i32) {\n\
                      audit::record(b);\n\
                      record(b + 1);\n\
                      crate::audit::record(b);\n\
                      audit::keep(b);\n\
                      }\n";
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let found = sites::find(
            &dir,
            "shop",
            &[PathBuf::from("src/lib.rs")],
            &[sites::REMOVE_VOID_CALL],
        );
        std::fs::remove_dir_all(&dir).ok();
        let own = crate::config::Rule {
            output: true,
            target: "crate::audit::record".into(),
            reason: "the audit trail".into(),
            ..crate::config::Rule::default()
        };
        let skipped = |rules: &[crate::config::Rule]| {
            let mut found = found.clone();
            apply_rules(rules, "shop", &mut found);
            found
                .mutants
                .iter()
                .map(|m| m.skipped.clone().unwrap_or_default())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            skipped(std::slice::from_ref(&own)),
            [
                "arid: only calls audit::record (the audit trail)",
                "arid: only calls record (the audit trail)",
                "arid: only calls crate::audit::record (the audit trail)",
                "",
            ]
        );
        assert_eq!(skipped(&[]), ["", "", "", ""]);
        let kept = crate::config::Rule {
            mutate: true,
            target: "told".into(),
            reason: "the audit is checked".into(),
            arid: vec!["mutastic:output".into()],
            ..crate::config::Rule::default()
        };
        assert_eq!(skipped(&[own, kept]), ["", "", "", ""]);
    }
}
