# Progress: what a run says while it runs

Both editions show progress the same way, byte for byte where the same thing happens. This file is
the contract; the Java repository keeps an identical copy. A change to a line's words changes both
editions in the same week.

Progress goes to stderr. stdout is the result and never carries progress.

## The rule

A run is never silent. Every phase gets a line when it starts or while it runs, and a line when it
ends; on a terminal a clock moves at least every 100 ms, and anywhere else a line is printed at
least every 30 seconds.

## Phases

`reading`, `building`, `coverage`, `mutants`, `checking`, then the result. The phase word sits in a
column 10 characters wide (`building  `, `mutants   `), as the existing lines have it.

| Phase | Starts with | Ends with |
|---|---|---|
| reading | `mutastic 1.1.0 · reading the Cargo package…` (Java: `the Gradle build…`, `the Maven build…`) | — |
| building | `building  5525 mutants in one build · 8 workers` | `built     5525 mutants in 9m41s · 2 rounds (check and build), none withdrawn` |
| coverage | — | `coverage  181 tests on rustc 1.94.0 in 7m23s (1 flaky, left out) · 5525 mutants` (as today) |
| mutants | — | each file's line as today, then `ran       5525 mutants in 34m12s · ✓ 5250 caught · ✗ 52 missed · ○ 214 not covered · ⏱ 9 timed out` |
| checking | — | `checked   52 missed mutants in 51.0s · a test to write for 41` |

`built`'s tail names the rounds (`1 round (check)` when the build stopped after checking, `3 rounds`
when mutants were withdrawn) and the withdrawn count (`none withdrawn`, `1 withdrawn`,
`12 withdrawn`). Java has no rounds: `built     5525 mutants in 9m41s`.

`checked` is printed only when there was something to check. `a test to write for N` counts the
missed mutants a test was found for; with none, `no test found to suggest`.

Durations use the summary's form: `840ms`, `3.5s`, `51.0s`, `9m41s`, `62m05s`.

## On a terminal: the live line

While a phase runs, a live block sits below the permanent lines and is redrawn in place every 100
ms by a ticker, not by events, so the clock moves even when nothing finishes. It starts with a
spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`, one frame per redraw) and the phase word, bold:

```text
⠴ reading   the Cargo package · 1s
⠴ building  cargo check, round 1 · 212 crates · 3m12s
   └ Checking ra_ap_syntax v0.0.301
⠧ coverage  72/181 tests · 4m02s
⠏ mutants   [2871/5525] ✓ 2610 caught · ✗ 41 missed · ○ 214 not covered · ⏱ 6 timed out · 24m10s
⠋ checking  52 missed · looking for a test that would catch each, 18/52 · 38s
```

The clock is the phase's own. Before the coverage pass has listed its tests it says `listing the tests`. `└` lines are dim, and show the build tool's latest line: Cargo's
`Checking`/`Compiling` for the last crate it started (its round counts check and build apart: `cargo build, round 2`), Gradle's or Maven's last log line. Java's
building line names the tool's task: `⠴ building  gradle testClasses · 3m12s`.

A permanent line (a phase's end, a file's line) clears the block, prints, and redraws the block
under it. The block is cleared before the result.

Clock form: `38s` under a minute, then `3m12s`, `1h02m`.

## Anywhere else: heartbeats

When progress isn't drawn in place, the same fields print as a plain line, without spinner or
colour, whenever 30 seconds have passed since the last progress line of any kind:

```text
reading   the Cargo package · 30s
building  cargo check, round 1 · 31 crates · 30s
coverage  [18/181 tests] · 1m00s
mutants   [312/5525] ✓ 290 caught · ✗ 3 missed · ○ 214 not covered · ⏱ 0 timed out · 4m30s
checking  52 missed · 18/52 · 30s
```

Phase lines and file lines print as on a terminal, plain. So the longest a log can go without a
line is 30 seconds, well inside an agent harness's idle timeout (often 180 s), and an hour-long run
writes about 120 heartbeats.

`MUTASTIC_HEARTBEAT=<seconds>` changes the 30 seconds, for tests and for logs that want fewer lines.

## Which one: `--progress`

`--progress=auto|bar|counter|none`, or `MUTASTIC_PROGRESS` when the option isn't given.

`auto` is `bar` when all of these hold, and `counter` otherwise:

- stderr is a terminal. (Java can't see stderr: the launcher passes `MUTASTIC_STDERR_TTY=1` or `0`
  under its `exec`, and the start script `[ -t 2 ]`. Without either, Java asks the console.)
- `TERM` isn't `dumb`.
- No CI is detected: `CI` set to anything but `false` or `0`, or `TF_BUILD`, `GITHUB_ACTIONS`,
  `GITLAB_CI`, `BUILDKITE`, `JENKINS_URL`, `TEAMCITY_VERSION` or `CIRCLECI` set. Some CI pretends to
  be a terminal.
- No coding agent is detected: `MUTASTIC_AGENT=1`, or any of `CLAUDECODE`, `CLAUDE_CODE`,
  `GEMINI_CLI`, `CODEX_SANDBOX`, `CODEX_THREAD_ID`, `OPENCODE`, `COPILOT_AGENT`, `COPILOT_CLI`,
  `CURSOR_AGENT`, `REPL_ID`, `JUNIE_DATA`, `GOOSE_PROVIDER`, `AUGMENT_AGENT`. Detecting an agent
  changes progress only, never stdout's format.

`counter` means heartbeats for every command, `run` included. `none` is silent.

`--format=agent` and `--format=jsonl` no longer silence stderr: they get `counter`, unless
`--progress` says otherwise. An agent's harness kills a process that's quiet for too long.

`NO_COLOR` turns colour off and nothing else.

## Testing

Frames are a pure function of the state, the phase's elapsed time and the spinner frame, and are
tested as text. Each edition has a maximum-silence test: a run with `MUTASTIC_HEARTBEAT=1` on a
fixture whose test sleeps through more than one heartbeat, stderr piped, every line timestamped.
It fails if two lines are further apart than the heartbeat plus a second, or if a phase passes
without a line.

## Later

P2 adds a bar, a weighted percent, the estimate, a line per worker with SLOW tags, the `t` dump,
live JSONL `phase-started` and `progress` events, width handling, and the summary ending on the
score. P3 adds OSC 9;4 tab progress, synchronized output and Gradle's footer. A preview of all of it:
https://claude.ai/artifact/EXHy5dovr98tnBYAfBXn2M
