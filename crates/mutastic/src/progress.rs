//! What a run says while it runs, on stderr, so stdout stays the result: a line for every phase,
//! a live block redrawn in place on a terminal, and a heartbeat line everywhere else, so a run is
//! never silent. `PROGRESS.md` at the repository's root is the contract, shared with the Java
//! edition word for word.

use std::io::{IsTerminal as _, Write as _};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::engine::{self, Status};
use crate::report::{self, Style};

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
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map_or(Duration::from_secs(30), Duration::from_secs_f64)
}

/// What the run is doing now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Reading the package and finding its mutants.
    Reading,
    /// Cargo's round: `check` or `build`, its number, the crates finished and the latest it
    /// started.
    Building {
        step: String,
        round: usize,
        crates: usize,
        last: String,
    },
    /// The coverage pass: tests run so far, of how many.
    Coverage { done: usize, total: usize },
    /// The mutants, counted in [`Counts`].
    Mutants,
    /// Looking for a test that catches each missed mutant.
    Checking { done: usize, total: usize },
    /// Between phases, or after the last: nothing live.
    Idle,
}

/// The mutants' results so far.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub done: usize,
    pub total: usize,
    pub caught: usize,
    pub missed: usize,
    pub not_covered: usize,
    pub timed_out: usize,
}

impl Counts {
    fn add(&mut self, status: Status) {
        self.done += 1;
        match status {
            Status::Killed => self.caught += 1,
            Status::Survived => self.missed += 1,
            Status::NoCoverage => self.not_covered += 1,
            Status::Timeout => self.timed_out += 1,
            Status::Ignored | Status::RuntimeError => {}
        }
    }

    /// `✓ 2610 caught · ✗ 41 missed · ○ 214 not covered · ⏱ 6 timed out`.
    fn fates(&self, style: Style) -> String {
        format!(
            "{} · {} · ○ {} not covered · {}",
            paint(style, "32", &format!("✓ {} caught", self.caught)),
            paint(style, "1;31", &format!("✗ {} missed", self.missed)),
            self.not_covered,
            paint(style, "33", &format!("⏱ {} timed out", self.timed_out))
        )
    }
}

const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

fn paint(style: Style, code: &str, text: &str) -> String {
    if style.color {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

/// A phase's word, in a column of its own: `building  `.
fn word(word: &str) -> String {
    format!("{word:<10}")
}

/// A phase's clock: `38s`, `3m12s`, `1h02m`.
pub fn clock(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m{:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h{:02}m", seconds / 3600, (seconds / 60) % 60)
    }
}

/// `1 test`, `3 tests`.
pub fn count(number: usize, noun: &str) -> String {
    format!("{number} {noun}{}", if number == 1 { "" } else { "s" })
}

/// What a phase says, between its word and its clock, and its word; `None` when nothing is live.
fn fields(
    phase: &Phase,
    counts: &Counts,
    style: Style,
    live: bool,
) -> Option<(&'static str, String)> {
    Some(match phase {
        Phase::Reading => ("reading", "the Cargo package".to_string()),
        Phase::Building {
            step,
            round,
            crates,
            ..
        } => (
            "building",
            format!("cargo {step}, round {round} · {}", count(*crates, "crate")),
        ),
        Phase::Coverage { total: 0, .. } => ("coverage", "listing the tests".to_string()),
        Phase::Coverage { done, total } if live => ("coverage", format!("{done}/{total} tests")),
        Phase::Coverage { done, total } => ("coverage", format!("[{done}/{total} tests]")),
        Phase::Mutants => (
            "mutants",
            format!("[{}/{}] {}", counts.done, counts.total, counts.fates(style)),
        ),
        Phase::Checking { done, total } if live => (
            "checking",
            format!("{total} missed · looking for a test that would catch each, {done}/{total}"),
        ),
        Phase::Checking { done, total } => ("checking", format!("{total} missed · {done}/{total}")),
        Phase::Idle => return None,
    })
}

/// The live block on a terminal: the phase's line, with a spinner and its clock, and for the
/// build, Cargo's latest line beneath it, dim.
pub fn live(
    phase: &Phase,
    counts: &Counts,
    elapsed: Duration,
    frame: usize,
    style: Style,
) -> Vec<String> {
    let Some((name, said)) = fields(phase, counts, style, true) else {
        return Vec::new();
    };
    let spinner = SPINNER[frame % SPINNER.len()].to_string();
    let mut lines = vec![format!(
        "{} {}{said} · {}",
        paint(style, "32", &spinner),
        paint(style, "1", &word(name)),
        clock(elapsed)
    )];
    if let Phase::Building { last, .. } = phase {
        if !last.is_empty() {
            lines.push(paint(style, "2", &format!("   └ {last}")));
        }
    }
    lines
}

/// A heartbeat: the live line's fields, plain, for a log.
pub fn heartbeat_line(phase: &Phase, counts: &Counts, elapsed: Duration) -> Option<String> {
    let (name, said) = fields(phase, counts, Style::default(), false)?;
    Some(format!("{}{said} · {}", word(name), clock(elapsed)))
}

/// `built     5525 mutants in 9m41s · 2 rounds (check and build), none withdrawn`.
pub fn built_line(mutants: usize, took: Duration, rounds: &[&str], withdrawn: usize) -> String {
    let rounds = match rounds {
        ["check"] => "1 round (check)".to_string(),
        ["check", "build"] => "2 rounds (check and build)".to_string(),
        _ => count(rounds.len(), "round"),
    };
    let withdrawn = match withdrawn {
        0 => "none withdrawn".to_string(),
        n => format!("{n} withdrawn"),
    };
    format!(
        "{}{} in {} · {rounds}, {withdrawn}",
        word("built"),
        count(mutants, "mutant"),
        report::duration(took)
    )
}

/// `ran       5525 mutants in 34m12s · ✓ 5250 caught · …`.
pub fn ran_line(counts: &Counts, took: Duration, style: Style) -> String {
    format!(
        "{}{} in {} · {}",
        word("ran"),
        count(counts.done, "mutant"),
        report::duration(took),
        counts.fates(style)
    )
}

/// `checked   52 missed mutants in 51.0s · a test to write for 41`.
pub fn checked_line(missed: usize, took: Duration, found: usize) -> String {
    let found = if found == 0 {
        "no test found to suggest".to_string()
    } else {
        format!("a test to write for {found}")
    };
    format!(
        "{}{} in {} · {found}",
        word("checked"),
        count(missed, "missed mutant"),
        report::duration(took)
    )
}

struct State {
    phase: Phase,
    since: Instant,
    counts: Counts,
    /// Lines of the live block on screen now.
    drawn: usize,
    /// When the last progress line was written, for the heartbeat.
    printed: Instant,
    frame: usize,
    stopped: bool,
    package: String,
}

struct Inner {
    shown: Shown,
    style: Style,
    /// Whether each source file's line is printed as its last mutant finishes.
    files: bool,
    workers: usize,
    heartbeat: Duration,
    state: Mutex<State>,
    wake: Condvar,
}

/// The run's progress on stderr, with a ticker thread that moves the clock on a terminal and
/// writes the heartbeat elsewhere, so nothing waits on the next event to say something.
pub struct Progress {
    inner: Arc<Inner>,
    ticker: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Progress {
    /// Starts the run's progress: its first line, before anything slow, and the ticker.
    pub fn start(shown: Shown, style: Style, files: bool, workers: usize) -> Progress {
        let now = Instant::now();
        let inner = Arc::new(Inner {
            shown,
            style,
            files,
            workers,
            heartbeat: heartbeat(),
            state: Mutex::new(State {
                phase: Phase::Reading,
                since: now,
                counts: Counts::default(),
                drawn: 0,
                printed: now,
                frame: 0,
                stopped: shown == Shown::None,
                package: String::new(),
            }),
            wake: Condvar::new(),
        });
        let progress = Progress {
            inner: Arc::clone(&inner),
            ticker: Mutex::new(None),
        };
        if shown == Shown::None {
            return progress;
        }
        progress.line(&format!(
            "mutastic {} · reading the Cargo package…",
            env!("CARGO_PKG_VERSION")
        ));
        let ticking = Arc::clone(&inner);
        if let Ok(mut ticker) = progress.ticker.lock() {
            *ticker = Some(std::thread::spawn(move || ticking.tick()));
        }
        progress
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
        if self.inner.shown == Shown::None {
            return;
        }
        let Some(mut state) = self.inner.lock() else {
            return;
        };
        let inner = &self.inner;
        match step {
            engine::Progress::Building { mutants } => {
                inner.enter(
                    &mut state,
                    Phase::Building {
                        step: "check".into(),
                        round: 1,
                        crates: 0,
                        last: String::new(),
                    },
                );
                let line = format!(
                    "{}{} in one build · {}",
                    word("building"),
                    count(mutants, "mutant"),
                    count(inner.workers, "worker")
                );
                inner.write(&mut state, &line);
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
            } => {
                inner.enter(&mut state, Phase::Coverage { done: 0, total: 0 });
                inner.write(&mut state, &built_line(mutants, took, rounds, withdrawn));
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
            } => {
                state.counts.total = mutants;
                inner.enter(&mut state, Phase::Mutants);
                let line = format!(
                    "{}{} on rustc {rustc} in {}{} · {}",
                    word("coverage"),
                    count(tests, "test"),
                    report::duration(took),
                    if flaky == 0 {
                        String::new()
                    } else {
                        format!(" ({flaky} flaky, left out)")
                    },
                    count(mutants, "mutant")
                );
                inner.write(&mut state, &line);
            }
            engine::Progress::FromShares { summary, mutants } => {
                state.counts.total = mutants;
                inner.enter(&mut state, Phase::Idle);
                inner.write(&mut state, summary);
            }
            engine::Progress::MutantFinished { status } => state.counts.add(status),
            engine::Progress::FileFinished { found, results } if inner.files => {
                let line = report::file_finished(&state.package, found, results, inner.style);
                inner.write(&mut state, line.trim_end_matches('\n'));
            }
            engine::Progress::FileFinished { .. } => {}
            engine::Progress::Ran { took } => {
                inner.enter(&mut state, Phase::Idle);
                let line = ran_line(&state.counts, took, inner.style);
                inner.write(&mut state, &line);
            }
            engine::Progress::Checking { done, total } => {
                if !matches!(state.phase, Phase::Checking { .. }) {
                    state.since = Instant::now();
                }
                state.phase = Phase::Checking { done, total };
            }
            engine::Progress::Checked {
                missed,
                took,
                found,
            } => {
                inner.enter(&mut state, Phase::Idle);
                inner.write(&mut state, &checked_line(missed, took, found));
            }
        }
    }

    /// Stops the ticker and clears the live block, before the result.
    pub fn finished(&self) {
        if let Some(mut state) = self.inner.lock() {
            state.stopped = true;
            self.inner.clear(&mut state);
            self.inner.wake.notify_all();
        }
        let ticker = self.ticker.lock().ok().and_then(|mut ticker| ticker.take());
        if let Some(ticker) = ticker {
            let _ = ticker.join();
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
    }

    /// Clears the live block, writes `text` as permanent lines, and draws the block again.
    fn write(&self, state: &mut State, text: &str) {
        if self.shown == Shown::None {
            return;
        }
        self.clear(state);
        stderr_line!("{text}");
        state.printed = Instant::now();
        self.draw(state);
    }

    fn clear(&self, state: &mut State) {
        if state.drawn == 0 {
            return;
        }
        let up = "\x1b[1A\x1b[2K".repeat(state.drawn - 1);
        stderr_text!("\r\x1b[2K{up}");
        state.drawn = 0;
    }

    /// The live block, on a terminal. Wrapping is off while it's drawn, so a line wider than the
    /// terminal is cut there rather than wrapped onto a line the next redraw wouldn't clear.
    fn draw(&self, state: &mut State) {
        if self.shown != Shown::Bar || state.stopped {
            return;
        }
        let lines = live(
            &state.phase,
            &state.counts,
            state.since.elapsed(),
            state.frame,
            self.style,
        );
        if lines.is_empty() {
            return;
        }
        stderr_text!("\x1b[?7l{}\x1b[?7h", lines.join("\n"));
        let _ = std::io::stderr().flush();
        state.drawn = lines.len();
    }

    /// Every 100 ms on a terminal, the block again with its clock moved; elsewhere, a heartbeat
    /// whenever the log has been quiet for [`heartbeat`].
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
            if self.shown == Shown::Bar {
                state.frame += 1;
                self.clear(&mut state);
                self.draw(&mut state);
            } else if state.printed.elapsed() >= self.heartbeat {
                if let Some(line) =
                    heartbeat_line(&state.phase, &state.counts, state.since.elapsed())
                {
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

    fn counts() -> Counts {
        Counts {
            done: 2871,
            total: 5525,
            caught: 2610,
            missed: 41,
            not_covered: 214,
            timed_out: 6,
        }
    }

    fn building() -> Phase {
        Phase::Building {
            step: "check".into(),
            round: 1,
            crates: 212,
            last: "Checking ra_ap_syntax v0.0.301".into(),
        }
    }

    #[test]
    fn each_phase_has_a_live_line_with_its_clock() {
        let plain = Style::default();
        let at = |phase: Phase, seconds: u64| {
            live(&phase, &counts(), Duration::from_secs(seconds), 5, plain)
        };
        assert_eq!(
            at(Phase::Reading, 1),
            ["⠴ reading   the Cargo package · 1s"]
        );
        assert_eq!(
            at(building(), 192),
            [
                "⠴ building  cargo check, round 1 · 212 crates · 3m12s",
                "   └ Checking ra_ap_syntax v0.0.301"
            ]
        );
        assert_eq!(
            at(
                Phase::Coverage {
                    done: 72,
                    total: 181
                },
                242
            ),
            ["⠴ coverage  72/181 tests · 4m02s"]
        );
        assert_eq!(
            at(Phase::Mutants, 1450),
            ["⠴ mutants   [2871/5525] ✓ 2610 caught · ✗ 41 missed · ○ 214 not covered · ⏱ 6 timed out · 24m10s"]
        );
        assert_eq!(
            at(
                Phase::Checking {
                    done: 18,
                    total: 52
                },
                38
            ),
            ["⠴ checking  52 missed · looking for a test that would catch each, 18/52 · 38s"]
        );
        assert!(at(Phase::Idle, 3).is_empty());
    }

    #[test]
    fn the_spinner_turns_and_the_clock_runs_past_an_hour() {
        let line = |frame, seconds| {
            live(
                &Phase::Reading,
                &counts(),
                Duration::from_secs(seconds),
                frame,
                Style::default(),
            )
        };
        assert!(line(0, 0)[0].starts_with('⠋'));
        assert!(line(11, 0)[0].starts_with('⠙'));
        assert!(line(0, 3725)[0].ends_with("1h02m"));
    }

    #[test]
    fn a_heartbeat_says_the_same_plainly() {
        let beat = |phase: Phase, seconds: u64| {
            heartbeat_line(&phase, &counts(), Duration::from_secs(seconds))
        };
        assert_eq!(
            beat(building(), 30).unwrap(),
            "building  cargo check, round 1 · 212 crates · 30s"
        );
        assert_eq!(
            beat(
                Phase::Coverage {
                    done: 18,
                    total: 181
                },
                60
            )
            .unwrap(),
            "coverage  [18/181 tests] · 1m00s"
        );
        assert_eq!(
            beat(Phase::Mutants, 270).unwrap(),
            "mutants   [2871/5525] ✓ 2610 caught · ✗ 41 missed · ○ 214 not covered · ⏱ 6 timed out · 4m30s"
        );
        assert_eq!(
            beat(
                Phase::Checking {
                    done: 18,
                    total: 52
                },
                30
            )
            .unwrap(),
            "checking  52 missed · 18/52 · 30s"
        );
        assert_eq!(beat(Phase::Idle, 30), None);
    }

    #[test]
    fn colour_marks_the_spinner_the_word_and_the_fates() {
        let style = Style {
            color: true,
            ..Style::default()
        };
        let line = &live(&Phase::Mutants, &counts(), Duration::from_secs(1), 0, style)[0];
        assert!(
            line.starts_with("\x1b[32m⠋\x1b[0m \x1b[1mmutants   \x1b[0m"),
            "{line:?}"
        );
        assert!(line.contains("\x1b[1;31m✗ 41 missed\x1b[0m"), "{line:?}");
    }

    #[test]
    fn each_phase_ends_on_a_line_of_its_own() {
        let check_and_build = ["check", "build"];
        assert_eq!(
            built_line(5525, Duration::from_secs(581), &check_and_build, 0),
            "built     5525 mutants in 9m41s · 2 rounds (check and build), none withdrawn"
        );
        assert_eq!(
            built_line(
                1,
                Duration::from_millis(400),
                &["check", "check", "build"],
                12
            ),
            "built     1 mutant in 400ms · 3 rounds, 12 withdrawn"
        );
        let mut done = counts();
        done.done = 5525;
        assert_eq!(
            ran_line(&done, Duration::from_secs(2052), Style::default()),
            "ran       5525 mutants in 34m12s · ✓ 2610 caught · ✗ 41 missed · ○ 214 not covered · ⏱ 6 timed out"
        );
        assert_eq!(
            checked_line(52, Duration::from_secs(51), 41),
            "checked   52 missed mutants in 51.0s · a test to write for 41"
        );
        assert_eq!(
            checked_line(1, Duration::from_millis(80), 0),
            "checked   1 missed mutant in 80ms · no test found to suggest"
        );
    }

    #[test]
    fn auto_is_counter_under_ci_or_an_agent() {
        // Read as the resolver reads them; the test process's own environment decides the rest.
        assert_eq!(resolve(Some(Shown::None)), Shown::None);
        assert_eq!(resolve(Some(Shown::Bar)), Shown::Bar);
        assert_eq!(Shown::parse("auto"), Some(None));
        assert_eq!(Shown::parse("sometimes"), None);
    }
}
