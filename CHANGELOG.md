# Changes

What changes from one release of mutastic for Rust to the next, written as changes land. The
newest come first.

## Unreleased

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
