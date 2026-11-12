# Changes

What changes from one release of mutastic for Rust to the next, written as changes land. The
newest come first.

## Unreleased

### Additions

- **The output options the Java edition has.** `--format=brief` (each card's head: the code with
  the change marked and what the run saw) and `--format=grouped` (the missed mutants of each file
  by function, a line each); `--vocabulary=classic` (killed and survived, in what a person or an
  agent reads, never in the events) and `MUTASTIC_VOCABULARY`; `--color=auto|always|never`; and
  `--hyperlinks=auto|always|never`, file locations as links a terminal opens. See
  [Output](docs/concepts/output.md#formats).

### Additions

- **`--report=sarif`, for code scanning, as in the Java edition.** It writes `mutation.sarif`,
  SARIF 2.1.0: a result for each missed mutant (a warning) and each one no test reaches (a note),
  on its line, with the card's headline, keyed by the mutant's id in `partialFingerprints` so an
  alert follows its mutant; never a fix. Paths are from the git repository's root. GitHub shows it
  as alerts on a pull request's lines. See [Report files](docs/reference/reports.md#code-scanning).

### Card changes

- **No suggested test of a crate's own function on an input its callers never gave.** A function
  that isn't `pub` (or a trait impl's) gets its inputs from the crate's callers, so a test calling
  it alone on a value none of them passed may test what can't happen: an agent was offered
  `fee(-1)` where the only caller passes a `u8`. Its card now offers no such test. It says no
  caller passed the value, and to test a caller that can, or, if none can, to propose the mutant
  unreachable.

### Scoring changes

- **A boundary where both sides give the same value at the edge is skipped as equivalent,** as in
  the Java edition: `if x > max { max = x }`, `if a < b { a } else { b }`,
  `if fee < 10 { 10 } else { fee }` and `if x < 2 { x = 2 }`, on whole numbers the source declares.
  Those mutants, which no test could catch, no longer count as missed, so a score can go up: a
  change to scores, as a minor release may make. See
  [Conditions](docs/operators/conditions.md#boundary).

### Fixes

- **A workspace that excludes a package its tests run Cargo on can be mutated.** mutastic's copy
  of the workspace is under its target directory, so Cargo, finding a package the copy's workspace
  excludes, looked further up, reached the workspace itself and refused it: the tests failed in the
  copy before any mutant, and the run stopped. In the copy, such a package now stands alone, as it
  does in the workspace; a package of a workspace nested in it is left as it is.

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
