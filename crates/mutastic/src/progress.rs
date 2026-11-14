//! What a run says while it runs, on stderr, so stdout stays the result: a line for every phase,
//! a live block redrawn in place on a terminal, and a heartbeat line everywhere else, so a run is
//! never silent. `PROGRESS.md` at the repository's root is the contract, shared with the Java
//! edition word for word.

mod frames;

use std::io::{IsTerminal as _, Write as _};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::engine::{self, Status};
use crate::report::{self, Style};
use crate::terminal;

pub use frames::{count, Counts, Phase, View, Worker};

/// How progress is shown: `bar`, a live block redrawn in place on a terminal; `counter`, a plain
/// line at each phase and a heartbeat between, for logs, CI and agents; or `none`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shown {
    Bar,
    Counter,
    None,
}

impl Shown {
    /// `auto` as `Some(None)`: the choice is left to [`resolve`].
    pub fn parse(value: &str) -> Option<Option<Shown>> {
        match value {
            "auto" => Some(None),
            "bar" => Some(Some(Shown::Bar)),
            "counter" => Some(Some(Shown::Counter)),
            "none" => Some(Some(Shown::None)),
            _ => None,
        }
    }
}

/// Set by a CI service; `CI` itself is read apart, as some set it to `false`.
const CI: &[&str] = &[
    "TF_BUILD",
    "GITHUB_ACTIONS",
    "GITLAB_CI",
    "BUILDKITE",
    "JENKINS_URL",
    "TEAMCITY_VERSION",
    "CIRCLECI",
];

/// Set by a coding agent's harness, as std-env reads them.
const AGENTS: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE",
    "GEMINI_CLI",
    "CODEX_SANDBOX",
    "CODEX_THREAD_ID",
    "OPENCODE",
    "COPILOT_AGENT",
    "COPILOT_CLI",
    "CURSOR_AGENT",
    "REPL_ID",
    "JUNIE_DATA",
    "GOOSE_PROVIDER",
    "AUGMENT_AGENT",
];

/// Whether a CI service runs this: some pretend to be a terminal, and a redrawn line in a log is
/// noise.
pub fn in_ci() -> bool {
    std::env::var("CI").is_ok_and(|value| !matches!(value.as_str(), "" | "false" | "0"))
        || CI.iter().any(|name| std::env::var_os(name).is_some())
}

/// Whether a coding agent runs this: it reads the output as text, after the fact, and a harness
/// stops a process that's quiet for too long. It changes how progress is shown, never stdout.
pub fn under_agent() -> bool {
    std::env::var_os("MUTASTIC_AGENT").is_some_and(|value| value == "1")
        || AGENTS.iter().any(|name| std::env::var_os(name).is_some())
}

/// `--progress`, else `MUTASTIC_PROGRESS`, else a live block on a terminal that isn't `dumb`,
/// isn't CI's and isn't an agent's, and heartbeat lines anywhere else.
pub fn resolve(asked: Option<Shown>) -> Shown {
    asked
        .or_else(|| {
            std::env::var("MUTASTIC_PROGRESS")
                .ok()
                .and_then(|value| Shown::parse(&value))
                .flatten()
        })
        .unwrap_or_else(|| {
            let dumb = std::env::var("TERM").is_ok_and(|term| term == "dumb");
            if std::io::stderr().is_terminal() && !dumb && !in_ci() && !under_agent() {
                Shown::Bar
            } else {
                Shown::Counter
            }
        })
}

/// How long a log may go without a line: 30 seconds, or `MUTASTIC_HEARTBEAT` seconds.
pub fn heartbeat() -> Duration {
    std::env::var("MUTASTIC_HEARTBEAT")
        .ok()
        .and_then(|value| seconds(&value))
        .unwrap_or(Duration::from_secs(30))
}

/// `value` as a number of seconds above 0, as `MUTASTIC_HEARTBEAT` takes it: `30`, `0.5`.
pub fn seconds(value: &str) -> Option<Duration> {
    value
        .parse::<f64>()
        .ok()
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map(Duration::from_secs_f64)
}

impl Counts {
    fn add(&mut self, status: Status) {
        self.done += 1;
        match status {
            Status::Killed => self.caught += 1,
            Status::Survived => self.missed += 1,
            Status::NoCoverage => self.not_covered += 1,
            Status::Timeout => self.timed_out += 1,
            Status::Ignored | Status::RuntimeError => self.unscored += 1,
        }
    }
}

/// A test, or a mutant, a worker has started.
struct Running {
    doing: String,
    id: String,
    started: Instant,
    expected: Duration,
    /// Past this it's SLOW.
    slow: Duration,
    tests: usize,
}

/// A test in the coverage pass is SLOW after a minute: there's nothing to measure it against yet.
const SLOW_TEST: Duration = Duration::from_secs(60);

/// Nothing is SLOW before two seconds: a mutant whose tests take milliseconds would otherwise be
/// marked for being a little slower than that, and the mark would flash on every line.
const SLOW_FLOOR: Duration = Duration::from_secs(2);

struct State {
    phase: Phase,
    since: Instant,
    counts: Counts,
    /// What each worker is running.
    workers: Vec<Option<Running>>,
    /// What the mutants should take, all told, and what those finished were expected to.
    cost: Duration,
    cost_done: Duration,
    /// Lines of the live block on screen now.
    drawn: usize,
    /// When the last progress line was written, for the heartbeat.
    printed: Instant,
    frame: usize,
    stopped: bool,
    package: String,
    /// Whether the score has fallen below the threshold: the tab stays red from then.
    red: bool,
    /// The live events on stdout, for `--format=jsonl`, once asked for.
    events: Option<Events>,
    /// The phase the events last said started, and when.
    event_phase: Option<(&'static str, Instant)>,
    /// Whether `run-started` has gone out: `progress` events wait for it.
    started_sent: bool,
    /// When the last `progress` event went out.
    evented: Instant,
}

/// What `run-started` says besides the build's profile, for the live events: how unsafe code is
/// treated, and the share, for `--shard`.
pub struct Events {
    pub unsafe_mode: crate::sites::UnsafeMode,
    pub share: Option<serde_json::Value>,
}

struct Inner {
    shown: Shown,
    style: Style,
    /// Whether each source file's line is printed as its last mutant finishes.
    files: bool,
    workers: usize,
    heartbeat: Duration,
    /// Whether `t` asks what's running: on a terminal that's also stdin.
    keys: bool,
    /// Whether the tab shows progress (OSC 9;4): on a terminal that draws it.
    tab: bool,
    /// The score below which the tab turns red; none when the threshold doesn't decide.
    threshold: Option<f64>,
    state: Mutex<State>,
    wake: Condvar,
}

/// The run's progress on stderr, with a ticker thread that moves the clock on a terminal and
/// writes the heartbeat elsewhere, so nothing waits on the next event to say something.
pub struct Progress {
    inner: Arc<Inner>,
    ticker: Mutex<Option<std::thread::JoinHandle<()>>>,
    keys: Mutex<Option<terminal::Keys>>,
    guard: Mutex<Option<terminal::Guard>>,
}

impl Progress {
    /// Starts the run's progress: its first line, before anything slow, and the ticker. Below
    /// `threshold`, the tab's progress turns red.
    pub fn start(
        shown: Shown,
        style: Style,
        (files, workers): (bool, usize),
        threshold: Option<f64>,
    ) -> Progress {
        let now = Instant::now();
        let guard = (shown == Shown::Bar).then(terminal::Guard::take);
        let keys = (shown == Shown::Bar).then(terminal::Keys::listen);
        let tab = shown == Shown::Bar && terminal::tab_progress(|name| std::env::var(name).ok());
        let inner = Arc::new(Inner {
            shown,
            style,
            files,
            workers,
            heartbeat: heartbeat(),
            keys: keys.as_ref().is_some_and(terminal::Keys::listening),
            tab,
            threshold,
            state: Mutex::new(State {
                phase: Phase::Reading,
                since: now,
                counts: Counts::default(),
                workers: (0..workers).map(|_| None).collect(),
                cost: Duration::ZERO,
                cost_done: Duration::ZERO,
                drawn: 0,
                printed: now,
                frame: 0,
                stopped: false,
                package: String::new(),
                red: false,
                events: None,
                event_phase: None,
                started_sent: false,
                evented: now,
            }),
            wake: Condvar::new(),
        });
        let progress = Progress {
            inner: Arc::clone(&inner),
            ticker: Mutex::new(None),
            keys: Mutex::new(keys),
            guard: Mutex::new(guard),
        };
        let ticking = Arc::clone(&inner);
        if let Ok(mut ticker) = progress.ticker.lock() {
            *ticker = Some(std::thread::spawn(move || ticking.tick()));
        }
        if shown == Shown::None {
            return progress;
        }
        terminal::listen_for_signals();
        progress.line(&format!(
            "mutastic {} · reading the Cargo package…",
            env!("CARGO_PKG_VERSION")
        ));
        progress
    }

    /// Writes the run's phases and progress as events on stdout as they happen, for
    /// `--format=jsonl`: `run-started` as soon as the build is done, and before it, phases only.
    pub fn stream(&self, events: Events) {
        if let Some(mut state) = self.inner.lock() {
            let started = state.since;
            state.events = Some(events);
            self.inner.phase_event(&mut state, Some("reading"));
            if let Some((_, at)) = state.event_phase.as_mut() {
                *at = started;
            }
        }
    }

    /// Whether `run-started` went out live, so the result doesn't say it again.
    pub fn streamed(&self) -> bool {
        self.inner.lock().is_some_and(|state| state.started_sent)
    }

    /// The package whose files the file lines name, once it's read.
    pub fn package(&self, package: &str) {
        if let Some(mut state) = self.inner.lock() {
            state.package = package.to_string();
        }
    }

    /// A permanent line, above the live block.
    fn line(&self, text: &str) {
        if let Some(mut state) = self.inner.lock() {
            self.inner.write(&mut state, text);
        }
    }

    pub fn step(&self, step: engine::Progress) {
        let Some(mut state) = self.inner.lock() else {
            return;
        };
        self.inner.take(&mut state, step);
    }

    /// Stops the ticker, clears the live block before the result, and gives the terminal its
    /// input back.
    pub fn finished(&self) {
        if let Some(mut state) = self.inner.lock() {
            state.stopped = true;
            let tab = if self.inner.tab {
                terminal::TAB_CLEARED
            } else {
                ""
            };
            self.inner
                .send(&format!("{}{tab}", self.inner.clear(&mut state)));
            terminal::tab_shown(false);
            self.inner.wake.notify_all();
        }
        let ticker = self.ticker.lock().ok().and_then(|mut ticker| ticker.take());
        if let Some(ticker) = ticker {
            let _ = ticker.join();
        }
        if let Ok(mut keys) = self.keys.lock() {
            keys.take();
        }
        if let Ok(mut guard) = self.guard.lock() {
            guard.take();
        }
    }
}

impl Drop for Progress {
    /// A run that stops with an error leaves no live block behind its message.
    fn drop(&mut self) {
        self.finished();
    }
}

impl Inner {
    fn lock(&self) -> Option<MutexGuard<'_, State>> {
        self.state.lock().ok()
    }

    fn enter(&self, state: &mut State, phase: Phase) {
        state.phase = phase;
        state.since = Instant::now();
        state.workers.iter_mut().for_each(|worker| *worker = None);
    }

    fn take(&self, state: &mut State, step: engine::Progress) {
        match step {
            engine::Progress::Building { mutants } => {
                self.phase_event(state, Some("building"));
                self.enter(
                    state,
                    Phase::Building {
                        step: "check".into(),
                        round: 1,
                        crates: 0,
                        last: String::new(),
                    },
                );
                let line = format!(
                    "{}{} in one build · {}",
                    frames::word("building"),
                    frames::count(mutants, "mutant"),
                    frames::count(self.workers, "worker")
                );
                self.write(state, &line);
            }
            engine::Progress::Compiling {
                step,
                round,
                crates,
                last,
            } => {
                state.phase = Phase::Building {
                    step: step.to_string(),
                    round,
                    crates,
                    last: last.to_string(),
                };
            }
            engine::Progress::Built {
                mutants,
                took,
                rounds,
                withdrawn,
                profile,
            } => {
                self.phase_event(state, None);
                if let Some(events) = &state.events {
                    let started = report::run_started(
                        &state.package,
                        profile,
                        events.unsafe_mode,
                        events.share.clone(),
                    );
                    self.event(&started);
                    state.started_sent = true;
                }
                self.phase_event(state, Some("coverage"));
                self.enter(state, Phase::Coverage { done: 0, total: 0 });
                self.write(state, &frames::built_line(mutants, took, rounds, withdrawn));
            }
            engine::Progress::TestStarted { worker, name } => {
                if let Some(slot) = state.workers.get_mut(worker) {
                    *slot = Some(Running {
                        doing: name.to_string(),
                        id: String::new(),
                        started: Instant::now(),
                        expected: Duration::ZERO,
                        slow: SLOW_TEST,
                        tests: 1,
                    });
                }
            }
            engine::Progress::Tested { done, total } => {
                state.phase = Phase::Coverage { done, total };
            }
            engine::Progress::Covered {
                tests,
                rustc,
                took,
                mutants,
                flaky,
                cost,
            } => {
                state.counts.total = mutants;
                state.cost = cost;
                self.phase_event(state, Some("mutants"));
                self.enter(state, Phase::Mutants);
                let line = format!(
                    "{}{} on rustc {rustc} in {}{} · {}",
                    frames::word("coverage"),
                    frames::count(tests, "test"),
                    report::duration(took),
                    if flaky == 0 {
                        String::new()
                    } else {
                        format!(" ({flaky} flaky, left out)")
                    },
                    frames::count(mutants, "mutant")
                );
                self.write(state, &line);
            }
            engine::Progress::NothingToRun => {
                self.phase_event(state, None);
                self.enter(state, Phase::Idle);
            }
            engine::Progress::FromShares { summary, mutants } => {
                state.counts.total = mutants;
                self.enter(state, Phase::Idle);
                self.write(state, summary);
            }
            engine::Progress::MutantStarted {
                worker,
                found,
                mutant,
                expected,
                slow,
                tests,
            } => {
                let doing = report::running(found, &found.mutants[mutant], self.style);
                if let Some(slot) = state.workers.get_mut(worker) {
                    *slot = Some(Running {
                        doing,
                        id: found.mutants[mutant].id.clone(),
                        started: Instant::now(),
                        expected,
                        slow,
                        tests,
                    });
                }
            }
            engine::Progress::MutantFinished {
                result,
                found,
                project,
                worker,
                cost,
            } => {
                // Live, as the mutant finishes: before the phase it ran in ends (decision 7).
                if state.events.is_some() {
                    self.event(&report::mutant_finished(found, project, result));
                }
                state.counts.add(result.status);
                state.cost_done += cost;
                if let Some(slot) = state.workers.get_mut(worker) {
                    *slot = None;
                }
            }
            engine::Progress::FileFinished {
                found,
                results,
                took,
            } if self.files => {
                let line =
                    report::file_finished(&state.package, found, (results, took), self.style);
                self.write(state, line.trim_end_matches('\n'));
            }
            engine::Progress::FileFinished { .. } => {}
            engine::Progress::Ran { took } => {
                self.phase_event(state, None);
                self.enter(state, Phase::Idle);
                let line = frames::ran_line(&state.counts, took, self.style);
                self.write(state, &line);
            }
            engine::Progress::Checking { done, total } => {
                if !matches!(state.phase, Phase::Checking { .. }) {
                    state.since = Instant::now();
                    self.phase_event(state, Some("checking"));
                }
                state.phase = Phase::Checking { done, total };
            }
            engine::Progress::Checked {
                missed,
                took,
                found,
            } => {
                self.phase_event(state, None);
                self.enter(state, Phase::Idle);
                // As the Java edition says it: only when there's a miss to say it about.
                if missed > 0 {
                    self.write(state, &frames::checked_line(missed, took, found));
                }
            }
        }
    }

    /// One event on stdout, a line of its own.
    fn event(&self, event: &serde_json::Value) {
        let mut stdout = std::io::stdout().lock();
        let _ = writeln!(stdout, "{event}");
        let _ = stdout.flush();
    }

    /// The phase the events said started has finished; `next`, when given, has started.
    fn phase_event(&self, state: &mut State, next: Option<&'static str>) {
        if state.events.is_none() {
            return;
        }
        if let Some((phase, at)) = state.event_phase.take() {
            self.event(&serde_json::json!({
                "schema": report::SCHEMA,
                "edition": report::EDITION,
                "reason": "phase-finished",
                "phase": phase,
                "durationMs": u64::try_from(at.elapsed().as_millis()).unwrap_or(u64::MAX),
            }));
        }
        if let Some(phase) = next {
            self.event(&serde_json::json!({
                "schema": report::SCHEMA,
                "edition": report::EDITION,
                "reason": "phase-started",
                "phase": phase,
            }));
            state.event_phase = Some((phase, Instant::now()));
            state.evented = Instant::now();
        }
    }

    /// How far the phase under way is, as an event: what it knows, nothing it doesn't.
    fn progress_event(state: &State) -> Option<serde_json::Value> {
        let (phase, at) = state.event_phase?;
        let millis = |took: Duration| u64::try_from(took.as_millis()).unwrap_or(u64::MAX);
        let mut event = serde_json::json!({
            "schema": report::SCHEMA,
            "edition": report::EDITION,
            "reason": "progress",
            "phase": phase,
            "elapsedMs": millis(at.elapsed()),
        });
        match &state.phase {
            Phase::Building { round, crates, .. } => {
                event["round"] = (*round).into();
                event["crates"] = (*crates).into();
            }
            Phase::Coverage { done, total } | Phase::Checking { done, total } if *total > 0 => {
                event["done"] = (*done).into();
                event["total"] = (*total).into();
            }
            Phase::Mutants => {
                let counts = &state.counts;
                let (weighted, left) = Self::estimate(state);
                event["done"] = counts.done.into();
                event["total"] = counts.total.into();
                if let Some(weighted) = weighted {
                    event["weighted"] = ((weighted * 1000.0).round() / 1000.0).into();
                }
                if let Some(left) = left {
                    event["leftMs"] = millis(left).into();
                }
                event["caught"] = counts.caught.into();
                event["missed"] = counts.missed.into();
                event["notCovered"] = counts.not_covered.into();
                event["timedOut"] = counts.timed_out.into();
                event["running"] = state.workers.iter().flatten().count().into();
            }
            _ => {}
        }
        Some(event)
    }

    /// Each worker's line for a frame.
    fn workers(state: &State) -> Vec<Option<Worker>> {
        state
            .workers
            .iter()
            .map(|running| {
                running.as_ref().map(|running| {
                    let elapsed = running.started.elapsed();
                    Worker {
                        doing: running.doing.clone(),
                        elapsed,
                        slow: elapsed > running.slow.max(SLOW_FLOOR),
                    }
                })
            })
            .collect()
    }

    /// How far the mutants are by cost, and what's left once three have finished and ten seconds
    /// have passed: the time so far, scaled by the cost still to run.
    fn estimate(state: &State) -> (Option<f64>, Option<Duration>) {
        if state.phase != Phase::Mutants || state.cost.is_zero() {
            return (None, None);
        }
        let weighted = state.cost_done.as_secs_f64() / state.cost.as_secs_f64();
        let elapsed = state.since.elapsed();
        let ran = state.counts.caught + state.counts.missed + state.counts.timed_out;
        if ran < 3 || elapsed < Duration::from_secs(10) || state.cost_done.is_zero() {
            return (Some(weighted), None);
        }
        let left = (state.cost - state.cost_done).as_secs_f64() / state.cost_done.as_secs_f64();
        (Some(weighted), Some(elapsed.mul_f64(left)))
    }

    /// Clears the live block, writes `text` as permanent lines, and draws the block again, in one
    /// write, so the terminal never shows the block half gone.
    fn write(&self, state: &mut State, text: &str) {
        if self.shown == Shown::None {
            return;
        }
        let out = format!("{}{text}\n{}", self.clear(state), self.draw(state));
        self.send(&out);
        state.printed = Instant::now();
    }

    /// One write to stderr. On a terminal the frame is wrapped in synchronized output (mode 2026),
    /// so a terminal that knows it paints the frame at once; one that doesn't ignores it.
    fn send(&self, out: &str) {
        if out.is_empty() {
            return;
        }
        let out = if self.shown == Shown::Bar {
            format!("\x1b[?2026h{out}\x1b[?2026l")
        } else {
            out.to_string()
        };
        let mut stderr = std::io::stderr().lock();
        let _ = stderr.write_all(out.as_bytes());
        let _ = stderr.flush();
    }

    /// The tab's progress for now (OSC 9;4): busy while building, covering and checking; the
    /// mutants' weighted percent, red once the score has fallen below the threshold.
    fn tab(&self, state: &mut State, weighted: Option<f64>) -> String {
        if !self.tab || state.stopped {
            return String::new();
        }
        terminal::tab_shown(true);
        if state.phase != Phase::Mutants {
            return "\x1b]9;4;3;0\x1b\\".to_string();
        }
        let counts = &state.counts;
        let scored = counts.done - counts.unscored;
        if let (Some(threshold), true) = (self.threshold, scored >= 20) {
            let score = 100.0 * (counts.caught + counts.timed_out) as f64 / scored as f64;
            state.red |= score < threshold;
        }
        let percent = (weighted.unwrap_or(0.0) * 100.0).floor() as u32;
        format!("\x1b]9;4;{};{percent}\x1b\\", if state.red { 2 } else { 1 })
    }

    /// What clears the live block: the line it ends on, then each line above.
    fn clear(&self, state: &mut State) -> String {
        if state.drawn == 0 {
            return String::new();
        }
        let up = "\x1b[1A\x1b[2K".repeat(state.drawn - 1);
        state.drawn = 0;
        format!("\r\x1b[2K{up}")
    }

    /// The live block, on a terminal, fitted to its width. Wrapping is off while it's drawn, so a
    /// line the terminal narrowed under is cut there rather than wrapped onto a line the next
    /// redraw wouldn't clear.
    fn draw(&self, state: &mut State) -> String {
        if self.shown != Shown::Bar || state.stopped {
            return String::new();
        }
        let workers = Self::workers(state);
        let (weighted, left) = Self::estimate(state);
        let view = View {
            phase: &state.phase,
            counts: &state.counts,
            elapsed: state.since.elapsed(),
            weighted,
            left,
            workers: &workers,
            keys: self.keys,
        };
        let columns = terminal::width().unwrap_or(100);
        let lines = frames::live(&view, columns, state.frame, self.style);
        state.drawn = lines.len();
        let tab = self.tab(state, weighted);
        if lines.is_empty() {
            return tab;
        }
        format!("{tab}\x1b[?7l{}\x1b[?7h", lines.join("\n"))
    }

    /// What's running, above the live block, when someone asks.
    fn dump(&self, state: &mut State) {
        let workers = Self::workers(state);
        let held: Vec<Option<frames::Held>> = state
            .workers
            .iter()
            .zip(&workers)
            .map(|(running, worker)| {
                let (running, worker) = (running.as_ref()?, worker.as_ref()?);
                Some(frames::Held {
                    id: &running.id,
                    doing: &running.doing,
                    elapsed: worker.elapsed,
                    expected: running.expected,
                    slow: worker.slow,
                    tests: running.tests,
                })
            })
            .collect();
        let view = View {
            phase: &state.phase,
            counts: &state.counts,
            elapsed: state.since.elapsed(),
            weighted: None,
            left: None,
            workers: &workers,
            keys: self.keys,
        };
        let lines = frames::dump(&view, &held, self.style);
        drop(held);
        if !lines.is_empty() {
            self.write(state, &lines.join("\n"));
        }
    }

    /// Every 100 ms on a terminal, the block again with its clock moved; elsewhere, a heartbeat
    /// whenever the log has been quiet for [`heartbeat`]. Either way, what's running when asked.
    fn tick(&self) {
        let every = match self.shown {
            Shown::Bar => Duration::from_millis(100),
            _ => (self.heartbeat / 4).min(Duration::from_millis(250)),
        };
        let Some(mut state) = self.lock() else {
            return;
        };
        loop {
            state = match self.wake.wait_timeout(state, every) {
                Ok((state, _)) => state,
                Err(_) => return,
            };
            if state.stopped {
                return;
            }
            if state.started_sent && state.evented.elapsed() >= self.heartbeat {
                if let Some(event) = Self::progress_event(&state) {
                    self.event(&event);
                }
                state.evented = Instant::now();
            }
            if self.shown == Shown::None {
                continue;
            }
            if terminal::asked() {
                self.dump(&mut state);
            }
            if self.shown == Shown::Bar {
                state.frame += 1;
                let out = format!("{}{}", self.clear(&mut state), self.draw(&mut state));
                self.send(&out);
            } else if state.printed.elapsed() >= self.heartbeat {
                let workers = Self::workers(&state);
                let (weighted, left) = Self::estimate(&state);
                let view = View {
                    phase: &state.phase,
                    counts: &state.counts,
                    elapsed: state.since.elapsed(),
                    weighted,
                    left,
                    workers: &workers,
                    keys: false,
                };
                if let Some(line) = frames::heartbeat_line(&view) {
                    stderr_line!("{line}");
                }
                state.printed = Instant::now();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tab_shows_progress_only_where_the_terminal_says_it_draws_it() {
        let with = |vars: &[(&str, &str)]| {
            terminal::tab_progress(|name| {
                vars.iter()
                    .find(|(var, _)| *var == name)
                    .map(|(_, value)| value.to_string())
            })
        };
        assert!(with(&[("WT_SESSION", "f00")]));
        assert!(with(&[("TERM_PROGRAM", "ghostty")]));
        assert!(with(&[("TERM_PROGRAM", "WezTerm")]));
        assert!(with(&[("TERM", "xterm-ghostty")]));
        assert!(with(&[("ConEmuANSI", "ON")]));
        // iTerm2 takes `ESC ] 9` for a notification: nothing unless asked.
        assert!(!with(&[("TERM_PROGRAM", "iTerm.app")]));
        assert!(with(&[
            ("TERM_PROGRAM", "iTerm.app"),
            ("MUTASTIC_TERM_PROGRESS", "true")
        ]));
        assert!(!with(&[
            ("WT_SESSION", "f00"),
            ("MUTASTIC_TERM_PROGRESS", "false")
        ]));
        assert!(with(&[("CARGO_TERM_PROGRESS_TERM_INTEGRATION", "true")]));
        assert!(!with(&[]));
    }

    #[test]
    fn auto_follows_what_was_asked() {
        assert_eq!(resolve(Some(Shown::None)), Shown::None);
        assert_eq!(resolve(Some(Shown::Bar)), Shown::Bar);
        assert_eq!(Shown::parse("auto"), Some(None));
        assert_eq!(Shown::parse("sometimes"), None);
    }
}
