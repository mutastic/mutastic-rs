//! What progress says, as text: pure functions of what the run is doing, how long it's been, the
//! spinner's frame and the terminal's width, so each can be tested as text. `PROGRESS.md` is the
//! contract.

use std::time::Duration;

use crate::report::{self, Style};

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

/// What a worker is running: a test in the coverage pass, a mutant after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worker {
    /// `src/lib.rs:26 discount() >= becomes >`, or a test's name.
    pub doing: String,
    pub elapsed: Duration,
    pub slow: bool,
}

/// Everything a frame shows.
#[derive(Clone, Debug)]
pub struct View<'a> {
    pub phase: &'a Phase,
    pub counts: &'a Counts,
    /// The phase's own clock.
    pub elapsed: Duration,
    /// How far the mutants are by what each was expected to take, from 0 to 1.
    pub weighted: Option<f64>,
    /// What's left, once there's enough to tell.
    pub left: Option<Duration>,
    /// Each worker's test or mutant, `None` when it's idle.
    pub workers: &'a [Option<Worker>],
    /// Whether `t` shows what's running here.
    pub keys: bool,
}

const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const GREEN: &str = "32";
const RED: &str = "1;31";
const AMBER: &str = "33";
const BOLD: &str = "1";
const DIM: &str = "2";

pub fn paint(style: Style, code: &str, text: &str) -> String {
    if style.color {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

/// A phase's word, in a column of its own: `building  `.
pub fn word(word: &str) -> String {
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

/// A worker's timer: `[00:03:51]`.
fn timer(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    format!(
        "[{:02}:{:02}:{:02}]",
        seconds / 3600,
        (seconds / 60) % 60,
        seconds % 60
    )
}

/// What's left, steadily: whole minutes above ten, ten seconds above one, five below.
pub fn left(remaining: Duration) -> String {
    let seconds = remaining.as_secs_f64();
    if seconds >= 600.0 {
        format!("~{}m left", (seconds / 60.0).round())
    } else if seconds >= 60.0 {
        let tens = ((seconds / 10.0).round() * 10.0) as u64;
        format!("~{}m{:02}s left", tens / 60, tens % 60)
    } else {
        format!("~{}s left", (((seconds / 5.0).round() * 5.0) as u64).max(5))
    }
}

/// `1 test`, `3 tests`.
pub fn count(number: usize, noun: &str) -> String {
    format!("{number} {noun}{}", if number == 1 { "" } else { "s" })
}

/// The fates in words, `✓ 2610 caught · ✗ 41 missed · ○ 214 not covered · ⏱ 6 timed out`, or
/// shorter where there's less room: `✓ 2610 caught · ✗ 41 missed · ○ 214 · ⏱ 6`, then
/// `✓2610 ✗41 ○214 ⏱6`.
pub fn fates(counts: &Counts, style: Style, tier: usize) -> String {
    let (caught, missed) = (counts.caught, counts.missed);
    let (uncovered, timed_out) = (counts.not_covered, counts.timed_out);
    match tier {
        0 => format!(
            "{} · {} · ○ {uncovered} not covered · {}",
            paint(style, GREEN, &format!("✓ {caught} caught")),
            paint(style, RED, &format!("✗ {missed} missed")),
            paint(style, AMBER, &format!("⏱ {timed_out} timed out"))
        ),
        1 => format!(
            "{} · {} · ○ {uncovered} · {}",
            paint(style, GREEN, &format!("✓ {caught} caught")),
            paint(style, RED, &format!("✗ {missed} missed")),
            paint(style, AMBER, &format!("⏱ {timed_out}"))
        ),
        _ => format!(
            "{} {} ○{uncovered} {}",
            paint(style, GREEN, &format!("✓{caught}")),
            paint(style, RED, &format!("✗{missed}")),
            paint(style, AMBER, &format!("⏱{timed_out}"))
        ),
    }
}

/// The width a terminal gives `text`: its characters, without colour or links.
pub fn width(text: &str) -> usize {
    let mut width = 0;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            width += 1;
            continue;
        }
        match chars.next() {
            Some('[') => {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == '\x07' || (c == '\x1b' && chars.next().is_some()) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    width
}

/// `text` cut to `columns`, ending in `…` when cut, its colour closed.
pub fn fit(text: &str, columns: usize) -> String {
    if width(text) <= columns {
        return text.to_string();
    }
    let mut out = String::new();
    let mut shown = 0;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            out.push(c);
            for c in chars.by_ref() {
                out.push(c);
                if ('@'..='~').contains(&c) && c != '[' {
                    break;
                }
            }
            continue;
        }
        if shown + 1 >= columns {
            break;
        }
        out.push(c);
        shown += 1;
    }
    out.push('…');
    if text.contains('\x1b') {
        out.push_str("\x1b[0m");
    }
    out
}

/// A bar `columns` wide, filled to `fraction`.
fn bar(fraction: f64, columns: usize, style: Style) -> String {
    let full = ((fraction.clamp(0.0, 1.0) * columns as f64).round() as usize).min(columns);
    format!(
        "{}{}",
        paint(style, GREEN, &"━".repeat(full)),
        paint(style, DIM, &"─".repeat(columns - full))
    )
}

/// The phase's first line: the spinner and the phase's word, then for coverage and mutants a bar,
/// then what it's done, then its clock and what's left. The bar shrinks first as the terminal
/// narrows, then the fates lose their words.
fn status(view: &View, columns: usize, frame: usize, style: Style) -> Option<String> {
    let spinner = SPINNER[frame % SPINNER.len()].to_string();
    let lead = |name: &str| {
        format!(
            "{} {}",
            paint(style, GREEN, &spinner),
            paint(style, BOLD, &word(name))
        )
    };
    let clock = clock(view.elapsed);
    let line = match view.phase {
        Phase::Reading => format!("{}the Cargo package · {clock}", lead("reading")),
        Phase::Building {
            step,
            round,
            crates,
            ..
        } => format!(
            "{}cargo {step}, round {round} · {} · {clock}",
            lead("building"),
            count(*crates, "crate")
        ),
        Phase::Coverage { total: 0, .. } => {
            format!("{}listing the tests · {clock}", lead("coverage"))
        }
        Phase::Coverage { done, total } => {
            let said = format!("  {done}/{total} tests · {clock}");
            let room = columns.saturating_sub(12 + said.chars().count());
            let fraction = *done as f64 / *total as f64;
            format!(
                "{}{}{said}",
                lead("coverage"),
                bar(fraction, room.min(40), style)
            )
        }
        Phase::Mutants => mutants(view, &lead("mutants"), columns, style),
        Phase::Checking { done, total } => format!(
            "{}{total} missed · looking for a test that would catch each, {done}/{total} · {clock}",
            lead("checking")
        ),
        Phase::Idle => return None,
    };
    Some(fit(&line, columns))
}

fn mutants(view: &View, lead: &str, columns: usize, style: Style) -> String {
    let counts = view.counts;
    let percent = view.weighted.map_or(String::new(), |weighted| {
        format!("{}% · ", (weighted * 100.0).floor())
    });
    let head = format!("  {percent}{}/{} · ", counts.done, counts.total);
    let tail = format!(
        " · {}{}",
        clock(view.elapsed),
        view.left
            .map_or(String::new(), |remaining| format!(" · {}", left(remaining)))
    );
    for tier in 0..3 {
        let said = fates(counts, style, tier);
        let room = columns.saturating_sub(12 + width(&head) + width(&said) + width(&tail));
        if room >= if tier == 2 { 6 } else { 12 } {
            let fraction = view
                .weighted
                .unwrap_or(counts.done as f64 / counts.total.max(1) as f64);
            return format!(
                "{lead}{}{head}{said}{tail}",
                bar(fraction, room.min(40), style)
            );
        }
    }
    format!(
        "{lead}{}{}{tail}",
        head.trim_start(),
        fates(counts, style, 2)
    )
}

/// The live block on a terminal: the phase's line, then Cargo's latest line while it builds, or a
/// line for each worker with its own timer, SLOW past what its mutant should take, up to eight
/// (four below 100 columns), and how many more.
pub fn live(view: &View, columns: usize, frame: usize, style: Style) -> Vec<String> {
    let Some(status) = status(view, columns, frame, style) else {
        return Vec::new();
    };
    let mut lines = vec![status];
    if let Phase::Building { last, .. } = view.phase {
        if !last.is_empty() {
            lines.push(fit(&paint(style, DIM, &format!("   └ {last}")), columns));
        }
        return lines;
    }
    if !matches!(view.phase, Phase::Coverage { .. } | Phase::Mutants) || view.workers.is_empty() {
        return lines;
    }
    let shown = if columns >= 100 { 8 } else { 4 };
    for (number, worker) in view.workers.iter().enumerate().take(shown) {
        let line = match worker {
            None => paint(style, DIM, &format!("{:>4} idle", number + 1)),
            Some(worker) => format!(
                "{:>4} {} {} {}",
                number + 1,
                paint(style, DIM, &timer(worker.elapsed)),
                if worker.slow {
                    paint(style, "1;33", "SLOW ")
                } else {
                    "     ".to_string()
                },
                worker.doing
            ),
        };
        lines.push(fit(&line, columns));
    }
    let more = view
        .workers
        .iter()
        .skip(shown)
        .filter(|w| w.is_some())
        .count();
    let mut footer = Vec::new();
    if more > 0 {
        footer.push(format!("… and {more} more running"));
    }
    if view.keys {
        footer.push("t for what’s running".to_string());
    }
    if !footer.is_empty() {
        lines.push(fit(
            &paint(style, DIM, &format!("     {}", footer.join(" · "))),
            columns,
        ));
    }
    lines
}

/// A heartbeat: the phase's fields as a plain line, for a log.
pub fn heartbeat_line(view: &View) -> Option<String> {
    let clock = clock(view.elapsed);
    let plain = Style::default();
    Some(match view.phase {
        Phase::Reading => format!("{}the Cargo package · {clock}", word("reading")),
        Phase::Building {
            step,
            round,
            crates,
            ..
        } => format!(
            "{}cargo {step}, round {round} · {} · {clock}",
            word("building"),
            count(*crates, "crate")
        ),
        Phase::Coverage { total: 0, .. } => {
            format!("{}listing the tests · {clock}", word("coverage"))
        }
        Phase::Coverage { done, total } => {
            format!("{}[{done}/{total} tests] · {clock}", word("coverage"))
        }
        Phase::Mutants => {
            let counts = view.counts;
            let percent = view.weighted.map_or(String::new(), |weighted| {
                format!(" {}%", (weighted * 100.0).floor())
            });
            let remaining = view
                .left
                .map_or(String::new(), |remaining| format!(" · {}", left(remaining)));
            format!(
                "{}[{}/{}]{percent} · {} · {clock}{remaining}",
                word("mutants"),
                counts.done,
                counts.total,
                fates(counts, plain, 0)
            )
        }
        Phase::Checking { done, total } => format!(
            "{}{total} missed · {done}/{total} · {clock}",
            word("checking")
        ),
        Phase::Idle => return None,
    })
}

/// A running mutant for the dump: its id, where it is, its timer against what it should take, and
/// the tests that cover it.
pub struct Held<'a> {
    pub id: &'a str,
    pub doing: &'a str,
    pub elapsed: Duration,
    pub expected: Duration,
    pub slow: bool,
    pub tests: usize,
}

/// What's running, for `t`, Ctrl-T or SIGUSR1: a heading, then each worker's work.
pub fn dump(view: &View, held: &[Option<Held>], style: Style) -> Vec<String> {
    let heading = |text: String| format!("{}{text}", paint(style, "1;34", &word("running")));
    let at = clock(view.elapsed);
    match view.phase {
        Phase::Reading => vec![heading(format!("reading the Cargo package at {at}"))],
        Phase::Building {
            step,
            round,
            crates,
            last,
        } => vec![
            heading(format!(
                "cargo {step}, round {round} at {at} · {} done",
                count(*crates, "crate")
            )),
            paint(style, DIM, &format!("          last: \"{last}\"")),
        ],
        Phase::Coverage { done, total } => {
            let mut lines = vec![heading(format!(
                "{} at {at} · {done} of {total} done",
                count(view.workers.iter().flatten().count(), "test")
            ))];
            for (number, worker) in view.workers.iter().enumerate() {
                if let Some(worker) = worker {
                    lines.push(format!(
                        "{:>4} {}  {}{}",
                        number + 1,
                        worker.doing,
                        paint(style, DIM, &timer(worker.elapsed)),
                        if worker.slow {
                            paint(style, "1;33", " SLOW")
                        } else {
                            String::new()
                        }
                    ));
                }
            }
            lines
        }
        Phase::Mutants => {
            let mut lines = vec![heading(format!(
                "{} at {at}",
                count(held.iter().flatten().count(), "worker")
            ))];
            for (number, held) in held.iter().enumerate() {
                let Some(held) = held else {
                    lines.push(paint(style, DIM, &format!("{:>4} idle", number + 1)));
                    continue;
                };
                lines.push(format!(
                    "{:>4} {}  {}  {}{} {}",
                    number + 1,
                    paint(style, "36", held.id),
                    held.doing,
                    paint(style, DIM, &timer(held.elapsed)),
                    if held.slow {
                        paint(style, "1;33", " SLOW")
                    } else {
                        String::new()
                    },
                    paint(style, DIM, &format!("(expected {})", clock(held.expected)))
                ));
                lines.push(paint(
                    style,
                    DIM,
                    &format!("               covered by {}", count(held.tests, "test")),
                ));
            }
            lines
        }
        Phase::Checking { done, total } => vec![heading(format!(
            "suggestions for {total} missed mutants at {at} · {done} done"
        ))],
        Phase::Idle => Vec::new(),
    }
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
        fates(counts, style, 0)
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

    fn working() -> Vec<Option<Worker>> {
        let mut workers = vec![None; 8];
        workers[0] = Some(Worker {
            doing: "src/engine.rs:540 finished() && becomes ||".into(),
            elapsed: Duration::from_secs(4),
            slow: false,
        });
        workers[6] = Some(Worker {
            doing: "src/testrun.rs:114 spawn() call removed".into(),
            elapsed: Duration::from_secs(151),
            slow: true,
        });
        workers[7] = Some(Worker {
            doing: "src/cli.rs:753 line() + becomes -".into(),
            elapsed: Duration::from_secs(1),
            slow: false,
        });
        workers
    }

    fn view<'a>(
        phase: &'a Phase,
        counts: &'a Counts,
        workers: &'a [Option<Worker>],
        seconds: u64,
    ) -> View<'a> {
        View {
            phase,
            counts,
            elapsed: Duration::from_secs(seconds),
            weighted: Some(0.52),
            left: Some(Duration::from_secs(1260)),
            workers,
            keys: true,
        }
    }

    #[test]
    fn each_phase_has_a_live_line_with_its_clock() {
        let plain = Style::default();
        let (counts, none) = (counts(), Vec::new());
        let at =
            |phase: &Phase, seconds| live(&view(phase, &counts, &none, seconds), 120, 5, plain);
        assert_eq!(
            at(&Phase::Reading, 1),
            ["⠴ reading   the Cargo package · 1s"]
        );
        assert_eq!(
            at(&building(), 192),
            [
                "⠴ building  cargo check, round 1 · 212 crates · 3m12s",
                "   └ Checking ra_ap_syntax v0.0.301"
            ]
        );
        assert_eq!(
            at(
                &Phase::Checking {
                    done: 18,
                    total: 52
                },
                38
            ),
            ["⠴ checking  52 missed · looking for a test that would catch each, 18/52 · 38s"]
        );
        assert!(at(&Phase::Idle, 3).is_empty());
    }

    #[test]
    fn the_mutants_line_has_a_bar_the_weighted_percent_and_whats_left() {
        let (counts, workers) = (counts(), working());
        let lines = live(
            &view(&Phase::Mutants, &counts, &workers, 1450),
            160,
            9,
            Style::default(),
        );
        assert_eq!(
            lines,
            [
                "⠏ mutants   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  52% · 2871/5525 · ✓ 2610 caught · ✗ 41 missed · ○ 214 not covered · ⏱ 6 timed out · 24m10s · ~21m left"
                    .replace("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", &format!("{}{}", "━".repeat(21), "─".repeat(19))),
                "   1 [00:00:04]       src/engine.rs:540 finished() && becomes ||".into(),
                "   2 idle".into(),
                "   3 idle".into(),
                "   4 idle".into(),
                "   5 idle".into(),
                "   6 idle".into(),
                "   7 [00:02:31] SLOW  src/testrun.rs:114 spawn() call removed".into(),
                "   8 [00:00:01]       src/cli.rs:753 line() + becomes -".into(),
                "     t for what’s running".into(),
            ]
        );
    }

    #[test]
    fn a_narrow_terminal_shrinks_the_bar_then_the_words_then_shows_four_workers() {
        let (counts, workers) = (counts(), working());
        let at = |columns| {
            live(
                &view(&Phase::Mutants, &counts, &workers, 1450),
                columns,
                9,
                Style::default(),
            )
        };
        let wide = at(120);
        assert!(
            wide[0].contains("✓ 2610 caught · ✗ 41 missed · ○ 214 · ⏱ 6 · "),
            "{}",
            wide[0]
        );
        assert!(wide.iter().all(|line| width(line) <= 120));
        let narrow = at(80);
        assert!(narrow[0].contains("✓2610 ✗41 ○214 ⏱6"), "{}", narrow[0]);
        assert!(narrow.iter().all(|line| width(line) <= 80), "{narrow:#?}");
        assert_eq!(narrow.len(), 6, "{narrow:#?}");
        assert_eq!(
            narrow[5],
            "     … and 2 more running · t for what’s running"
        );
    }

    #[test]
    fn a_heartbeat_says_the_same_plainly() {
        let (counts, none) = (counts(), Vec::new());
        let beat =
            |phase: &Phase, seconds| heartbeat_line(&view(phase, &counts, &none, seconds)).unwrap();
        assert_eq!(
            beat(&building(), 30),
            "building  cargo check, round 1 · 212 crates · 30s"
        );
        assert_eq!(
            beat(
                &Phase::Coverage {
                    done: 18,
                    total: 181
                },
                60
            ),
            "coverage  [18/181 tests] · 1m00s"
        );
        assert_eq!(
            beat(&Phase::Mutants, 270),
            "mutants   [2871/5525] 52% · ✓ 2610 caught · ✗ 41 missed · ○ 214 not covered · ⏱ 6 timed out · 4m30s · ~21m left"
        );
        assert_eq!(
            beat(
                &Phase::Checking {
                    done: 18,
                    total: 52
                },
                30
            ),
            "checking  52 missed · 18/52 · 30s"
        );
        assert_eq!(heartbeat_line(&view(&Phase::Idle, &counts, &none, 1)), None);
    }

    #[test]
    fn whats_left_is_steady() {
        assert_eq!(left(Duration::from_secs(1260)), "~21m left");
        assert_eq!(left(Duration::from_secs(1289)), "~21m left");
        assert_eq!(left(Duration::from_secs(254)), "~4m10s left");
        assert_eq!(left(Duration::from_secs(41)), "~40s left");
        assert_eq!(left(Duration::from_secs(1)), "~5s left");
    }

    #[test]
    fn fitting_counts_what_shows_and_closes_the_colour() {
        let style = Style {
            color: true,
            ..Style::default()
        };
        let line = format!("{} and more", paint(style, GREEN, "✓ 10 caught"));
        assert_eq!(width(&line), 20);
        assert_eq!(fit(&line, 8), "\x1b[32m✓ 10 ca…\x1b[0m");
        assert_eq!(fit("short", 8), "short");
    }

    #[test]
    fn the_dump_says_what_each_worker_holds() {
        let (counts, workers) = (counts(), working());
        let held = [
            Some(Held {
                id: "m3c91e0a2",
                doing: "src/testrun.rs:114 spawn() call removed",
                elapsed: Duration::from_secs(164),
                expected: Duration::from_secs(40),
                slow: true,
                tests: 23,
            }),
            None,
        ];
        let lines = dump(
            &view(&Phase::Mutants, &counts, &workers, 1471),
            &held,
            Style::default(),
        );
        assert_eq!(
            lines,
            [
                "running   1 worker at 24m31s",
                "   1 m3c91e0a2  src/testrun.rs:114 spawn() call removed  [00:02:44] SLOW (expected 40s)",
                "               covered by 23 tests",
                "   2 idle",
            ]
        );
    }

    #[test]
    fn each_phase_ends_on_a_line_of_its_own() {
        assert_eq!(
            built_line(5525, Duration::from_secs(581), &["check", "build"], 0),
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
    }
}
