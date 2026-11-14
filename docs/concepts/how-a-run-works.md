# How a run works

A mutation testing tool for Rust has a problem the Java edition doesn't: a change to the source
needs the compiler, and building a crate once per mutant takes too long. mutastic builds your crate
once, with every mutant in it, and picks which one is live when the tests run. What it says as it goes, on `fixtures/pricing`:

<!-- output: concepts/run-progress -->
```text
mutastic 1.1.0 · reading the Cargo package…
building  11 mutants in one build · 4 workers
built     11 mutants in 306ms · 2 rounds (check and build), none withdrawn
coverage  3 tests on rustc 1.99.1 in 2ms · 11 mutants
 ❯  pricing  src/lib.rs (11 mutants | 1 missed) 26ms
   × discount() :26 >= becomes >
ran       11 mutants in 8ms · ✓ 10 caught · ✗ 1 missed · ○ 0 not covered · ⏱ 0 timed out
checked   1 missed mutant in 4ms · a test to write for 1
```

Each phase says when it ends. While one runs, a terminal shows a line under these, redrawn every
tenth of a second with the phase's own clock: the crate Cargo is on while it builds, how many tests
the coverage pass has run, the mutants' counts so far. In a log, in CI or under a coding agent, the
same line is printed instead whenever 30 seconds pass without one, so a run is never quiet for
longer. `--progress` chooses.

## One copy, one build

mutastic never touches your source, your `Cargo.toml` or your `target/`. It copies the workspace to
`target/mutastic/copy`, beside a target directory of its own, and writes each mutant into the copy
behind a switch. A site with mutants becomes a `match` on the switch: the mutants are its arms, and
your code is the last one.

<!-- from: fixtures/pricing/src/lib.rs -->
```rust
        if total >= 100 {
```

becomes, in the copy:

<!-- output: concepts/schema-line -->
```text
if (match crate::__mutastic::at(2) { 2 => { (total) > (100) } 3 => { true } 4 => { false } _ => { total >= 100 } }) {
```

The switch reads `MUTASTIC_ACTIVE` once, as the test binary starts. With no mutant named, every
`match` takes your code's arm, so the copy behaves as your crate does. A small module at the end of
your crate root holds the switch; nothing is added to `Cargo.toml`, so a `--locked`, `--offline` or
vendored build works as it does for you.

The copy keeps your line numbers: the rewritten code never adds a line, so a panic's line and a
card's line are your file's. The copy is written only where its bytes change, so Cargo's own
freshness checks keep the next run's build short.

mutastic builds the copy with your own `cargo` and `rustc`, stable or not, from Rust 1.79 on: the
code it adds to your crate uses inline `const` blocks, which came then. Flags reach
only your crate, through `RUSTC_WORKSPACE_WRAPPER`, never `RUSTFLAGS`, so your own rustflags still
apply and no dependency is built again. The one flag added caps lints, so a `#![deny(warnings)]`
in your crate doesn't fail a build on code it was never meant to judge. The build uses a profile
of mutastic's own: see [Profiles](profiles.md).

## A mutant that doesn't compile is rolled back

Each mutant and your code are arms of one `match`, so they must have the same type. Most operators
only make changes that are sure to compile. A few rely on the compiler to say: an argument list
swapped where the types might differ, a call left out whose result might be another type, a `?`
turned into `.unwrap()`.

When the build fails, mutastic reads the compiler's errors, works out which mutants they belong
to, withdraws those, and builds again. An error in a mutant's own arm names that mutant. An error
around a guard, such as a borrow that a mutant's arm upsets, names the mutants in it. Only when an
error can't be placed closer does it name every mutant in its function. The rounds are `cargo
check`, and once that passes, a `cargo build`; the summary counts them, and a build that needed
nothing withdrawn takes two (`2 rounds (check and build)`).

A withdrawn mutant is skipped as unviable, with the compiler's error as its reason, and stays out
of the score: a change that doesn't compile is one no test needs to catch. See
[Troubleshooting](../troubleshooting.md#a-mutant-is-skipped-as-unviable) for what one looks like.

If the build fails for a reason no mutant explains, mutastic stops and prints the compiler's
error: your crate needs to build as it is first.

## What each test reaches

With the build done, mutastic lists every test in your package's test binaries: unit tests in
`#[cfg(test)]` modules, integration tests in `tests/`, and the tests of each binary. Tests marked
`#[ignore]` are left out. Then it runs each test once, on its own, with the switches recording
which sites it reaches and the whole numbers each function was called with. Those numbers are the
"inputs seen" on a card.

Every test must pass here. A test that fails with no mutant in would look as if it caught every
mutant it met, so a test that fails runs again, on its own, a few times. If it passes, it's flaky:
the run goes on without it, and says so (`! flaky:` before the summary, and a `Flaky` row), with
what it reaches that answers differently every run, such as `SystemTime::now` or a `RandomState`.
Its catches don't count, so code only it reaches is not covered. A test that fails every time
stops the run, with its name and where its output is.

## Each mutant against the tests that reach it

Each mutant then runs against only the tests that reach its site, in the test binary mutastic
built, with `MUTASTIC_ACTIVE` naming it. The first test to fail stops it: the mutant is caught. If
they all pass, it's missed. If none reach it, it isn't run, and it's not covered. Mutants run side
by side, one process each, on `--workers` threads.

Each run has a time limit, set from how long its tests took in the coverage pass, so a mutant
that makes a loop run forever is stopped and counted as timed out, which counts as caught.

A test process that dies without a test failing, by a signal or an abort, is run again one test at a
time to see how it died. What that means depends on where the mutant is: see
[Results](results.md#caught-only-because-the-test-process-crashed).

## When there's nothing to run

A run of a change that touched no mutant's line, of skipped mutants only, or of a share that holds
none has nothing to measure, and building the crate and running every test would take minutes to
find that out. So mutastic builds nothing and runs no test: it finds the mutants, lists the skipped
ones with their reasons, and passes with nothing to measure, in a second or so. On a clean tree:

```sh
mutastic run fixtures/pricing --since=HEAD
```

<!-- output: concepts/nothing-to-run -->
```text

    Files  0 clean (0)
  Mutants  ✓ 0 caught (0)
  Changed  0 lines in 0 files since HEAD
    Build  nothing built: no mutant to run
 Start at  19:31:48
 Duration  74ms (setup 74ms)
    Tests  none ran: no mutant to run
    Score  none  [PASS] Nothing to measure: no mutants to score.
```

Such a run leaves the last run's record and report files as they were. `--force-coverage` builds
the crate and runs the tests anyway, to check that they pass or to time them.

## Running it again

The copy and its build stay in `target/mutastic`, so the next run, or a `verify`, rebuilds only what
changed. `cargo clean` removes them with the rest of `target/`.

## Compared with the last run

Each full run keeps what it found in `target/mutastic/last-run.tsv`: every mutant's result and the
test that caught it, the score, and hashes of the code and of the mutastic that ran. The next run
under the same operators compares with it. A score is a share of what its operators make, so runs
under other operators aren't compared; the last run under each set you use stays in
`target/mutastic/last-runs/` until you come back to it, and the sets you've used least recently go
first. The score line says what changed in between: `same code`, `code changed` or `mutastic
changed`, beside the change in score (`= vs last run`, `▲ 2.0 vs last run`).

With the same code, tests and mutastic, every mutant should be caught or missed exactly as before.
One that changed result points at the test that caught it, in whichever run it was caught: that
test is named as `! likely flaky: …`, with the mutants whose results it changed. With anything
changed, a different result is just the change at work, and nothing is named. A mutant that's
skipped, or that crashed where a crash proves nothing, is out of the score in either run, so a
change to or from one isn't counted.

Only a full run compares and records: a run of one mutant (`--only`, `verify`, `explain`) says
nothing about the others, and a run of a change (`--since`) has a different score by design. A
run with no mutant to run measured nothing, so it isn't kept either. A
share (`--shard`) keeps its record in its own folder, so a whole run is never compared with part of
one.

## One run across CI jobs

A crate's run can take longer than one CI job should. `--shard=N/M` runs only the Nth of M shares of
its mutants, so M jobs side by side each run one share, and `--from-shards` puts their results back
together as the whole run:

```sh
mutastic run --shard=2/4 --threshold=0
mutastic run --from-shards=shares --threshold=70
```

Each mutant is dealt to a share by its id alone, so every mutant is in exactly one share, whichever
job runs it, and the shares come out about the same size, each with mutants from across the crate.
Each job still makes the one build, with only its share's mutants in, and runs every test in the
coverage pass, so a mutant's result is the one a whole run would give it. That build is each share's
setup, so splitting a run shares out the mutants' time, not the build's.

A share's score is only that share's, so give each share `--threshold=0` and the threshold you mean
to the run that puts them together. A share keeps everything that run needs in one folder,
`target/mutastic/shares/N-of-M/`: its results, as the [events](../reference/events.md)
`--format=jsonl` writes, whatever format it shows, and its own report files, so neither `explain`
nor the next run takes a share for the whole run.

`--from-shards=DIR` runs no mutants. It reads the package, finds the mutants as a whole run would,
takes each one's result from the share that ran it, wherever the share's folder is under `DIR`, and
then is that whole run: the same summary and cards in every format, the same report files, the
threshold checked on the whole score, and the whole run's `mutation.json`, which `explain` reads. A
share's folder name doesn't matter, so each job can upload its own and the last job download them
all into one directory.

It refuses, with exit status 2 and what's wrong, shares that aren't every share of one run: a share
missing or there twice, one that didn't finish, shares split into a different number, a mutant in
two shares, shares run by another mutastic or on other code than each other or than the package it
reads, shares of another package, or a share that ran only a change's lines with `--since`. Put them
together on the same commit, with the same options and `mutastic.yaml`. A share also fails on its
own, even with `--threshold=0`, when a test fails with no mutant in, as the whole run would.

In GitHub Actions, a matrix of shares, then one job that puts them together:

```yaml
jobs:
  mutants:
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        share: [1, 2, 3, 4]
    steps:
      - uses: actions/checkout@v5
      - run: |
          curl -fsSL https://mutastic.io/install.sh | sh
          echo "$HOME/.local/bin" >> "$GITHUB_PATH"
      - run: mutastic run --shard=${{ matrix.share }}/4 --threshold=0
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: share-${{ matrix.share }}
          path: target/mutastic/shares/${{ matrix.share }}-of-4/

  mutation-score:
    needs: mutants
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - run: |
          curl -fsSL https://mutastic.io/install.sh | sh
          echo "$HOME/.local/bin" >> "$GITHUB_PATH"
      - uses: actions/download-artifact@v4
        with:
          pattern: share-*
          path: shares
      - run: mutastic run --from-shards=shares --threshold=70
```

In GitLab CI, `parallel` runs a job as that many jobs, each told which it is in `CI_NODE_INDEX`,
counting from 1, and how many there are in `CI_NODE_TOTAL`. A job that `needs` it waits for all of
them, and gets each one's artifacts at the paths they were uploaded from, so every share's folder
lands under `target/mutastic/shares/`. The image is pinned by its version tag, and has the `curl`
the installer needs:

```yaml
mutants:
  image: rust:1.99.1
  parallel: 4
  before_script:
    - curl -fsSL https://mutastic.io/install.sh | sh
    - export PATH="$HOME/.local/bin:$PATH"
  script:
    - mutastic run --shard=$CI_NODE_INDEX/$CI_NODE_TOTAL --threshold=0
  artifacts:
    when: always
    paths:
      - target/mutastic/shares/

mutation-score:
  image: rust:1.99.1
  needs:
    - job: mutants
      artifacts: true
  before_script:
    - curl -fsSL https://mutastic.io/install.sh | sh
    - export PATH="$HOME/.local/bin:$PATH"
  script:
    - mutastic run --from-shards=target/mutastic/shares --threshold=70
```

When a share's job fails, the pipeline fails, and the last job doesn't run; when they all pass, the
last job fails if the whole score is below the threshold. The last job builds nothing: it installs
a release and puts the shares together. For a member of a workspace, name its directory in both
commands (`mutastic run crates/core --shard=…`) and upload the share from the workspace's
`target/mutastic/shares/…`.

## Brownfield: fail only on new misses

A package that comes to mutastic with years of code may have a score far below any threshold worth
holding, and a job that fails on every run is soon ignored. `--fail-on-new` fails the run only on
misses that are new: it passes while every mutant no test catches, missed or reached by none, is in
the package's [baseline](next.md#a-baseline-for-older-misses), and fails on any that isn't,
whatever the score. The threshold isn't applied; the score is still shown.

Take the baseline once, from `main`, and commit `.mutastic/baseline.yaml`:

```sh
mutastic baseline
```

Then CI runs with `--fail-on-new`:

```sh
mutastic run --fail-on-new
```

A failed run names each new miss by id: `[FAIL] 2 new misses, not in the baseline (12 in it): …`
in the summary, the same on the first line of `--format=agent`, whose entries put the new misses
first, and `failOnNew`, `newMisses` and `inBaseline` in `--format=jsonl`'s `run-finished`. Write
the test, or for a case that can't happen, [ignore](../reference/commands.md#ignore) the mutant:
the baseline is never rewritten to take in a new miss, which would hide it. Without a baseline,
`--fail-on-new` stops with exit status 2 and says `mutastic baseline` writes one, rather than guess
which misses are new.

As tests catch the baseline's mutants, a full run says how many, and `mutastic baseline --prune`
drops them, so the file shrinks and a mutant caught once can't be missed again unnoticed. Commit
the pruned file as you would any other change.

With `--since`, a run is judged on the new misses on the lines a pull request changed. A share of a
[split run](#one-run-across-ci-jobs) is judged on its own mutants, and `--from-shards`, given
`--fail-on-new`, judges the whole. `--fail-on-new` judges a run of the package, so it isn't taken
with `--only`.
