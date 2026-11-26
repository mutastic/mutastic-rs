# Changes

What changes from one release of mutastic for Rust to the next, written as changes land. The
newest come first.

## Unreleased

What's merged since the last release waits in [`changes/`](changes/), one file for each entry,
until the next release gathers it here.

## 1.4.0 (2026-11-26)

### Scoring changes

- **A property test runs unshrunk on a mutant, and the run says which frameworks it can't seed.**
  A mutant's run now sets `PROPTEST_MAX_SHRINK_ITERS=0`: proptest's first counterexample catches the
  mutant, and shrinking it, often the costly part of a failing property, no longer runs. A mutant
  caught the same way has the same status, but one whose property ran out of time or crashed
  while shrinking is now caught by the property's failure, so a timeout or a crash can become a
  plain catch, and a crash where a crash proves nothing, out of the score, can come back into it.
  proptest's seed was already pinned, but only proptest 1.7.0 and later read `PROPTEST_RNG_SEED`:
  an older one, as winnow locks, drew afresh in every run. quickcheck, arbtest and bolero seed
  themselves from the system with no variable to pin it. A run of a package that uses any of these
  now ends with a note on stderr, `note: property tests in proptest 1.6.0 run with their own seeds,
  which mutastic can't set: …`, as a mutant only some of their inputs catch can flip from run to
  run, and a property that fails before any mutant no longer offers a `PROPTEST_RNG_SEED=0`
  command that can't repeat its draw. See [Tables of cases and property
  tests](docs/troubleshooting.md#tables-of-cases-and-property-tests).
- **Modules a macro of the crate's declares are mutated.** A crate that declares its modules from
  a `macro_rules!` of its own, as serde and serde_core do with `crate_root!()`, had none of their
  files read: a run found no mutant, and passed with no score and no word of why. mutastic now
  follows a `mod name;` in the expansion of a macro the crate defines, in the file that calls it or
  in a `#[macro_use] mod` it declares, where the call is an item, and reads the file as rustc
  would. Their mutants are new, so a score can move either way.
- **A mutant is timed out by its tests' own time, side by side and under a ceiling, so a loop that
  never ends is always stopped.** Its tests could run for five times their coverage-pass times
  added up, and the coverage pass records every call's whole numbers: a test of 10.8s took minutes
  there, and a base64 mutant reached by 86 tests ran for over 40 minutes before its run was killed.
  Each test now has five times its own time and a second, timed again without the recording when
  that made 100,000 calls; a binary's tests share their limits out as they run side by side, never
  under ten seconds; and no mutant's tests run past ten minutes, or twenty times the suite's own
  time, which `timeout-ceiling:` sets. Twenty one-second tests and a fast one that a mutant makes
  loop are stopped after 21s on 8 cores, not 1m42s. A slow mutant whose tests would have finished
  can now be timed out, which counts as caught: no fixture's mutant moved, but check a threshold on
  a suite with tests that make millions of calls. The coverage pass skips calls a thread has
  recorded, 7 to 10 times faster on such a test, and the time left counts what a slow mutant has
  left of its limit
  ([docs/troubleshooting.md](docs/troubleshooting.md#how-long-a-mutants-tests-may-run)).
- **A likely flaky test's catches don't count, and the next runs check it harder.** A test was named
  likely flaky (`! likely flaky:`) when a mutant it caught changed result with the code unchanged,
  or a catch of it didn't repeat, but its catches still counted in that run, so a run could pass
  its threshold on them. Now, in the run that names it, each mutant it reached runs again without
  it. `last-runs/flaky-watch.tsv` remembers it, and later runs confirm each catch it makes 3 more
  times (`! watched:`), until its catches have held for 3 full runs. A score can fall where a flaky
  test's catches were counted. See [Flaky
  tests](https://docs.mutastic.io/concepts/flaky-tests/#when-a-test-is-named-likely-flaky).

### Score corrections

- **Code the build doesn't compile is not built here, whatever its `cfg`.** mutastic judged a `cfg`
  from its text, and took any name it didn't know as on: code under `cfg(docsrs)`, `cfg(loom)`, a
  build script's `cfg` it didn't set, or `cfg(not(..))` of one it did, was mutated, and counted as
  not covered though it never compiles, so anyhow scored 60.4% where 72.5% was right. Now the build
  says: as it compiles your package, mutastic asks the same rustc which `cfg`s it has, the features,
  the build script's `cargo::rustc-cfg` and the `--cfg`s from `RUSTFLAGS` and `.cargo/config.toml`
  included, and a name nothing set is off, as rustc has it. Only mutants that could never run go,
  so a score can only rise. See `docs/troubleshooting.md`.

### Additions

- **`--at` runs, or lists, the mutants on a line.** Checking one line took listing the mutants,
  finding their ids and passing them to `--only`, though a person fixing a line, an agent on one
  function or an editor has a file and a line, not ids. `mutastic run --at=<file>:<line>`, or
  `<file>:<from>-<to>`, runs the mutants whose cards point at those lines, under the run's
  operators, and says each one's fate after a line that counts them:
  `lines 26–29 of lib.rs: 6 mutants, 5 caught, 1 missed`. `mutastic mutants --at` lists them.
  Lines with none are an error naming the nearest that have one, never a run of none. At a
  workspace's root the file picks its member. As in the Java edition. See `docs/scoping.md`.
- **A card names the input a property caught a mutant with.** When a proptest, quickcheck or
  bolero property catches a mutant, `verify`, `explain` and the card say which input it failed on,
  ``caught by props::reverses_twice with `xs = [0]` ``, and `mutant-finished` has it as
  `caughtInput`, as `mutation.json` has `mutastic.caughtInput`. In a run of every mutant it's the
  first case that failed, unshrunk; a run of mutants named by id runs the catching proptest once
  more with shrinking on, for the smallest. `run-started` says which property-testing frameworks
  the package uses in `propertyTests`, with the seed mutastic drew proptest's cases from and the
  frameworks it couldn't seed.
- **`timeout-ceiling:` in `mutastic.yaml` sets the most one mutant's tests may take.** Without it,
  the ceiling is ten minutes, or twenty times the suite's own time when that's longer. It takes a
  whole number with its unit, `timeout-ceiling: 30m`, `90s` or `2h`, and a mutant stopped there is
  timed out with a note that says so: `its tests ran for the most a mutant's tests may, 30m00s
  (timeout-ceiling:)`. The Java edition doesn't read it yet
  ([docs/reference/configuration.md](docs/reference/configuration.md)).
- **Arguments for Cargo.** What follows `--` goes to Cargo on each build call, and never to the
  tests: `mutastic run -- --features postgres`. The features it turns on choose the code mutated
  too, and each command a run offers keeps it. `MUTASTIC_BUILD_ARGS`, or `build:`'s `args:` in
  `mutastic.yaml`, gives the same to every run, and no command repeats those, so a secret goes
  there. An argument that fights how mutastic drives Cargo is refused, with the reason.

### Fixes

- **A share stopped part way keeps what it finished.** A share's `events.jsonl` was written only
  at the end, so one stopped by a job's time limit, `timeout` or Ctrl-C left nothing: base64's share
  ran 35 of its 36 mutants, then lost them all. The file is now written as the run goes, each
  mutant as it finishes, and has no `run-finished` until the share does, so `--from-shards` still
  refuses it, now saying how many mutants it holds. A mutant whose tests the same Ctrl-C or kill
  ended isn't kept as caught ([docs/concepts/how-a-run-works.md](docs/concepts/how-a-run-works.md)).
- **The agent format's docs say what it prints at 100%.** With nothing missed and nothing
  unreached, it's the verdict, `vs last run:`, `reports:`, `rules:` and `decline:`, with no
  fence: `docs/concepts/output.md` now says so, and `docs/differences-from-java.md` that the Java
  edition leaves out `rules:` and `decline:` there.
- **`gate check` and `next` confirm a watched test's catches, as `run` does.** A test an earlier run
  named likely flaky has each catch it makes run 3 more times, and a catch that doesn't repeat
  doesn't count, so the gate no longer passes a task on a lucky catch, and `next` queues a mutant
  only such a catch hid. `explain` and `baseline` confirm the same way. None of them writes the
  watch.
- **A mutant only a likely flaky test reached says so, not that no test reaches it.** Once that
  test is left out, nothing reaches the mutant, and it's not covered: `verify`, the cards and the
  agent entry now say `only <test> reached it, and it's likely flaky: its catch didn't count`, or
  `…: it was left out` when the test hadn't caught it, and the same for several tests. A run put
  together with `--from-shards` says it too.
- **A name given to `--exclude-tests` that matches no test says so.** One that named none of the
  package's tests, mistyped or since renamed, was taken, left nothing out and said nothing. Now
  the run says so in every format, `! left out nothing: tests::big_order names no test of pricing.
  There's one called tests::big_orders.`, with `matched: false` and the close name as `maybe` in
  `run-finished`, and the Score row doesn't count it among the tests left out. It's a warning, as
  in the Java edition: a test renamed since it was listed under `exclude-tests:` doesn't stop the
  run.

### Internal

- **The build's everyday commands are one word each, with [just](https://just.systems).** `just`
  lists them; `just check` is everything a pull request needs to pass (the format, clippy and the
  tests), and `just ci` is what CI runs. The words mean the same in the Java edition, and a move off
  cargo changes the recipes beneath them, not the words people and agents type.
- **A release is checked before it's tagged, and its draft has notes.** 1.2.0, 1.2.1 and 1.3.0
  were drafted with empty notes; the draft's are now the release's section of `CHANGELOG.md`
  (`scripts/changelog.sh notes X.Y.Z`), and a tag without one fails within a minute.
  `scripts/release-check.sh X.Y.Z` checks what a release needs at once and lists what's wrong: no
  scoring change in a patch and no release without entries (`scripts/changelog.sh check --for
  X.Y.Z`), the version in `Cargo.toml`, only the newest contract pin, and ids all kept. CI runs it
  on a release pull request.
- **A release is checked once it's out, and its candidate before it's tagged.**
  `release-verify.yml` installs the published release with mutastic.io's `install.sh` and
  `install.ps1` and with Scoop, on Linux, macOS and Windows, then runs it on a crate and, through
  the Java edition it fetches, on a Gradle project; `candidate-run.yml` runs the native `mutastic`
  built from the tree over a share of every fixture, and fails on a crash, a run that can't run, or
  tests that fail without any mutant.
- **The contributor docs and AGENTS.md say `just <verb>`.** CONTRIBUTING, AGENTS.md, the README's
  contributing part and the pull request template name the recipes (`just check`, `just fmt`,
  `just docs-output`, `just ids`, `just merge`, `just setup`) instead of cargo and scripts, so
  people and agents type the same words in both editions. A test that says how to write its copy
  again, or how to pin the contract or the report page, names the recipe and the command beneath.
- **Every release asset is attested, and checked against its attestation once it's out.** The
  release workflow attests each archive, `SHA256SUMS`, Scoop's `mutastic.json` and the SBOM as
  built by `release.yml` at the release's tag, and the SBOM as what each archive is made of,
  before the draft is made; `release-verify.yml` runs `gh attestation verify` on every asset it
  downloads. RELEASING.md's *Checking a download* says how to check one.
- **CI runs the build through `just`.** Each job calls a recipe, `just lint`, `just test`, `just
  clippy`, `just msrv`, `just fuzz`, `just miri`, `just asan`, `just contract`, and Self's `just
  self-build`, `just self-shard`, `just self-merge` and `just self-terminal`, in place of cargo
  commands written into the workflows, so CI and a contributor type the same words and a job that
  fails can be run as CI ran it. Commits checks the justfile's format and its list of verbs, so
  none goes unseen.
- **mutastic runs on a corpus of real crates every night.** The fixtures hold the shapes we thought
  of, and the bugs that mattered turned up in real crates instead: a crate its own dev-dependencies
  pulled in again, modules declared inside a macro, a mutant no limit stopped. `corpus/corpus.yaml`
  pins 24 packages, each to a commit, with what a healthy run does on it: a score, or the refusal
  that's right today. `.github/workflows/corpus.yml` runs a share of each one's mutants nightly and
  on demand before a release (`scripts/corpus-run.sh`, through `just corpus`), checks the agent
  format's head on each score, and tables the results on the run's page against the last good
  night's: the run is red when a crate that passed then fails now. CONTRIBUTING.md says how to add
  a crate, and that each corpus failure becomes a fixture in the pull request that fixes it.
- **Releases leave on a train.** From 1.4.0, a minor release leaves every second Tuesday, with the
  Java edition's `X.Y.0` before it, once the corpus, `release-check` and the candidate run pass;
  entries merged by the day before go with it. Patches between trains are for regressions and
  security fixes only, a release candidate is a GitHub pre-release and never reaches Central, and
  Brian decides each patch. RELEASING.md's *The release train* has the rules, as the Java
  edition's does.
- **A release is published with one approval.** After the tag, the release workflow drafts the
  release and writes a summary for its approver, from the draft as it is on GitHub: the version,
  each asset with its digest and its attestation checked, the notes and the Java edition it pairs
  with. Its `publish` job waits in the `release` environment; once approved, it checks again that
  the Java edition `X.Y.*` is published with its CLI, publishes the draft as the latest release and
  starts `release-verify.yml`. It refuses while the environment has no required reviewer, so
  nothing is published unapproved. Publishing a draft by hand is no longer a step.
- **A release is a list of `just` recipes.** `just release-prepare X.Y.Z` makes the release commit
  on a branch (the version in Cargo.toml and Cargo.lock, the changelog's section, the docs' kept
  outputs, what the release takes for compat, and the contract's pins pruned) and opens its pull
  request, through `scripts/release-prepare.sh`, whose `--dry-run` stops before the push.
  `release-check`, `candidate` and `release-verify` wrap the scripts and workflows that were typed
  by hand. `release`, Brian's, asks first, then tags main's release commit and pushes the tag
  (`scripts/release-tag.sh`, which for `X.Y.0` first checks the Java edition is out). RELEASING.md
  opens with the release as these recipes in order, and AGENTS.md says which an agent may run. The
  `release` environment's approval stays what guards a publish: `just --yes` skips a recipe's
  question.
- **Each module keeps to its layer, and a test holds it there.** ARCHITECTURE.md's Layers puts
  every module of `crates/mutastic` in one of six layers, and `tests/layers.rs` fails on a
  `crate::` path that reaches a layer above its own. `engine` and `compile` no longer reach into
  `report`: the helpers they used for paths, shell words, counts and durations moved below it.
- **The output kit is a crate of its own.** Colour roles, glyphs and labels, escaping and the
  agent's fence, close names, help's layout, progress's bar and heartbeat, and the events' envelope
  moved to `crates/mutastic-output`, which depends on none of the workspace's crates, as
  `tests/layers.rs` checks, so proptastic can share them without mutastic. What mutastic prints is
  unchanged.
- **Clippy holds processes, the environment, printing and `expect` to where they're used today.**
  `Command::new`, `env::var` and `env::var_os` are disallowed in `clippy.toml`, and each file that
  may use them says why in an `#![expect]` at its top, as Clippy can't allow a method by path. The
  library denies `println!` and `eprintln!`, which panic on a closed stream, and `expect_used` warns
  outside tests, each use outside them with its reason.
- **The ceiling test times the mutant, not the run.** `a_mutant_stopped_at_the_ceiling_says_so`
  held the whole run, the crate's build included, to ten seconds, and failed twice on a slow
  Windows runner at 10.78s and 11.6s with the ceiling working. It now holds the mutant's own
  `durationMs` between the 2s ceiling and the 10s floor.

## 1.3.1 (2026-11-23)

### Security

- **The summary's `Reports` row escapes what can't be seen in a path.** A report's path with an ANSI
  escape or a bidi control in it went to the terminal as it was, free to rewrite the screen; it's
  now shown escaped, as every other value from the project is.
- **The agent format quotes a cfg the build doesn't have inside the fence.** A cfg's value is free
  text, so `#[cfg(target_os = "…")]` could put a sentence of the project's on the `not built here:`
  line, which came before `<<mutastic-data …>>`, where an agent reads mutastic's own words. The
  line is now inside the fence, which opens for it alone when nothing was missed
  ([docs/concepts/output.md](docs/concepts/output.md#agent-output-and-untrusted-text)). The person's
  format and `mutastic mutants` show it as before.

### Changes

- **Patches may correct a score upward, and never lower one.** A patch release used to promise
  never to move a score. It now promises never to lower a score, never to add a mutant and never
  to turn a passing run into a failing one: the one score change it may make is a correction that
  drops mutants that could never run, such as code the build never compiles counted by mistake, so
  a score can only rise. Its changelog lists each under a new section, *Score corrections*.
  Anything else that moves a score still waits for a minor release's *Scoring changes*. See
  [Compatibility](docs/reference/compatibility.md#scores).

### Fixes

- **The agent format names the report files a run wrote.** It never said there were any, so an
  agent took it there was no report and ran again with `--report=html`. A `reports:` line after the
  verdict names them as the summary's Reports row does,
  `reports: target/mutastic/mutation.json · target/mutastic/mutation.html`, and with `--report=none`
  there's no such line.
- **A run with `--only` says it writes no report when one is asked for.** `--report` with `--only`
  was passed over in silence, so a person or an agent asking for `mutation.json` got none and no
  reason. A note on stderr now says why: a run of some mutants says nothing about the rest. The
  exit code is as it was.
- **Writing the reports takes as long as the tests are many, not their square, and the run's
  Duration counts it.** Each test that covers a mutant is placed in its file for
  `mutation.json` and the HTML report, and each was found by reading and parsing its file again:
  with typenum's 1,800 tests, the reports took 74 of the run's 80 seconds, while the Duration row
  said 6.1s. Each file is now read once per report, and the Duration row has a `reports` phase, in
  its total.
- **A package whose name a dependency also has, from crates.io, is built.** When a dev-dependency
  pulls the package itself from crates.io, as itoa's and serde_json's do through criterion and
  trybuild, Cargo has two packages of that name, and the bare `-p itoa` that mutastic built with
  was ambiguous: the run stopped with `build-failed` before any mutant ran. mutastic now names the
  package by its spec, `path+file:///…/itoa#itoa@1.0.19`, and so does a command it shows when the
  name alone would be ambiguous.

## 1.3.0 (2026-11-23)

### Card changes

- **A removed release's card says what to look at.** A removed `remove_file` or `remove_dir_all`
  asks for a test that the path is gone, and a statement removed from a `Drop` impl's `drop` for a
  test that lets a value go and checks what that changes. A removed `drop(x)` says that `x` still
  goes at the end of its scope, so the mutant matters only where code after it takes the lock
  again or waits on a thread that does. Each card's test is run against its mutant. The Java
  edition's cards do the same for `close()`, a pool's `shutdown()`, a shutdown hook and
  `clean()`. See [Calls](docs/operators/calls.md).
- **Code no test reaches gets an assertion to start with.** Under `Not covered`, `try:` gives an
  assertion for a function's first test, `assert_eq!(Pricing::new().loyalty_credit(0), 0);`, and
  `--format=agent` lists up to ten as `start with:`, as the Java edition does. Each is found by
  calling the function as it is and with one of its mutants in, so its value is what the code gave
  and the test it starts catches that mutant. A function only the crate calls, or one a test
  can't call with whole numbers alone, gets none. `mutant-finished` carries the call in
  `suggestion`. See [Code no test reaches](docs/concepts/suggested-tests.md#code-no-test-reaches).

### Scoring changes

Expect your score to move, for these reasons, not because your tests changed:

- **A call whose result is only checked is removed: `handle.join().unwrap();` is a mutant.** A
  statement ending in `.unwrap()` or `.expect(..)` had no mutant and wasn't counted, so a test that
  didn't wait for a thread, or never saw what `tx.send(v).unwrap();` sent, was never asked to. Now
  the statement is removed, by `mutastic/remove-discarded-call`, as the Java edition removes
  `thread.join();`. One that checks a value it's given, `x.unwrap();`, is counted and skipped as
  arid, `arid: only checks the value is there`, as the Java edition skips a removed
  `Objects.requireNonNull(x);`. New ids appear; none moves. See [Remove discarded
  call](docs/operators/calls.md#remove-discarded-call).
- **A thread's name, its stack size and how long a runtime's idle threads live are skipped as
  arid.** None of them changes whether, when or how often work runs. A name given to
  `thread::Builder`'s `name`, or to a tokio runtime builder's `thread_name` or `thread_name_fn`, is
  `arid: only names a thread`; a stack size, `stack_size` or `thread_stack_size`, is `arid: only
  sizes a thread's stack` while the source shows it stays above zero; and tokio's
  `thread_keep_alive` is `arid: only sets how long idle threads live` on the same terms. The call
  is known by the builder its chain starts at, so any other `.name(..)` is mutated as before. A
  crate that reads names (`thread::current().name()`) keeps its name mutants. A spawn, a `join`
  and `worker_threads` still run. Runs on code that sets threads up get fewer mutants, and scores
  can go up; a `mutate:` rule brings them back. The Java edition skips them too, and a priority.
- **Twins run once, and their place counts once.** Two mutants at one place that always do the
  same now run as one: the other is skipped as `twin: same as <id>`, naming the one that runs, and
  is out of the score. Two operators making the same code are twins, as a function that ends in
  `n > 0` made `true` by `mutastic/return-value` and by `mutastic/remove-conditional` is; so are
  the changes to a comparison of an unsigned value with 0 that agree for 0 and above, `n > 0` made
  `n >= 0` and made `true`. No id changes, and each still works: `verify`, `run --only` and
  `explain` run the one it's the same as, `mutastic ignore` records the decision for that one, and
  an ignore of either holds for both. Runs have fewer mutants to build and run, and a score moves
  a little where a place was counted twice. Events give a twin's `sameAs` and the skip kind
  `twin`. The Java edition has twins too. See [Twins](docs/concepts/results.md#twins).
- **A call stored and then only checked is removed: `let removed = remove_file(&p);
  removed.unwrap();` is a mutant.** The check was skipped as arid, and the call that made the value
  got no mutant at all, though `remove_file(&p).unwrap();` written in one statement has one. Now
  both statements go, by `mutastic/remove-discarded-call`, when the check comes straight after and
  nothing else uses the value. `x.unwrap();` on a value it's given stays skipped. New ids appear;
  none moves. See [Remove discarded call](docs/operators/calls.md#remove-discarded-call).
- **More of the arid cases are mutants, made and skipped, as in the Java edition.** A number
  given to `with_capacity`, `with_capacity_and_hasher` or `reserve` had no mutant, and arithmetic
  there, `Vec::with_capacity(n * 2)`, was counted; both are now skipped as `arid: only sizes a
  collection's first allocation`. A sleep removed, `thread::sleep(d);` or `sleep(d).await;`, was
  counted; it's `arid: only waits`, and the number or the sum that only sets how long it waits is
  `arid: only sets how long to wait`. An `Err`'s, a `context`'s or an `expect`'s text, and every
  mutant in a `Debug` or `Hash` impl, are made and skipped too, with the Java edition's ids. Each
  is counted under `skipped`, audited by `MUTASTIC_ARID=audit` and brought back by a `mutate:`
  rule. Scores on code that sizes collections or sleeps can go up.
- **A `Display` impl is counted, and `new` has its body replaced.** A `fmt`'s `Ok(())`, and with
  `extended` a `fmt`'s or a `hash`'s or a `new`'s body replaced, had no mutant. Outside a `Debug`
  or `Hash` impl they're mutants now: what a `Display` writes is often what a person or a program
  reads. New ids appear; none moves.
- **A variable's first `0` or `1` is counted.** `let mut count = 0;` had no mutant; now its
  changes run like any other number's, as the Java edition counts them since dropping
  `mutastic:first-values`: a counter or a total that starts wrong is a wrong answer.
- **A time that isn't a sleep's is counted.** A number in `Duration::from_secs(30)` had no mutant
  wherever it was; now only one that goes into a sleep is skipped, so a timeout, a deadline or a
  backoff is mutated, as the Java edition counts them. A runtime's keep-alive that the change
  makes zero is counted too, as the keep-alive rule says. Scores on code with timeouts can go
  down.
- **A value a `let` works out only for a log line is skipped as arid.** In `let took =
  start.elapsed().as_millis() - base; info!("took {took} ms");` the subtraction's mutants only
  change what the log says, so they're now `arid: only feeds a log line`, part of
  `mutastic:output`. Every read of the name has to be in an output macro (`println!`, `info!`,
  `tracing::warn!` and the rest): an argument, `{took}` in the format string, or a tracing field.
  A value read anywhere else stays counted: returned, compared, pushed, passed to a function,
  `format!`, `write!` or `assert!`, read in a closure, or shadowed by another binding of the same
  name. Inline arguments, `info!("{}", a - b)`, never had mutants. Code that logs timings gets
  fewer mutants, and its score can go up; `arid: [mutastic:output]` on a `mutate:` rule brings
  them back. The Java edition skips the same values, inline or through a local.
- **A log level's check around only output is skipped as arid.** `if log_enabled!(Level::Debug)
  { debug!(…); }`, and `tracing`'s `enabled!`, with no `else`: changing the check only decides
  whether the line is written, so its mutants are `arid: only decides whether output is written`.
  A body with anything but output macros in it keeps them counted. Runs on such code get fewer
  mutants, and scores can go up; `arid: [mutastic:output]` brings them back.

### Additions

- **Bring back one arid rule: `arid:` on a `mutate:` rule.** `arid: [mutastic:output]` under a
  `mutate:` rule takes back only that rule's skips in the code it covers; without it, every arid
  skip, as before. An id mutastic doesn't have is refused, with the one meant when it's close.
  The thread settings' skips have ids too: `mutastic:thread-names`, `mutastic:stack-sizes` and
  `mutastic:keep-alive`.
- **A skip says which arid rule made it, and how to undo it.** `explain` on a mutant an arid rule
  skipped has an `arid` row: the rule's id, what it skips, a link to its docs and the
  `mutastic.yaml` rule to copy that runs it. The HTML report says the same. Events carry the id as
  `skipRule` (`mutant-finished`) and the row as `arid` (`mutant-explained`), and mutation.json as
  `mutastic.skipRule`, as in the Java edition.
- **The arid rules have the Java edition's ids.** A statement that only writes output is
  `mutastic:output`, and a removed `x.unwrap();` that only checks the value is there is
  `mutastic:null-checks`, as Java's `Objects.requireNonNull(x)` is. The editions' contract lists
  every rule, and each has a section on the [Arid rules](https://docs.mutastic.io/reference/arid/)
  page.
- **`mutastic/boolean-constant`, in `stronger`: a flag set the other way.** A `true` or `false`
  passed to a function, bound by a `let`, assigned, or given to a struct's field becomes the other,
  where only a returned one did. One in a condition isn't changed. The Java edition has it too. See
  [Boolean constant](docs/operators/arguments.md#boolean-constant).
- **`library/plain-division`, in `stronger`: division that rounds towards zero.** `a.div_euclid(b)`
  becomes `a / b` and `rem_euclid` becomes `%`, so a remainder that's wrong for negative numbers,
  `-1 % 3` being `-1` where `rem_euclid` gives `2`, shows unless a test has one. A value the source
  proves unsigned is skipped as equivalent. The Java edition has it too, for `Math.floorDiv` and
  `floorMod`. See [Plain division](docs/operators/arithmetic.md#plain-division).
- **`library/swap-ends`, in `stronger`: one end of a collection for the other.** `first` and
  `last`, `front` and `back` (and their `_mut` forms), and a `BTreeMap`'s and a `BTreeSet`'s
  `first_key_value` and `last_key_value`, `first_entry` and `last_entry`, and `pop_first` and
  `pop_last` swap. It finds the newest taken where the oldest was meant, which tests with one
  element can't see. An iterator's `last` is left alone, as it has no `first`. The Java edition has
  it too, for `getFirst` and `getLast` and a `Deque`'s and a sorted collection's ends. See [Swap
  ends](docs/operators/library.md#swap-ends).
- **The summary says when the machine slept during the run.** Duration counts the time the
  machine was awake, so a run a Mac slept through for four minutes said 15.4s. When the clock ran
  at least 5 seconds and a tenth of its time more, a line under Duration says `The machine slept
  for 3m49s during the run, which Duration leaves out.`, and `run-finished` has it as `sleptMs`,
  else 0. The Java edition says it too. See [When the machine
  sleeps](docs/concepts/output.md#when-the-machine-sleeps).
- **Code only `main` runs says so, and what to do.** A binary's `main` no test reaches read as not
  covered, like any other function. Its line in the Not covered list now says it's reached only
  from `main()`, and to move what it does into `run(args, out)`, which returns the exit code, test
  `run`, and leave `main()` a one-line call to it; a `main` whose mutants are all on one line gets
  no such advice. The Java edition says the same. See [Code reached only from
  main](docs/concepts/results.md#code-reached-only-from-main).
- **More of what a suggestion is found by, in events.** `suggestion` names the constant or field
  the call is at the edge of (`named`) and the receiver's parameters, and `explain`'s site says
  which of its function's sites it is (`site.ordinal`), as in the Java edition. `mutant-finished`'s
  keys come in the Java edition's order.
- **Events say where a catch failed, and which tests catch everything alike.** `mutant-finished`
  and mutation.json have `failureOrigin`, as in the Java edition: `assertion` when an `assert!`
  failed, `test` when the test's own code panicked, `code` when the code under test did. A test
  that caught mutants in three or more functions, each time with a panic in its own code, gets a
  note under the summary and in the agent format, and `run-finished`'s `unrelatedCatches` names
  it, where the list was always empty: it may be flaky, or call a lot of code.
- **`MUTASTIC_ARID=audit`: see whether the arid rules hide anything.** Set it, and a run runs the
  mutants mutastic's arid rules skip as well, out of the score and the counts, and the summary has
  an `Arid audit` row: how many ran, how many the tests caught, how many of those by a test's own
  assertion, and where. A catch by an assertion means a test checks what the arid code does, so the
  row gives the `mutastic.yaml` rule that brings that rule's mutants back there. `run-finished`
  has it as `aridAudit`. The Java edition has it too.
- **`MUTASTIC_PILLS` draws labels as you like them.** `rounded` gives `MISSED` and `PASS` round
  ends, for a terminal with a Nerd Font; `off` leaves out the coloured block and keeps the words in
  their colour; `square`, as before, unless set. Any other value is refused. The Java edition has
  it too.
- **Four of the Java edition's options for a run: `--confirm-catches`, `--kill-matrix`,
  `--exclude-tests` and `--each-mutant`.** `--confirm-catches` runs each catching test again on its
  own against the mutant: a catch that doesn't repeat doesn't count, the other covering tests
  decide, and the test is named as likely flaky (`unconfirmedCatchBy` in the events, and
  `catch-not-repeated` in `run-finished`'s `likelyFlaky`). `--kill-matrix` runs every test that
  reaches a caught mutant and names each that catches it, as `caughtByAll` in the events and
  `killedBy` in `mutation.json`. `--exclude-tests=TEST` leaves out a test, by its name as a run
  shows it, or a module of tests by its path: the summary's `Left out` row names it, what only it
  reaches is not covered, and a flaky test's line now offers it. `--each-mutant` gives every missed
  mutant a card of its own, and lists code no test reaches by line. `verify`, `explain`, `next` and
  `baseline` take `--confirm-catches` and `--exclude-tests` too. See
  [How a run works](docs/concepts/how-a-run-works.md) and
  [Troubleshooting](docs/troubleshooting.md#a-test-is-flaky).
- **Annotations and a job summary on GitHub Actions.** Where `GITHUB_ACTIONS=true`, `mutastic run`
  now annotates the line of each of its first ten misses, ranked as the agent format ranks them,
  so a pull request's changed files show them, and adds a Markdown summary to the job's page
  (`$GITHUB_STEP_SUMMARY`): the verdict, the score, the counts and the first misses with where they
  are. It needs no token or permission, and the annotations go to stderr, so a format's own output
  is untouched. `MUTASTIC_GITHUB=off` turns both off. The Java edition does the same. See [On
  GitHub Actions](docs/reference/reports.md#on-github-actions).
- **A rules file can ask for the Java edition's other rewrites.** Beside `remove` and `rename`, an
  operator's `rewrite:` takes `keep argument` (`set.unwrap_or(8080)` becomes `8080`), `unchecked
  arithmetic` (`a.checked_add(b)` becomes `Some(a.wrapping_add(b))`, and a `saturating_` method the
  `wrapping_` one) and `call name(TYPES) with A, B` (`call take(usize) with usize::MAX`), whose
  arguments are constants and whose types are for the reader. `omit last argument` stays an error,
  now with its reason: it calls a shorter overload, and Rust has none. See
  [Rewrites](docs/reference/configuration.md#rewrites).
- **Your own output calls, as a `mutastic.yaml` rule.** `- output: crate::audit::record` with a
  `reason:` names a function or a macro of your own that only writes output, an audit log or a
  metric (`metrics::counter`), and a statement that calls it is skipped as `mutastic:output`:
  `arid: only calls audit::record (<reason>)`. The path in the code may be shorter after a `use`.
  A `mutate:` rule brings them back, and none are output until a rule says so. As in the Java
  edition.
- **A way past a test that fails on its own: `exclude-tests:` in `mutastic.yaml`, each with a
  reason.** A test that fails with no mutant in still stops the run, and its message now ends in
  the way past it: list it under `exclude-tests:` with a `reason:`, or give `--exclude-tests` for
  one run. The summary's `Left out` row names each test with its reason, the Score row ends in
  `· 1 test left out`, and `run-finished` lists them as `testsLeftOut`. A left-out test can only
  make the score fall: what only it reaches is not covered. The agent format never offers it, and
  `mutastic gate` holds a task that adds one, as a task that took a test away. See
  [Configuration](docs/reference/configuration.md#leaving-a-test-out).
- **A case that catches a mutant is named by its row.** `verify` and `explain` say `caught by
  rstest_tests::discounts::case_2_at_the_edge (case #[case::at_the_edge(100, 10)])`, the rstest
  `#[case]` or test-case `#[test_case]` row as written, and the `mutant-finished` event has
  `caughtCase`, with the `fn` the row is written in, its place in this run and the row. A case that
  fails with no mutant in is named with its row too. The number in a case's name moves when a row
  is added above it; the row says which one it is (`docs/concepts/results.md`).
- **`mutastic mutants` says what a set of operators would add to a run's time, before it's run.**
  With `--operators`, the line before the count estimates it from the last whole run: `estimate: 24
  more mutants than defaults, up to 64ms more of tests, from the run of 23 Nov 2026 04:58 UTC`, the
  time of the tests that reach each new mutant's code. It says why when there's no estimate: no run
  recorded, the code changed since, a run under other operators than the project's, or new mutants
  in code that run didn't measure. `--estimate` runs every test once, with no mutant in, and
  estimates what `defaults`, `stronger` and `all` would each take, keeping what it measured for the
  next `mutants --operators`. A whole run and `--estimate` keep the times in
  `target/mutastic/test-times.tsv`. See [Coverage and mutants](docs/coverage-and-mutants.md).

### Changes

- **Five failures have the Java edition's codes, and two of them now stop the run.** A package
  whose tests are all `#[ignore]`d, and one whose tests pass and reach none of the code with
  mutants, used to finish with every mutant not covered and exit 1: a 0% that measures nothing.
  Each now stops the run with exit 2 and says why, as `no-tests` and `nothing-covered`; a package
  with no tests at all still finishes, and says `Found no tests`. A test that ends its process
  before any mutant, by `process::exit` or a crash, is `tests-stopped` and named with how it ended,
  `(it exited with status 3)`; one that runs out of time is `tests-timed-out`; both were
  `red-baseline`. A file mutastic can't read or write is `disk`, not `other`, and its message ends
  in what to do about a full disk, a directory that isn't writable or a file where a directory
  goes. See [Errors](docs/reference/errors.md).
- **A test that fails with no mutant in, or caught mutants and fails on its own, exits 3, as in
  the Java edition.** It exited 2, as a usage error does; a script that checks for 2 there now
  sees 3. There's a way past it, leaving the test out with a reason, so it's told apart from a
  mistake in the command. Every other failure still exits 2 until 2.0.

### Fixes

- **A file one mutant's tests leave in the temporary directory can't change the next mutant's
  result.** Tests shared the system's temporary directory, so a mutant that removes a
  `remove_file` left its file there, and a test of another mutant that removes the write could
  read that file and pass: missed in one run, caught in the next. Each test process now gets an
  empty temporary directory of its own, as `TMPDIR` (and `TMP` and `TEMP` on Windows), deleted
  when it ends. Nothing is left in yours either. The Java edition empties its worker's temporary
  directory between mutants the same way. See [How a run
  works](docs/concepts/how-a-run-works.md#each-mutant-against-the-tests-that-reach-it).
- **A thread started and not waited for says which test catches it.** A missed `thread::spawn(..)`
  or `thread::Builder`'s `spawn` said only that no test checked what the statement does. Its card
  now says no test waits for the thread and checks what it did, and to join its handle or wait on a
  channel its closure sends to first. A scoped thread's `spawn` keeps the card any statement gets,
  as its scope waits for it. The Java edition's cards name the test the same way for threads,
  pools, interrupts, locks and daemon flags.
- **A count of one reads as one.** The gate's pass said the tests `catch 1 of the 1 mutant`; it
  now says they `catch the only mutant`, `catch all 6 mutants` when they catch every one, and
  `catch 4 of the 6 mutants` otherwise. `1 of the baseline's mutants is caught now`, from a run
  and from `baseline --prune`, and `removed 1 ignore that no longer holds`, from `ignore
  --prune`, take their verb from the count too, as the Java edition's do.
- **A test stopped for time takes the processes it started with it.** A mutant that made a test
  run past its time stopped the test process, and left whatever it had started running: a build,
  a server, a `sleep`. They went on taking memory and cores from the next mutants' tests, and on
  a small machine could take it down. Those processes are now stopped too, as the Java edition
  does.
- **An error reads as the Java edition's, and ends in the one command to run.** A mistyped
  command or option with nothing close ends in `mutastic --help` or `mutastic run --help`, without
  a `Usage:` line. A value that isn't one of an option's says `--format takes full, …, not xyz`,
  or with a close one `--format has no value agnt`; `--workers=0` and `--tries=0` say the range
  and what they got. A missing value, id or reason ends in your line with a `<…>` to fill in, where
  `--format=FORMAT` was taken by an agent as a command to run as it was. An id one slip from a
  mutant's ends in your line corrected, not in `mutastic mutants`, and `gate` with a close action
  in `mutastic gate open`. A script or an agent reads one message in both editions.
- **`--operators` given twice adds, as in the Java edition.** `--operators=stronger
  --operators=-stream` runs `stronger` without the `stream` operators, where the second replaced
  the first. A list that only leaves operators out, `--operators=-mutastic/boundary`, leaves them
  out of the defaults, where it ran no operator at all. `run --help` lists `--shard` and
  `--from-shards` after `--workers`, in the editions' shared order.
- **A test whose code exits is named, and an exit isn't a crash.** A mutant that made code a test
  ran call `process::exit(2)` read as caught only because the test process crashed (`crash: an
  abnormal exit`), with no test named. It's now caught by the test that was running, by the exit:
  the card says `caught by …, by the process exiting with status 2`, and the event has `caughtWith:
  "exit: 2"`. Status 0, which `cargo test` reads as a pass, is no catch: the tests that never ran
  after it now run without it, and decide. The Java edition does the same for `System.exit`. See
  [When the code a test runs exits](docs/concepts/results.md#when-the-code-a-test-runs-exits).
- **`ignore` names its files from where you are, and drops a `.` from its undo.** From above the
  package, `recorded in fixtures/pricing/.mutastic/ignores.yaml`, where it said
  `.mutastic/ignores.yaml`; in the package, `to undo: mutastic ignore --remove mb2f8ae57`, without
  a ` .`. `ignore --prune` on another directory names that directory's file to `git checkout`, and
  `--propose` with `--accept`, `--remove` or `--prune` is refused, as in the Java edition.
- **A mistake in `mutastic.yaml` or `.mutastic/ignores.yaml` is told as the Java edition tells
  it.** A section mutastic doesn't have says `colour: isn't a section`, with the one meant when
  it's close, where it listed every section first; `operators: stronger` on one line says the list
  goes a line under it, and shows it; `- skp:` says there's a rule called `skip:`. In the ignores,
  a line with no colon gives an example, `reason: only logs`, an entry with a key close to the
  missing `reason:` says so, and `ignore --accept` or `--remove` of an id one slip from a recorded
  one names it, with the command. `MUTASTIC_INCREMENTAL` is in the help, and the configuration
  page says how an editor checks `mutastic.yaml` and rules files against their JSON Schemas.
- **The summary, `explain`, the brief card and SARIF read as the Java edition's.** The Mutants row
  ends in every mutant in brackets, after the skipped count, where the total came first and left
  the skipped out; `Left out`, the code not mutated, comes after Reports. A brief card shows the
  mutated line alone. `explain`'s operator row says what the operator does, as its docs section
  opens, where it repeated the change, and its rows wrap at 100 columns. A run of one mutant says
  1 worker, not 4. SARIF's rules have the operator's explanation, its first sentence as the short
  description, and `informationUri` is the docs.

## 1.2.1 (2026-11-18)

### Fixes

- **A Maven or Gradle build keeps its own JDK.** The Java edition needs JDK 25, so `mutastic`
  had you set `JAVA_HOME` to one, and the Java edition then ran your build and its tests on it
  too: an older Gradle can't run on JDK 25, and an older Mockito can't mock there. `mutastic`
  now finds a JDK for the Java edition alone, `MUTASTIC_JAVA_HOME` or else the newest JDK 25 or
  later of `JAVA_HOME` and those installed (SDKMAN, asdf, mise, Gradle's, IntelliJ's,
  Homebrew's, macOS's and Linux's; on Windows, Gradle's, IntelliJ's, Scoop's and the
  installers'), and hands it over as `MUTASTIC_JAVA_HOME`, leaving `JAVA_HOME` and `PATH` to the
  build. It looks before it runs a kept release too, not only before a download, and says where
  it looked when there's none. It needs the Java edition of this patch or later to keep the two
  apart. See [One command, two editions](docs/editions.md#the-java-edition).

## 1.2.0 (2026-11-15)

### Scoring changes

Expect your score to move, for these reasons, not because your tests changed:

- **A catch the mutant didn't cause is checked, and doesn't count: scores fall where catches were
  false.** Once the mutants have run, each test that caught one runs again, on its own with no
  mutant in. One that fails there (it trips on a file it, another test or the run left behind)
  didn't need the mutant to fail. It's named as flaky, as a test that "passes among the others but
  fails on its own"; its catches don't count, and every mutant it reached runs again without it,
  where the other covering tests decide, so a suite that keeps state no longer scores catches its
  tests didn't make. Unlike other flaky tests it fails the run whatever the score, with exit status
  2, and `verify` and `gate check` don't pass on it either. In `--format=jsonl`, a mutant run again
  comes as a `mutant-revised` event after the `mutant-finished` ones, naming the test as
  `falseCatchBy` where its catch didn't count, and the test's `flakyEvidence` entry has
  `"failsAlone": true`, as in the Java edition. A healthy suite's results don't change, at the cost
  of one run of each catching test. No id changes. See [A test that fails on its
  own](docs/concepts/results.md#a-test-that-fails-on-its-own).
- **A property test draws the same cases in every run.** mutastic sets `PROPTEST_RNG_SEED` for
  the tests it runs, unless you've set it, and turns proptest's saved failures off. A property that
  fails for some cases fails in every run or in none, so it can't stop one run, be called flaky in
  the next and catch a mutant in a third; and a case one of mutastic's runs saved can't decide its
  next runs. `proptest-regressions/` isn't copied, so the cases you've saved aren't replayed, and
  nothing is written to your tree. Where only a property test reaches a mutant, its result can
  change either way, as the cases drawn are now the seed's, and then stays the same from run to
  run; one caught only by a case saved in `proptest-regressions/` may now be missed.
- **A run of mutants asked for by id is judged on each one, not on a score,** as `verify` judges
  it and as the Java edition does. `mutastic run --only` passes when each mutant is caught,
  skipped or crashed; its summary has no `Score` row and no `Next:`, and ends on each fate, and
  `run-finished`'s `passed` says the same. It printed `Score 0.0% [FAIL]`, and its pass followed
  the score, so `--threshold=0` passed a mutant still missed.
- **The defaults are as they were.** No operator joins or leaves the default set, `stronger` or
  `extended`, and no filter skips more or less.

### HTML report

- **`mutation.html` is mutastic's own report page,** the same as the Java edition's. It replaces
  the generic viewer it embedded, and shows what the cards say: for each miss the change, why the
  tests let it through, the inputs they used, the test to add and the `mutastic verify` that proves
  it; tests to write, missed and not covered, with `j`/`k` through them; every mutant by status and
  operator; each test with its source and what it let through; code coloured for Rust. **Copy**
  puts a list's tests to write on the clipboard, a line each, for an issue or a pull request. It's
  still one file that opens from disk with no network. The old view isn't an option:
  `mutation.json` hasn't changed shape, so Stryker's report tools, which read it, still show it
  the old way. See [Report files](docs/reference/reports.md).

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

- **The gate takes an operator set.** `gate open` and `gate check` take `--operators`, as `run`
  does, and `mutastic.yaml` takes `gate:` with `operators:` under it, the gate's set in place of
  `operators:`, so a project can hold its agent's tasks to the stronger set without changing its
  everyday runs; the plugins' hooks need nothing new. The set is chosen when the gate opens and
  kept with the task: every check runs it, and a check given another set is refused (exit 2)
  rather than run on fewer operators than the task opened with. The open names the set, and the
  `mutastic run` it gives the agent carries `--operators` when it isn't what a run would choose;
  each verdict on the mutants says `(operators: stronger)`. See [the gate](docs/test-first-agent.md#which-operators).
- **A mutant's id opens the HTML report on it.** With links on, each id the terminal shows, on a
  card, in the misses list or in a command, links to a page beside the report,
  `mutation-mutants/<id>.html`, which opens `mutation.html` on that mutant. A link straight to
  `mutation.html#/m/<id>` loses its `#…` on macOS, whose opener drops it from a `file:` URL, and
  on Linux and Windows by reports; the page's own address has nothing to lose. Only a run that
  wrote the HTML report links its ids: not with `--report=none`, nor a share's. The Java edition
  does the same.
- **A run under other operators than the last says it isn't compared.** A score is a share of
  what its operators make, so a run is compared only with the last under the same set; with none,
  the score line said nothing. It now says `not compared with the last run, which used other
  operators (defaults)`, in the summary and the agent format; the `run-finished` event has no
  comparison, as before.
- **`mutation.json` says what the cards say.** A `mutastic` key at its top level gives the run's
  edition, the commands its output gives for a mutant (with `<id>`), where its docs are, its
  score, threshold, whether it passed, its toolchain and when it started; one on each mutant gives
  its function (`Pricing::discount()`), and for a miss the card's evidence and fix, the inputs seen
  and the test to add, with the value it passes each parameter. Both editions write the same,
  described by the schema `report/v1`, so one HTML report can read either. `testFiles` keys the
  tests by the file each is written in (`tests/pricing.rs`), with its text and where each test is
  declared, as the Java edition does, where it keyed them by module. A miss's `statusReason`
  keeps the headline's backticks. Tools that read the report schema read it as before. See
  [What mutastic adds](docs/reference/reports.md#what-mutastic-adds).
- **With more than one test to write, the agent format says how to take them.** After `rules:`
  and `decline:`, a `tasks:` line asks the agent to make a task list, one item per mutant grouped
  by file, then write each test, run its `mutastic verify` and mark it done before the next. The
  Java edition and the HTML report's Copy as prompt say the same.
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

- **A hooked gate check keeps within the hook's time.** The plugins run `gate check` from a Stop
  hook that's stopped after ten minutes, and a hook stopped gates nothing; on a slow crate the
  wait for the tests the task started with, up to four minutes, and the check's own run could pass
  that. As a hook, a check now waits for that list only as long as its budget leaves once a run as
  long as the last check's is set aside (`MUTASTIC_GATE_BUDGET`, in seconds, 600 unless set), and
  never longer than `MUTASTIC_GATE_COUNT_WAIT` seconds (240 unless set); when the list isn't done
  by then, it counts from the source, and a pass says so, as before. When the last check's own run
  took longer than the whole budget, the next hands over before it runs and says so: run
  `mutastic gate check` yourself. A check run by a person has no budget. The words and both
  variables are the Java edition's (decision 17 of the editions' contract).
- **Help is written from the commands' declarations, in the Java edition's layout.** `mutastic
  --help` was written by hand, and had fallen behind: it left out `--report=sarif`, the ignore and
  gate options, the environment and every exit code but a line's worth. Now both it and `mutastic
  <command> --help` are made from `src/commands.rs`, as the docs are: a `Usage:` line, what the
  command does, an `Exits` paragraph, examples, every option it takes, `--edition` included, with
  its values and variables, the exit codes, and its page on the docs site, every line within 100
  columns. An example too long for that is broken with ` \`, so it still pastes as one command.
  `mutastic` with nothing after it prints the help on stderr, not stdout, and exits 2 as before.
- **Synopses are written in capitals,** `mutastic verify ID... [DIR]` for `mutastic verify
  <id>[,<id>…] [<id>…] [dir]`, as the Java edition writes them and as option values already were
  (`--threshold=N`). They show in help, in `commands.json` and in the errors that give a usage.
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

- **The summary's report files open from where you ran mutastic.** Run from a parent directory
  on a crate, the `Reports` row said `target/mutastic/mutation.html`, which is there only from
  the crate, and isn't a link: a terminal that guessed one from the text opened
  `/target/mutastic/mutation.html`. Each file is now shown from the directory you ran in
  (`pricing/target/mutastic/mutation.html`), whole when it's outside it, and with links on it
  links to the file itself. The `tests.log` a failure names is linked the same way, and file
  links percent-encode what a URI can't hold, such as a space. `run-finished`'s `reports` are
  unchanged: relative to the directory given. The Java edition does the same.
- **`baseline --prune` doesn't drop what another set of operators would make.** A mutant only the
  baseline's operators make wasn't found by a run under others, and looked gone, so pruning after
  `operators:` changed dropped it. `.mutastic/baseline.yaml` now keeps the operators it was taken
  under, `operators: defaults`, and `--prune` under others says so and drops nothing; `run` no
  longer counts the baseline's mutants caught under others. A baseline written before keeps
  working as it did.
- **A baseline git doesn't see no longer holds the gate for good.** The check for baseline entries
  a task added itself compared the baseline with the start commit's, so in a project whose git
  ignores `.mutastic/`, where that commit has none, every entry of a local baseline counted as the
  task's and the gate never passed. `gate open` now keeps the baseline's ids, as it keeps the
  ignores, and a check compares with those (decision 16 of the editions' contract). A task opened
  by an older mutastic still compares with the start commit.
- **Each mutant's event goes out as it finishes.** In `--format=jsonl`, every `mutant-finished`
  came at the end of the run, after the `checking` phase, so a tool watching the stream saw
  nothing of the results until the run was over. Each now goes out as its mutant finishes, before
  the `mutants` phase ends, as in the Java edition (decision 7 of the editions' contract), in the
  order the mutants finish. A missed mutant's test to suggest is looked for as it's found missed,
  so its event still carries it, and `checking` is now the check of each catch on the code as it
  is, which every run makes: its phase events are always there, and `checked` is said only when a
  mutant was missed. A mutant run again without a test that fails on its own still gets its
  `mutant-revised` after the phases. The results, the score and the report files don't change.
  See [Events](docs/reference/events.md).
- **Each error's code is set where it's raised.** `run-failed`'s `code` was worked out afterwards
  from the error's words, and two errors got `other` that way: `--report=xyz` and `gate` with no
  action, which are now `option-value` and `unknown-command`. A mistyped gate action, `gate opn`,
  is said as that before mutastic looks for a project, so outside one it no longer answers "there's
  no project here". A file mutastic can't write is `other`, not `config`, and a git failure under
  `--since` is `git`.
- **A bad `--format` value ends the stream the environment asks for.** With
  `MUTASTIC_FORMAT=jsonl`, `--format=xyz` was refused on stderr with no `run-failed` event.
- **A panic ends as a failure.** A bug in mutastic exited 101 with Rust's own message and no
  event. It now says `internal error:`, where, and where to report it, writes a `run-failed` event
  with code `internal` in the agent and jsonl formats, and exits 2, as every failure does until
  2.0.
- **A property test's command sets its seed.** When a property test fails before any mutant, the
  command mutastic offers to run it alone sets `PROPTEST_RNG_SEED` to the seed mutastic drew its
  cases from, `PROPTEST_RNG_SEED=0 cargo test …`, and a line says so: without it, the test may pass
  when run again.
- **A property test's inputs don't flood a card.** Past 10 values, "inputs seen" shows the five
  smallest and five largest and how many: `a = -931, -923, -791, -777, -723, …, 884, 909, 948,
  950 (62 values)`. Where a property test passed a name that many values, the suggested test tries
  small numbers before them, so it reads `add_with_a_0_and_b_1`, not a value a generator drew.
- **`explain` on a caught return doesn't say no test fails.** The card for a caught or uncovered
  return-value mutant gave the missed card's note, "tests reach this return, but none fails when
  it returns `0`", under the test that caught it.
- **The test named as catching a mutant is the same in every run.** A mutant's covering tests run
  together, on several threads, and the first to fail was named: where several cases catch it, as
  cases written alike do, the name changed from run to run, and with it `mutation.json` and the
  report's tests. The test named is now the first of those that failed as the tests are listed,
  the library's before the other targets'.
- **The gate knows a case by the name it runs under.** rstest and test-case give each case a test
  of its own, `halves::case_1`, and no function has that name. A case that failed at a check was
  remembered without its source, and once it passed the gate said it was gone, at every check,
  until `gate close`. The gate now looks tests up by the names the test binaries list, and counts
  them from the same list, so taking a `#[case]` row away holds the gate as taking a test-case row
  away did. `gate open` returns at once and lists the tests in the background; a check waits for
  that list, up to four minutes, and where it failed (a package that doesn't build when the task
  starts) or isn't done, counts from the source, as before, and a pass says so.
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
