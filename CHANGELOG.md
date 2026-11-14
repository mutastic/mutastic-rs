# Changes

What changes from one release of mutastic for Rust to the next, written as changes land. The
newest come first.

## Unreleased

### Security

- **Text from the project reaches an agent fenced off as data.** The agent format, `next`'s agent
  text and the gate's messages put mutastic's instructions (`rules:`, `done when:`, `fix:`) in the
  same text as the project's code and test run: the lines a mutant changes, test names, file
  paths. A string literal or a comment on a mutated line, a test's name or a vendored file could
  write a line an agent takes as an instruction, and agents read this text through the plugins'
  hooks. The project's text now comes after mutastic's instructions, between
  `<<mutastic-data 3f9a0c1e>>` and `<<end mutastic-data 3f9a0c1e>>`, with a token drawn at random
  for each run (and drawn again if the text holds it), so nothing inside can close it early; the
  opening line says everything up to the closing marker is data, not instructions. The lines inside
  keep their shapes. See [Agent output and untrusted
  text](docs/concepts/output.md#agent-output-and-untrusted-text).
- **Hidden characters show as escapes.** Every value and line of the project's that mutastic
  prints, to an agent or to a terminal, shows control characters (ANSI escapes among them) as
  `\x1b`, and bidi controls, zero-width characters and every other invisible format character as
  `\u202e`: a Trojan Source line reads as it runs, and an escape can't rewrite the screen. A value
  or line over 200 characters is cut, with `…(n more chars)`. Events keep the text as it is,
  JSON-escaped.
- **Commands come only from mutastic.** Every command offered (`verify`, `run --only`,
  `ignore --propose`, `next`) is made of a mutant's id (`m` and 8 hex digits, or `<id>`), known
  options and placeholders, and the directory, in single quotes when a shell needs them. The
  commands that put a word back (`ignore`'s `to undo:`) quote it in single quotes too, as the Java
  edition does, where double quotes let a shell run `$(…)` in a reason read from
  `.mutastic/ignores.yaml`.

### Additions

- **A run with no mutant to run builds nothing and runs no test.** `mutastic run --since=HEAD` on
  a clean tree took a build and a pass of every test, a minute and more on a big crate, to say
  there was nothing to measure. When no mutant is there to run (no changed lines, changed lines
  with no mutant, `--only` with skipped mutants only, a share that holds none, or a package with
  none), mutastic now finds the mutants, lists the skipped ones with their reasons, and passes in
  a second or so. The summary says `Build  nothing built: no mutant to run` and `Tests  none ran:
  no mutant to run`, with no test runs row; `run-finished` has `tests` 0 and no build rounds, and `run-started` comes with the results and leaves out the `optLevel` Cargo
  didn't report. Such a run leaves the last run's record and report files as they were, unless a
  `--report` is asked for, so `explain` and the next run's comparison still read the last run
  that measured something. The gate still runs the tests: see the fixes.
  `--force-coverage` builds and runs the tests anyway, as before, to check or time the suite. See
  [When there's nothing to run](docs/concepts/how-a-run-works.md#when-theres-nothing-to-run).
- **An error is an event too.** In `--format=jsonl` and `--format=agent`, when mutastic can't run,
  the last line on stdout is a `run-failed` event, with the error's `code`, its `message`, the
  commands it ends in as `help`, each with whether it runs as it is, has a `<…>` to fill in, or is
  a guess to check, the `exitCode`, and the error as stderr has it in `rendered`. A tool that reads
  only stdout was left with nothing. See [When mutastic can't
  run](docs/reference/events.md#when-mutastic-cant-run); the schema's copy has it.
- **Each error has an entry on [the errors page](docs/reference/errors.md),** under its code: what
  causes it, a kept copy of a real run, and what to do. Each command an error gives to run as it is,
  is run by the docs test, and gets past the error.
- **A value can follow its option after a space,** `--format agent` as `--format=agent`, for every
  option that must take a value, as the Java edition has taken it since 1.0. It was refused with
  "takes its value after an =". Only `--since`, whose value is optional, still needs its `=`, as
  the word after it may be the directory.

### Changes

- **A bad value in an environment variable is refused,** with code `environment`, as the Java
  edition refuses it: `MUTASTIC_FORMAT`, `MUTASTIC_VOCABULARY` or `MUTASTIC_PROGRESS` set to a
  value its option doesn't take, or `MUTASTIC_HEARTBEAT` to anything but a number of seconds above
  0. It was read as the default without a word, and a variable is set once and forgotten. The
  option on the command line still wins over it, unread.
- **`--threshold` is a whole number from 0 to 100,** as the events schema, mutation.json and the
  Java edition have it. `--threshold=72.5` and `--threshold=150` were taken; they're refused now,
  in the Java edition's words: `--threshold needs a whole number, not 72.5`, `--threshold is from
  0 to 100, not 150`.

### Fixes

- **A property test draws the same cases in every run.** mutastic sets `PROPTEST_RNG_SEED` for
  the tests it runs, unless you've set it, and turns proptest's saved failures off. A property that
  fails for some cases fails in every run or in none, so it can't stop one run, be called flaky in
  the next and catch a mutant in a third; and a case one of mutastic's runs saved can't decide its
  next runs. `proptest-regressions/` isn't copied, so the cases you've saved aren't replayed, and
  nothing is written to your tree.

- **The gate knows a case by the name it runs under.** rstest and test-case give each case a test
  of its own, `halves::case_1`, and no function has that name. A case that failed at a check was
  remembered without its source, and once it passed the gate said it was gone, at every check,
  until `gate close`. The gate now looks tests up by the names the test binaries list, and counts
  them from the same list, so taking a `#[case]` row away holds the gate as taking a test-case row
  away did. `gate open` builds the tests once to list them; a package that doesn't build when the
  task starts is counted from its source, as before.

- **A test that caught mutants and fails on its own fails the run.** Once the mutants have run,
  each test that caught one runs again, on its own with no mutant in. One that fails there (it
  trips on a file it, another test or the run left behind) is named as flaky, as a test that
  "passes among the others but fails on its own"; its catches don't count, and every mutant it
  reached runs again without it, so scores can go down. Unlike other flaky tests it fails the run
  whatever the score, with exit status 2, and `verify` and `gate check` don't pass on it either.
  In `--format=jsonl`, a mutant run again comes as a `mutant-revised` event after the
  `mutant-finished` ones, naming the test as `falseCatchBy` where its catch didn't count, and the
  test's `flakyEvidence` entry has `"failsAlone": true`, as in the Java edition. A healthy suite's
  results don't change, at the cost of one run of each catching test. No id changes. See
  [A test that fails on its own](docs/concepts/results.md#a-test-that-fails-on-its-own).
- **The gate holds a task that changes no line mutastic mutates to its other checks.** Such a task
  skipped the checks for ignores and baseline entries the task made itself, and for tests taken
  away or `#[ignore]`d, so an agent that only deleted a failing test, or ignored a mutant itself,
  passed once the rest of the tests did. Now they run on every check, whatever lines changed, with
  the same words and the same hand-over; only a task that changed nothing passes at once, and
  an ignore or baseline entry it made itself holds it even where git doesn't see `.mutastic/`.
  An ignore you accept between turns is still the task's to start from.
- **The gate runs the tests even with no mutant on the changed lines.** Since a run with no mutant
  to run stopped building and running the tests, a task that changed only lines with no mutant (a
  constant, an import, a log line) passed the gate with no test run, even when it broke one. The
  gate's run now always builds and runs the tests, as `--force-coverage` does, and a test that
  fails holds the task as a red suite does, with the same words and the same hand-over. A task
  that changed only files mutastic doesn't mutate (a test's data, `Cargo.toml`) runs the tests
  too; only one that changed nothing skips them. A plain `run --since` keeps the fast path.
- **An ignore you accept between turns doesn't hold the gate.** The gate counted the ignores from
  a session's first prompt, so a proposal you accepted and then prompted again counted as the
  task's own (`the task ignored … itself`). Now, as a hook, a check that lets the agent stop notes
  the ignores, and the next prompt takes in those added since: yours, made between turns. One the
  agent adds in its turn, with `ignore --accept` or by editing `.mutastic/ignores.yaml`, still
  holds the gate after your next prompt.
- **A closed stdout ends the run quietly.** `mutastic run | head -1` panicked and exited 101 when
  `head` closed the pipe; it now stops writing and exits as the run would.
- **Refusals are said on stderr, and stdout holds only results.** `baseline` with one already, or
  none to prune, `ignore --accept` or `--remove` on a mutant that isn't proposed or ignored, and
  `gate check` with no task open said so on stdout. `ignore --remove` on a mutant that isn't
  ignored now exits 1, as `--accept` does.
- **A directory that isn't there is said, with where mutastic looked.** `mutastic run mutastic-rs`
  in a workspace with no such directory said `couldn't run cargo: No such file or directory`. The
  directory is now checked before Cargo runs: mutastic says `there's no directory mutastic-rs in
  <where it looked>`, offers the one package or directory whose name is close to it, and ends in
  the command to run, as `mutastic run crates/mutastic-rs`. A member's package name typed alone,
  where there's no directory of that name, runs that member, and a note says which directory it
  used. When there's really no `cargo`, mutastic says so, and where it looked: the `CARGO`
  variable, when it's set, or the PATH.
- **A workspace's root gives a command for each member.** It named the packages only; now it ends
  in `mutastic run <dir>` for each, from where you are, up to ten, and past that, their names and
  the command to fill in. See [A workspace](docs/troubleshooting.md#a-workspace).
- **Inside a member, mutastic runs that member, as Cargo would.** From a member's `src`, mutastic
  said the directory was a workspace and to run in one of its packages. It now runs the package
  that holds it, and reads its `mutastic.yaml`, ignores, baseline and `--since` from the package's
  directory.
- **A word such as `-v` is an option mutastic doesn't have,** said with `mutastic run --help`, not taken
  for the directory, which ended in the same `couldn't run cargo`.
- **`run --help` prints run's help, and runs nothing.** `--help` or `-h` after any command, or
  `mutastic help run`, prints that command's words, options and exit status. It started a whole
  run before.
- **A mistyped command, option, value or id is answered with the one close to it, and the line
  again.** `mutastic run --formt=agent` says `run has no option --formt. There's one called
  --format.` and ends in `mutastic run --format=agent`: the line as typed, with only that word
  changed. The close name is found by an edit distance that counts two letters swapped as one
  edit, so `rnu` finds `run`; it's only offered, never run. A mutant's id is offered only one digit,
  or one swap, away. An unknown option or command gets one line of usage and the `--help` to run,
  where it got the whole help.
- **A value after a space says to use `=`.** `--format agent` said `--format:  isn't one of …`,
  and took `agent` for the directory. It now says `--format takes its value after an =, not a
  space` and ends in `mutastic run --format=agent`; `--since main` says the same, unless `main` is
  a directory.
- **`mutastic.yaml`, rules files and the ignores name the line, and what's valid there.** An
  unknown section offers the close one (`rulez:` finds `rules:`); an operator that isn't one names
  its line, the close name, the sets and namespaces, and `mutastic mutants --operators=all` to see
  each id; a plugin that isn't there names the line that names it; an unknown field in a rules file
  lists the fields; and an ignores entry that can't be read names its line and what it lacks.
- **A package that doesn't build is shown where you wrote it.** mutastic printed rustc's errors
  on its own instrumented copy, with its columns and its injected code. When the copy fails and
  no mutant is to blame, mutastic now checks the package as it is, names each error by your file
  and line, `src/lib.rs, line 27: cannot divide `u32` by `&str``, and ends in the command that
  shows them in full, `cargo test --no-run -p pricing`. If the package builds and only the copy
  doesn't, it says that's a bug in mutastic, with the copy's error to report.
- **A test that fails with no mutant in says how, and how to run it alone.** It named the test
  and the log. It now gives the panic's message, why a test that passes in `cargo test` can fail
  here, where the log is from where you ran mutastic, and the command that runs just that test,
  as `cargo test -p pricing --lib -- --exact tests::shipping_is_free_from_the_threshold`.
- **A package with no tests says so.** Its run ended `Below the threshold of 80: 0 missed, 2 not
  covered`, as if its tests missed. It now ends `Found no tests, so no mutant can be caught`, with
  what mutastic runs; the exit status is still 1.
- **No project here is said in both editions' terms.** In a directory with no build file in it or
  above it, mutastic answered with Cargo's `could not find Cargo.toml` alone, which says nothing to
  someone with a Maven or Gradle build. It now says where it looked, for `Cargo.toml`, `pom.xml`,
  `build.gradle(.kts)` and `settings.gradle(.kts)`, and ends in `mutastic run <project directory>`.

### Internal

- **The unsafe code has a crate of its own,** `crates/mutastic-terminal`: the C calls for the
  terminal's width, its input mode and signals, which were `mutastic`'s `terminal` module. The
  workspace now forbids unsafe code rather than denying it, so no `#[allow]` can let it back in
  elsewhere. In the crate, each `unsafe` block holds one operation and says why it's sound, and the
  logic over the calls is tested against a fake. A nightly workflow runs its tests under Miri, for
  each platform it lays out C's structs for, and under AddressSanitizer, the C calls included.
  Nothing it does changes on Linux, macOS or Windows, and the binary takes no new dependency.
- **The terminal's settings are read as each C library lays them out.** On FreeBSD, `struct
  termios` holds `unsigned int` flags, not `unsigned long` as on macOS, and on Android it's shorter
  than glibc's: mutastic read past what the C library wrote, which is undefined behaviour. Neither
  is a platform mutastic ships for. Tests now hold each struct and constant to the `libc` crate's.
  Unix platforms whose layouts aren't checked, such as Linux on MIPS, PowerPC or SPARC, NetBSD,
  OpenBSD and illumos, now get no width, no `t` key and no Ctrl-T, as any other platform without
  them, rather than the wrong constants.

## 1.1.0 (2026-11-12)

### Card changes

- **A suggested test is named by where its input sits, not its value,** as in the Java edition. A
  call of one argument is named for zero or a negative input, `fee_when_amount_is_zero`, or for
  the edge of the literal its condition compares with, `discount_at_the_total_edge` and
  `discount_just_below_the_total_edge`; a `const` the source names is used as before; anything
  else keeps its values. Two mutants on a card suggesting one call get one test.
- **Each suggested test says what it's for**, as in the Java edition. A card with a test for more
  than one of its mutants gave only the first mutant's fix, so the second test came with nothing
  saying why. Now `help:` says `one test for each mutant`, and each test follows its own mutant's
  fix and id; the agent format's `fix:` does the same, with a `fix for <id>:` line before each
  test.
- **`explain` says `run` or `next` records a last run.** With none kept, its `last run` row said a
  full run records one; `next`'s first pass now does too, and the row says so in the Java edition's
  words.
- **Never change production code to make a mutant go away.** The agent format's `rules:` line now
  says `add or change tests only; never change production code to make a mutant go away: tell the
  person`, and the gate's open and block messages say the same, as in the Java edition. Production
  code changed to pass a test the spec asks for is the task; code changed so a mutant can't exist
  hides what the mutant found, and was the one cheat mutastic's own evals saw.
- **No suggested test of a crate's own function on an input its callers never gave.** A function
  that isn't `pub` (or a trait impl's) gets its inputs from the crate's callers, so a test calling
  it alone on a value none of them passed may test what can't happen: an agent was offered
  `fee(-1)` where the only caller passes a `u8`. Its card now offers no such test. It says no
  caller passed the value, and to test a caller that can, or, if none can, to propose the mutant
  unreachable.

### Scoring changes

- **A package uses its nearest `mutastic.yaml`, however it's run,** as in the Java edition. A run
  on a workspace's top read only the top's file, while `mutastic run shop` read only
  `shop/mutastic.yaml`, so the same package could get two scores. Now each reads
  `shop/mutastic.yaml` for `shop` when there is one, and the nearest above it, up to the git
  repository's top, when there isn't. A package without a file of its own, in a repository with one
  at its top, now has the top's rules applied.
- **A zero check on an unsigned value skips what differs only below 0.** `n == 0` made `n <= 0`, or
  `n > 0` made `n != 0`, can't be told apart where `n` can't be negative, and is skipped as
  equivalent where the source proves it: a type written `u8` to `u128` or `usize` (parameter,
  `let` or field), a cast, a function declared beside it returning one, an `.enumerate()` index,
  a `let` from a literal with an unsigned suffix, a `Duration`'s whole-number count where the
  source names std's `Duration`, or arithmetic on those. Scores can go up; no id changes.
- **`cfg!(debug_assertions)` forced true is skipped as equivalent.** mutastic builds every mutant
  with debug assertions on, so under it the condition is always true, and forcing it true changes
  nothing a test can see. Forced false still runs. Measured on httparse: 3 of its misses, 3 of 3
  this. Scores can go up; no id changes.
- **A boundary where both sides give the same value at the edge is skipped as equivalent,** as in
  the Java edition: `if x > max { max = x }`, `if a < b { a } else { b }`,
  `if fee < 10 { 10 } else { fee }` and `if x < 2 { x = 2 }`, on whole numbers the source declares.
  Those mutants, which no test could catch, no longer count as missed, so a score can go up: a
  change to scores, as a minor release may make. See
  [Conditions](docs/operators/conditions.md#boundary).
- **A zero check on a `NonZero*` is skipped as equivalent where it can't differ.** In
  `if parts.get() == 0` with `parts` a `NonZeroU32`, the guard made never true behaves as the code
  does, since `parts` can't hold 0, so it's skipped with its reason (`equivalent: differs only when
  parts.get() is 0. That can't happen because parts is a NonZeroU32, which can't hold 0.`), and so
  is any other change to a comparison of a `NonZero*` with 0 that differs only at 0. It takes a
  parameter, a `let` with its type written or a field, of one of std's `NonZero*` types or a
  `NonZero<u32>`-style one; an inferred or generic type, or one from elsewhere, still runs. Those
  mutants, which no test could catch, no longer count as missed, so a score can go up: a change to
  scores, as a minor release may make. See
  [Remove conditional](docs/operators/conditions.md#remove-conditional).
- **A boundary inside a guard that its sides differ is skipped as equivalent.** In
  `if ver.major != cmp.major { return ver.major > cmp.major; }`, `>` made `>=` differs only when
  the two are equal, which the `if` rules out, so it's skipped with its reason (`equivalent: differs
  only when ver.major is cmp.major. That can't happen because the if ver.major != cmp.major around
  it rules it out.`), and so is any other change to that comparison that differs only when its sides
  are equal. The guard can be the `if` around it, the `else` of an `if a == b`, an `if a == b`
  before it that returns, or the left of an `&&`. Each side must be a whole number that can't
  change on the way: a parameter, a `let` or a `Some(x)` bound once without `mut`, a field of one,
  or a literal; a float, an inferred or generic type, or a `mut` binding still runs. On semver
  1.0.28 that's 10 boundary mutants no test could catch, in the defaults, that no longer count as
  missed, so a score can go up: a change to scores, as a minor release may make. See
  [Boundary](docs/operators/conditions.md#boundary).

### Additions

- **`mutastic baseline` and `run --fail-on-new`, as in the Java edition.** `baseline` records every
  mutant no test catches now in `.mutastic/baseline.yaml`, by id, to commit, in the Java edition's
  shape; it won't write over one, and `baseline --prune` drops the mutants the tests catch now,
  never taking in new ones. From then on a miss that isn't in it is new: `next` offers new misses
  first, with a `baseline` factor and `next-finished`'s `legacy`; the gate holds only new misses,
  and blocks a task that adds to the baseline itself; and a full run says how many of the
  baseline's mutants are caught now. `run --fail-on-new` passes while every mutant no test catches
  is in the baseline and fails on one that isn't, whatever the score, naming each new miss:
  `[FAIL] 1 new miss, not in the baseline (0 in it): mb2f8ae57.`, the same on the agent format's
  first line, and `failOnNew`, `newMisses` and `inBaseline` in `run-finished`. Without a baseline
  it exits 2 before the build. See
  [Brownfield: fail only on new misses](docs/concepts/how-a-run-works.md#brownfield-fail-only-on-new-misses).
- **`explain --format=jsonl`**, as in the Java edition: the run's events for the one mutant, then
  a `mutant-explained` event with what "About this mutant" says, a field for each row: the
  change as it's `applied` (at the `source` level, as `mutation.applied` says it), the `operator`, the `fingerprint` and how the id is
  made, the `site` and the `lastRun`. `explain` takes `full` and `jsonl`, and refuses another.
- **`extends:` in `mutastic.yaml`, as in the Java edition.** A package's file adds another's rules
  and plugins when it says `extends: ../mutastic.yaml`; its own `operators:`, if any, replace the
  other's. A file names one other, which may extend another in turn. See
  [In a workspace](docs/reference/configuration.md#in-a-workspace).
- **`mutastic next`: the misses one at a time.** A long list of missed mutants goes unread, so
  `next` hands over the one most worth a test first: its card, why it's first, the test to write
  and the `done when` that proves it, as in the Java edition. The queue is in the run's order (a
  condition or a removal first, then the line more tests run, then where it is), a line's mutants
  are one item, and `--top=N` shows the first few, at most one a function and two a file. While
  the code under test is the same, `next` runs only what the last one left uncaught, so a new test
  in `mod tests` doesn't run everything again; a mutant proposed for ignoring waits on a person,
  not in the queue. `--format=agent` gives exactly one task; `--format=jsonl` writes `next-item`
  and `next-finished`. It exits 1 while something is queued and 0 when nothing missed is left.
  Where there's a baseline, new misses come first. See
  [Working through misses](docs/concepts/next.md).
- **The output options the Java edition has.** `--format=brief` (each card's head: the code with
  the change marked and what the run saw) and `--format=grouped` (the missed mutants of each file
  by function, a line each); `--vocabulary=classic` (killed and survived, in what a person or an
  agent reads, never in the events) and `MUTASTIC_VOCABULARY`; `--color=auto|always|never`; and
  `--hyperlinks=auto|always|never`, file locations as links a terminal opens. See
  [Output](docs/concepts/output.md#formats).
- **`--report=sarif`, for code scanning, as in the Java edition.** It writes `mutation.sarif`,
  SARIF 2.1.0: a result for each missed mutant (a warning) and each one no test reaches (a note),
  on its line, with the card's headline, keyed by the mutant's id in `partialFingerprints` so an
  alert follows its mutant; never a fix. Paths are from the git repository's root. GitHub shows it
  as alerts on a pull request's lines. See [Report files](docs/reference/reports.md#code-scanning).
- **Links to each operator's docs, as in the Java edition.** `explain`'s About this mutant gives
  the operator's section in the docs, `docs:
  https://docs.mutastic.io/rust/operators/conditions/#boundary`. On a terminal with links on
  (`--hyperlinks`), a full card's `^^ changed to` mark links there too, and `--format=jsonl`'s
  `mutant-finished` carries it as `docs`. A rules file's operator has no section, so no link. See
  [One mutant, explained](docs/concepts/output.md#one-mutant-explained).

### Changes

- **Each full run is compared with the last, as in the Java edition.** A run keeps what it found in
  `target/mutastic/last-run.tsv` (each mutant's result and catching test, the score, and hashes of
  the code and of mutastic), and the next run under the same operators compares with it: the score
  line says `= vs last run`, `▲ 2.0 vs last run` or `▼ …`, with `same code`, `code changed` or
  `mutastic changed`. With the code and mutastic unchanged, a mutant that changed result names the
  test that caught it as `! likely flaky: …`. The agent format adds `vs last run:` and `likely
  flaky:` lines, and `run-finished` fills `previousScore`, `sameCodeAsLastRun`,
  `sameMutasticAsLastRun`, `changedSinceLastRun` and `likelyFlaky`, as the events schema
  describes. A run of one mutant or of a change neither compares nor is kept, and a share keeps
  its own. See [Compared with the last run](docs/concepts/how-a-run-works.md#compared-with-the-last-run).

### Fixes

- **Two runs on one package wait for each other.** Run at once, they rewrote each other's copy and
  build in `target/mutastic/` in the middle of a run, so a mutant could be judged by the other
  run's build. The first run now holds the package's work directory until it ends, and a second
  says it's waiting and runs after it.
- **A closed stderr no longer loses a run.** `mutastic run … 2>&1 | head` closed stderr under a
  run, and its first progress line stopped it with a panic. Progress and notes are now dropped
  when they can't be written, and the run goes on. A worker thread that panics now fails the run;
  before, its mutants' results went missing and the rest were kept as if it were whole.
- **A workspace that excludes a package its tests run Cargo on can be mutated.** mutastic's copy
  of the workspace is under its target directory, so Cargo, finding a package the copy's workspace
  excludes, looked further up, reached the workspace itself and refused it: the tests failed in the
  copy before any mutant, and the run stopped. In the copy, such a package now stands alone, as it
  does in the workspace; a package of a workspace nested in it is left as it is.

## 1.0.0 (2026-11-10)

### Breaking changes

Before 1.0.0 a change that breaks a reader is made outright, with no deprecation. From 1.0.0 the
events format only gains.

- **Events: the old names are gone.** The names kept beside their replacements are no longer
  written: `run-started`'s `rules` (read `rule`), a mutant's `killedBy` and `killedWith` (read
  `caughtBy` and `caughtWith`), `run-finished`'s counts keyed by status (`Killed`, `Survived`,
  `Timeout`, `NoCoverage`, `Ignored`, `RuntimeError`; read `killed`, `survived`, `timeout`,
  `noCoverage`, `ignored`, `runtimeError`), and a scoped run's `scope.files` and `scope.lines`
  (read `scope.fileCount` and `scope.lineCount`). The schema now requires what every run writes:
  `edition` on every event, `rule`, a mutation's `applied`, a missed mutant's `ignore` and
  `propose`, the counts, `toolchain`, `workers`, `workersStarted` and `coverageMs`. See
  [Events](docs/reference/events.md).
- **The summary counts an ignore by the sort it was given,** as the Java edition does:
  `1 skipped (1 ignored as equivalent)`, or `1 ignored` when it gave none, where it was counted
  with the project's other decisions as `1 user`. Rules in `mutastic.yaml` are still `user`. The
  events are unchanged: the mutant's `skipKind` is `user`, and `run-finished`'s `ignored` counts
  every skipped mutant.

### Changes

- **Windows has release builds and a one-line installer.** Each release now has x64 and Arm64
  Windows zips, which need nothing else installed. In PowerShell, `irm
  https://mutastic.io/install.ps1 | iex` installs one, checked against `SHA256SUMS`, and puts it on
  the user `Path`; in Git Bash, MSYS2 or Cygwin, `install.sh` installs the same build into the same
  folders, where it stopped with "no release for MINGW64_NT". Each release also has a Scoop
  manifest, `mutastic.json` (`scoop install
  https://github.com/mutastic/mutastic-rs/releases/latest/download/mutastic.json`), and `cargo
  binstall --git https://github.com/mutastic/mutastic-rs mutastic` installs a release's build. In a
  Maven or Gradle project on Windows, `mutastic` now fetches and runs the Java edition, where it
  tried to start its shell script. WSL is no longer needed. See [Installing](docs/install.md#on-windows).
- **What 1.0.0 promises is written down, and held.** A [Compatibility](docs/reference/compatibility.md)
  page says what stays the same between releases: mutant ids, scores (a patch never moves one),
  commands and configuration, the events format, and the agent format's structure. From 1.0.0 the
  tests fail on a command, option, choice, variable or exit status the last 1.0+ release took and
  this one doesn't, and on a `mutastic.yaml` or rules file it reads differently.
- **An agent's proposal carries its kind and a reduction.** The agent format's `or propose` line
  is `mutastic ignore <id> --propose --as=<equivalent|unreachable|…> --reason="Differs only when
  <case>. That can't happen because <why>."`, and the `decline:` line says so, with the command.
  `ignore --propose --as=equivalent` (or `unreachable`) refuses a reason without a "because", and
  any proposal that still has the template's `<case>` or `<why>`; other kinds keep a free reason.
  The formatter equivalents say their reason the same way.
- **Mutants no test reaches list their ids in the agent format,** by line, each with its `or
  propose` command, so an agent can propose one from what it's shown.
- **A name a card's suggested test uses says what it is and what it was.** A suggestion first
  tries the receiver as a covering test in the module beside the function makes it
  (`Delivery::new(50)`), and then makes it the same way, saying so: `let delivery =
  Delivery::new(50); // as tests::small_orders builds it`. The evidence says `inputs seen: total =
  20, 80, with free_from = 50 (Delivery's field, from Delivery::new(50))` and `none has total ==
  free_from`, or `the edge is FREE_FROM (FREE_FROM = 100)`. Events gain `suggestion.builtBy`.
- **The gate names every mutant that holds it,** by id, those no test reaches too, and says a proposed one is waiting on a person, not missed; its
  cards leave the proposed ones out.
- **An empty `--only` id is refused** (`--only=`, `--only=a,,b`), where it ran every mutant.

- **From 1.0.0, `mutastic` runs the newest Java edition of its own minor version.** The two
  editions release each minor version together, and either may ship a patch alone, so in a Maven or
  Gradle project `mutastic 1.2.0` runs the newest Java edition 1.2.x, fetching it if it isn't kept.
  It looks up the Java edition's releases at most once a day, and uses what it has kept when they
  can't be reached or the newest can't be fetched yet. Before 1.0.0 it still uses the newest kept, or the latest release.
  `MUTASTIC_VERSION` still picks one exactly. See
  [One command, two editions](docs/editions.md#the-java-edition).

### Cards

- **A card stands alone without the source.** The `-` and `+` lines show the whole statement the
  change is in, found from the syntax tree, when it spans several lines (an `if` up to its
  condition's end, a statement, a match arm), cut to six lines around the change with `…`. A
  statement the mutant takes out has a `-` and no `+`, where the `+` said `(nothing: the line is
  gone)`. A condition forced to a value reads as that value: `!true` is `false` and `!false` is
  `true`, the rest of the line as written. The label under the caret names both sides,
  `starts_with changed to ends_with`, as the Java edition's does.
- **Covering tests are listed in the Java edition's order:** the function's own test module
  first (`pricing::tests::…` for a function in `pricing`), then by name, where they were by name
  alone, so both editions' cards, events and suggestions agree.
- **Suggested tests say a value the way the source does.** A `const` the comparison reads, in
  the function's file, is passed by name and its value goes in the comment
  (`assert_eq!(discount(FREE_FROM), 10); // FREE_FROM = 100 · …`), and its value is tried among
  the inputs; a field the receiver is made with, of the parameter's type and passed the same
  value, is one `let`, passed to both. Either names the test (`discount_at_free_from`,
  `is_free_at_free_from`). Otherwise the receiver's values are named by its fields or `new`'s
  parameters (`open_on_with_day_5_for_start_5_and_end_10`), not by its type.
- **`explain` on a skipped mutant says why once,** `SKIPPED  unviable: …`, not `skipped: unviable:
  …`.

## 0.2.1 (2026-11-09)

Windows support, and installing through mutastic.io.

### Changes

- **One line installs it, from mutastic.io.** `curl -fsSL https://mutastic.io/install.sh | sh`
  installs the native `mutastic` for Rust, Maven and Gradle projects alike, and every page and CI
  example now names it. [Installing](docs/install.md) leads with that line, then what it does and
  the variables that change it, then checking the version, and only then building from source,
  for platforms without a release build (Windows, where WSL also works).

### Fixes

- **Windows: output reads as it does elsewhere.** mutastic built from source on Windows wrote its
  places and report paths with `\` (`src\lib.rs:15`, `target\mutastic\mutation.json`), so the
  report's files, cards and the gate's lines differed from Linux and macOS, and the command for a
  crate inside a Java build did too. Paths are now written with `/` on every platform. A test
  process that crashes is named by the signal the same crash raises elsewhere (Rust's abort is
  `SIGABRT`), where it said "an abnormal exit". Ids were already the same on every platform. The
  tests now run on Windows on every change.

## 0.2.0 (2026-11-09)

The first release of mutastic for Rust, the Rust edition of mutastic. These are the changes made
while it was built, before this release.

0.1.0 was tagged but never released: its changes are here too.

### Scoring changes

Expect your score to move, for these reasons, not because your tests changed:

- **`mutastic/unwrap-try` is in the defaults.** `f()?` becomes `f().unwrap()`, so the error `?`
  would hand back panics instead. When no test makes that call fail, the mutant is missed: an error
  path no test runs, which line coverage can't see, since the line runs either way. It was in
  `stronger`; now every run has it unless `--operators` says otherwise. A `?` that passes on only a
  formatter's error (inside a `Display` or `Debug` `fmt`, or on a write to a `Formatter`) is skipped
  as equivalent, since no test can make it fail. Crates that use `?` get more mutants and their
  scores usually fall a little: on five real crates, the defaults score moved by between −1.7 and
  +0.4 points. No mutant's id changes, and `--operators=stronger` and `extended` are unchanged. See
  [Unwrap try](docs/operators/returns.md#unwrap-try).

### Changes

- **One run across CI jobs.** `mutastic run --shard=N/M` runs only the Nth of M shares of the
  mutants, dealt by each mutant's id, and keeps its events and report files in
  `target/mutastic/shares/N-of-M/`; `mutastic run --from-shards=DIR` runs no mutants, and puts every
  share found under DIR back together as the whole run: the same summary, cards, reports, threshold
  and exit status, and the `mutation.json` `explain` reads. Shares that aren't every share of one
  run (one missing or there twice, unfinished, split differently, a mutant in two, another
  mutastic, other code, another package, or `--since`) are refused with exit status 2. As in the
  Java edition. See [One run across CI jobs](docs/concepts/how-a-run-works.md#one-run-across-ci-jobs).
- **Events: a mutation's `applied`, in place of `bytecode`.** Each `mutant-finished` event's
  `mutation.applied` is the change as the engine makes it, and at what level: `{"level": "source",
  "text": "x < 0 -> true"}`, the whole changed code before and after. The 0.1.0 tag wrote that text
  as `bytecode`, a name that said something it isn't; no release writes it, as the schema asks
  `bytecode` only of the Java edition, which writes `applied` too from 0.6.0 (`"level": "bytecode"`).
  A later engine may add a level (`llvm-ir` is kept for one): accept a level you don't know, and
  read `text` as opaque. See [Events](docs/reference/events.md).
- **The events say more,** as the Java edition's do: `run-started`'s `share` in a share's run, a
  missed mutant's `suggestion`, the call its suggested test was found by, and `run-finished`'s
  `workers`, `workersStarted`, `coverageMs` and `build.setupMs`. An input above `i64`'s largest,
  which a `u64` can hold, is written as it is, where it used to wrap.
- **A flaky test is left out of the run, not a reason to stop it.** A test that fails before any
  mutant runs again on its own, up to three times, as in the Java edition. One that then passes is
  flaky: the run goes on without it, and says so: `(1 flaky, left out)` on the coverage line,
  `! flaky: … failed, then passed when run again; left out of this run; it reaches RandomState`
  before the summary, a `Flaky` row, a `flaky:` line in the agent text, and `flaky` and
  `flakyEvidence` in the event stream. What it reaches that varies is read from the source: calls
  such as `SystemTime::now` or `rand::thread_rng`, and a `RandomState` field a method reads. Its
  catches don't count, so code only it reaches is not covered. A test that fails every time still
  stops the run, now in the Java edition's words, `the tests fail without any mutant: …; their
  output is in target/mutastic/tests.log`, where it used to say to mark the test `#[ignore]`, which
  agents are told never to do. See [A test is flaky](docs/troubleshooting.md#a-test-is-flaky).
- **A statement that only writes output is skipped as arid,** and counted: `println!`, `eprintln!`,
  `dbg!` and a logger's macros (`log::warn!`, `tracing::info!`). Before, it had no mutant at all, so
  a change that only added a log line ran `0 mutants` and said nothing more; now the summary says
  `1 skipped (1 arid)`, the agent text `(1 more skipped)`, and the event stream has the mutant with
  `skipKind: "arid"`, as the Java edition does. Skipped mutants are out of the score, so no score
  moves, and no existing mutant's id changes. A `mutate:` rule in `mutastic.yaml` takes the skip
  back. See [Remove void call](docs/operators/calls.md#remove-void-call).
