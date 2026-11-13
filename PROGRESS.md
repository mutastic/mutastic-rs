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
⠧ coverage  ━━━━━━━━━━━━━━━━──────────────────────────  72/181 tests · 4m02s
   1 [00:03:51] SLOW  docs::the_docs_show_what_the_operators_print
   2 [00:00:41]       since::changed_lines_are_read
   3 idle
     t for what’s running
⠏ mutants   ━━━━━━━━━━━━━━━━━━━━━───────────────────  52% · 2871/5525 · ✓ 2610 caught · ✗ 41 missed · ○ 214 not covered · ⏱ 6 timed out · 24m10s · ~21m left
   1 [00:00:04]       src/engine.rs:540 finished() && becomes ||
   7 [00:02:31] SLOW  src/testrun.rs:114 spawn() call removed
   8 idle
     … and 4 more running · t for what’s running
⠋ checking  52 missed · looking for a test that would catch each, 18/52 · 38s
```

The spinner is green (`ESC[32m`), the phase word bold (`ESC[1m`, its padding not), the `└` line
dim (`ESC[2m`, the whole line), each closed with `ESC[0m`. The clock is the phase's own. Before the coverage pass has listed its tests it says `listing the
tests`. `└` lines are dim, and show the build tool's latest line: Cargo's `Checking`/`Compiling`
for the last crate it started (its round counts check and build apart: `cargo build, round 2`),
Gradle's or Maven's last log line. Java's building line names the tool's task:
`⠴ building  gradle testClasses · 3m12s`.

**The bar and the percent** (coverage and mutants) are green `━` on dim `─`, at most 40 wide. In the
coverage pass they count tests. For mutants they're weighted: each mutant counts for what its
covering tests took in the coverage pass, and one no test reaches counts for nothing, so the bar
moves with the time that's passed rather than the number of mutants.

**A file's line** gives the time from its first mutant's start to its last one's end, not the sum
of its mutants' times across the workers.

**What's left** (`~21m left`) is the mutant phase's time so far, scaled by the cost still to run over
the cost done. It shows once three mutants have run and ten seconds have passed. It rounds to whole
minutes above ten minutes, to ten seconds above one, and to five below.

**Worker lines**, one per worker, with a `[HH:MM:SS]` timer, `SLOW` (bold amber) and what it runs: a
test's name, or a mutant as `file:line function() change`, its change in the words a miss line uses.
A mutant is SLOW at three times what its tests took, or at half its timeout, whichever comes first,
and never before two seconds, so a mutant whose tests take milliseconds isn't marked for being a
little slower. A test in the coverage pass is SLOW after a minute. Up to eight show, four below 100 columns, then
`… and K more running`. The block keeps its height while workers come and go: an idle one says
`idle`.

**Width.** Every line fits the terminal: the bar shrinks first, from 40 down to 12; then the fates
drop words, `✓ 2610 caught · ✗ 41 missed · ○ 214 · ⏱ 6`, then all of them,
`✓2610 ✗41 ○214 ⏱6`, with the bar down to 6; then a line is cut with `…`. Wrapping is turned off
(`ESC[?7l` … `ESC[?7h`) while the block is drawn, so a terminal narrowed mid-run cuts a line rather
than wrapping it where the next redraw can't clear it. The Rust edition asks the terminal; Java
reads `MUTASTIC_COLUMNS`, which the launcher passes, or `COLUMNS`, or assumes 100.

A permanent line (a phase's end, a file's line) clears the block, prints, and redraws the block
under it, in one write, so the terminal never shows the block half gone. The block is cleared
before the result.

Clock form: `38s` under a minute, then `3m12s`, `1h02m`.

## The tab: OSC 9;4

Where the terminal draws progress in its tab or taskbar, the run shows it there too, with
`ESC ] 9 ; 4 ; state ; percent ESC \`: busy (state 3) while reading, building, covering and
checking; the mutants' weighted percent (state 1) while they run; red (state 2) from the moment
the score, over the mutants run so far and once twenty have, falls below the threshold, and red
it stays; and nothing (state 0) on every way out: the result, an error, Ctrl-C, a kill or Ctrl-Z.
Missed mutants are normal, so a miss alone never turns it red. With `--fail-on-new` the score
doesn't decide, and it never does.

It's sent only to terminals that say they draw it, as some take `ESC ] 9` for a notification:
Windows Terminal (`WT_SESSION`), ConEmu (`ConEmuANSI=ON`), Ghostty (`TERM_PROGRAM=ghostty` or
`TERM=xterm-ghostty`) and WezTerm (`TERM_PROGRAM=WezTerm`). `MUTASTIC_TERM_PROGRESS=true|false`, or
Cargo's `CARGO_TERM_PROGRESS_TERM_INTEGRATION`, decides instead.

## Flicker: synchronized output

On a terminal, each frame (the block cleared, any permanent lines, the block drawn again) is one
write wrapped in `ESC [ ? 2026 h` … `ESC [ ? 2026 l`, so a terminal that knows synchronized output
paints it at once. One that doesn't ignores the two sequences.

## What's running: `t`, Ctrl-T, SIGUSR1

On a terminal that's also stdin, `t` prints what's running above the live block, as does Ctrl-T
(SIGINFO, on macOS and the BSDs) and `kill -USR1` anywhere on Unix, piped or not:

```text
running   8 workers at 24m31s
   7 mb2f8ae57  src/lib.rs:26 discount() >= becomes >  [00:02:44] SLOW (expected 40s)
               covered by 23 tests
   8 idle
```

During the coverage pass it lists the running tests with their timers; during the build, Cargo's
round, its crates done and its last line; while checking, how many it has looked at. To hear `t`,
the terminal reads a key at a time without echo while the run lasts. Ctrl-C and Ctrl-Z still work,
and every way out (finishing, an error, Ctrl-C, a kill, Ctrl-Z) leaves it as it was.

## The result ends on the score

The summary's last line is its score, with the verdict beside it, so the line the cursor lands
under is the one that matters. `Next:`, the commands for the misses, moves above the rows, under
the misses it's about. The Duration row names every phase it timed.

```text
 Next: mutastic verify <id> fixtures/pricing · mutastic run --only=<id> fixtures/pricing · mutastic explain <id> fixtures/pricing

     Files  0 clean | 1 with misses (1)
   Mutants  ✓ 10 caught · ✗ 1 missed (11)
  Duration  355ms (setup 20ms, build 303ms, coverage 2ms, mutants 8ms, checking 6ms)
     Score  90.9%  [PASS] 1 missed, above the threshold of 80.  = vs last run (same code)
```

The verdicts: `[PASS] Every mutant was caught.`, `[PASS] 1 missed, above the threshold of 80.`,
`[FAIL] Below the threshold of 80: 6 missed, 0 not covered.`, `[PASS] Nothing to measure: no
mutants to score.`, and with `--fail-on-new`, `[PASS] No new misses: 1 in the baseline.` or
`[FAIL] 1 new miss, not in the baseline (0 in it): mb2f8ae57.` In colour `[PASS]` is ` PASS ` on
green, `[FAIL]` on red.

## Anywhere else: heartbeats

When progress isn't drawn in place, a plain line, without spinner, bar or colour, prints whenever
30 seconds have passed since the last progress line of any kind. It's the live line's fields in
their short form: a count in brackets where there's a total (`[18/181 tests]`, `[312/5525]`), and
checking's count without its step's words:

```text
reading   the Cargo package · 30s
building  cargo check, round 1 · 31 crates · 30s
coverage  [18/181 tests] · 1m00s
mutants   [312/5525] 4% · ✓ 290 caught · ✗ 3 missed · ○ 214 not covered · ⏱ 0 timed out · 4m30s · ~21m left
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

Frames are a pure function of the state, the phase's elapsed time, the spinner frame and the width,
and are tested as text, at 160, 120 and 80 columns. Each edition has a maximum-silence test: a run with `MUTASTIC_HEARTBEAT=1` on a
fixture whose test sleeps through more than one heartbeat, stderr piped, every line timestamped.
It fails if two lines are further apart than the heartbeat plus a second, or if a phase passes
without a line.

## Live events: `--format=jsonl`

Three events join `mutastic.events/v1`, additive, so a consumer that ignores reasons it doesn't
know reads the stream as before. They go to stdout as they happen, among the others:

```json
{"schema":"mutastic.events/v1","edition":"rust","reason":"phase-started","phase":"building"}
{"schema":"mutastic.events/v1","edition":"rust","reason":"progress","phase":"mutants","elapsedMs":1450000,"done":2871,"total":5525,"weighted":0.52,"leftMs":1260000,"caught":2610,"missed":41,"notCovered":214,"timedOut":6,"running":8}
{"schema":"mutastic.events/v1","edition":"rust","reason":"phase-finished","phase":"building","durationMs":581000}
```

- `phase`: `reading`, `building`, `coverage`, `mutants` or `checking`.
- `phase-started` when a phase begins; `phase-finished`, with `durationMs`, when it ends.
- `progress` on the heartbeat's cadence (30 s, `MUTASTIC_HEARTBEAT`), whatever the format of
  stderr, with what the phase knows: always `elapsedMs` (the phase's own); `done` and `total`
  where there's a count (tests in coverage, mutants, missed mutants looked at in checking);
  `round` and `crates` while Cargo builds; for mutants, `weighted` (0 to 1, three decimals),
  `leftMs` once there's an estimate, the four fates' counts, and `running`, the busy workers.

`run-started` stays where each edition has it (Rust's carries the build's profile, so it comes once
the build is done), and the events before it are phase events only.

## Later

Gradle's footer is the Java edition's alone: the plugin follows the run's live events and drives
Gradle's own progress line, falling back to heartbeat lines where Gradle's internal API isn't there.
In the Rust edition the `mutant-finished` events still come at the end, after the phases', since
each carries what the checking phase found. A preview of all of it:
https://claude.ai/artifact/EXHy5dovr98tnBYAfBXn2M
