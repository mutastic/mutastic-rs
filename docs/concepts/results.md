# Results and the score

A **mutant** is your code with one small change in it, `>=` becoming `>` say. mutastic runs the
tests that reach the change. If one fails, the tests noticed: the mutant is **caught**. If they all
pass, nothing checks what that code does: the mutant is **missed**, and that's a gap in the tests.

## What happens to a mutant

| Result | Symbol | Means | In the score |
|---|---|---|---|
| Caught | ✓ | A test failed with the mutant in, or the test process crashed in safe code | caught |
| Missed | ✗ | Every test that reaches it still passes: a gap in the tests | missed |
| Timed out | ⏱ | The tests stopped finishing in time, usually a loop that no longer ends | caught |
| Not covered | ○ | No test reaches it | missed |
| Skipped | | Never run: it doesn't compile, or no test should have to notice it | left out |
| Crashed where a crash proves nothing | | The process died where the crash may be undefined behaviour, or where std's own check stopped it | left out |

Colour is never the only signal: each result also has its symbol or its words. The event stream uses
the standard names from the
[mutation-testing report schema](https://github.com/stryker-mutator/mutation-testing-elements/tree/master/packages/report-schema):
`Killed`, `Survived`, `Timeout`, `NoCoverage`, `Ignored` and `RuntimeError`.

## How a test catches a mutant

A test catches a mutant by failing: an `assert!` that doesn't hold, a panic, an `unwrap` on `None`,
a `#[should_panic]` test that no longer panics. mutastic says which test, and how, when the language
raised the panic itself. A test that fails only because the arithmetic overflowed is still a catch,
but worth knowing about: see [Profiles](profiles.md#why-overflow-checks-are-forced).

### When the code a test runs exits

A test that calls what `main` calls, in the test's own process, runs whatever it does, exits
included. If a mutant makes it call `std::process::exit` where it didn't, the test never finishes.
What that means depends on the status, as it does under `cargo test`:

- **Any status but 0 fails the run,** so the mutant is caught by the test that was running, by the
  exit, not a crash. The card says `caught by exits::tests::prints_the_discount, by the process
  exiting with status 2`, and in the event stream `caughtWith` is `exit: 2`.
- **Status 0 passes it,** though no test after it runs. So it isn't a catch: the tests after that
  one run without it, and decide.

A test that hangs is still timed out.

## Code reached only from main

`main` is often the one piece of code no test reaches: it reads the process's arguments, prints,
and sets the exit status, all of which a test finds awkward to check. So it reads as not covered,
and the line says so, with what to do:

```text
○ src/main.rs:5-8 main() (5 mutants)
  reached only from main(): move what it does into run(args, out), which returns the exit code,
  test run, and leave main() a one-line call to it
```

A test can then call `run` with the arguments it likes and an `out` it reads back, a `Vec<u8>`
say, and check the exit code it returns. What's left in `main`, the one line, stays not covered,
and gets no such advice. mutastic doesn't leave `main` out of the run for you: what it does matters
too.

## Tables of cases and property tests

A test often runs one check over a table of cases. What mutastic makes of it depends on how the
table is written:

- **Each rstest `#[case]` and test-case row is a test of its own,** as libtest lists it:
  `rstest_tests::discounts::case_2_at_the_edge`. A card says which case caught a mutant, or reached
  it and let it through, and the gate counts each case, so taking a row away is taking a test away.
  The number in a name is the row's place, so name your cases, `#[case::at_the_edge(100, 10)]`, and
  a row put in above doesn't rename the others. `verify` and `explain` show the row as written
  after the test that caught a mutant, `rstest_tests::discounts::case_2_at_the_edge (case
  #[case::at_the_edge(100, 10)])`, as does a test that fails with no mutant in, and the
  `mutant-finished` event has it as `caughtCase`.
- **A loop over rows in one `#[test]` is one test,** however many rows it has, and a card can't say
  which row failed. Put the row in the assertion's message,
  `assert_eq!(subtract(a, b), difference, "subtract({a}, {b})")`, so the test's own output does.
- **A proptest property draws the same cases every run,** as mutastic seeds it, so it catches a
  mutant in every run or in none. A card's `inputs seen` shows its smallest and largest few values.

See [Tables of cases and property tests](../troubleshooting.md#tables-of-cases-and-property-tests).

## Caught only because the test process crashed

Sometimes no test fails: the test process dies instead, by `std::process::abort()`, a panic that
can't unwind, or a stack overflow. mutastic runs the tests again one at a time to see how, and what
it means depends on where the mutant is.

**In safe code,** a crash is the mutant's doing and nothing else's, so the mutant is caught. But no
test's check caught it: the process died before any test could pass or fail. So no test is named,
and mutastic says so plainly:

<!-- output: concepts/verify-crash -->
```text
mutastic: m418fd2e4 is caught only because the test process crashed (SIGABRT): no test's check caught it
```

The summary counts these apart, on a line of their own (`caught only because the test process
crashed, not by a test's check`), and in the event stream the mutant has `caughtWith` starting
`crash:` and no `caughtBy`:

<!-- output: concepts/crash-event -->
```text
{
  "schema": "mutastic.events/v1",
  "edition": "rust",
  "reason": "mutant-finished",
  "id": "m418fd2e4",
  "fingerprint": "418fd2e4aad78dcd",
  "status": "Killed",
  "operator": "mutastic/remove-conditional",
  "docs": "https://docs.mutastic.io/rust/operators/conditions/#remove-conditional",
  "location": {
    "file": "src/plain.rs",
    "line": 5,
    "column": 8,
    "class": "plain",
    "method": "guarded"
  },
  "mutation": {
    "original": "x < 0",
    "replacement": "true",
    "description": "`x < 0` is always true",
    "applied": {
      "level": "source",
      "text": "x < 0 -> true"
    }
  },
  "coveredBy": [
    "run::guards"
  ],
  "caughtWith": "crash: SIGABRT",
  "inputs": {
    "x": [
      3
    ]
  },
  "testsRun": 1,
  "durationMs": 8
}
```

It counts as caught: the change does show, as the tests can't pass with it, and there's nothing to
fix. Here, `if true` makes `guarded()` abort on every call, before the test's `assert_eq!` gets to
look at the answer.

**Near unsafe code,** a crash may be undefined behaviour, which proves nothing about the tests. The
mutant is a `RuntimeError`, left out of the score. So is a mutant anywhere that std's own debug check
of an unsafe precondition stopped. See [Unsafe code](unsafe.md).

This is the unsafety fixture, which has one of each:

<!-- output: concepts/unsafe-run -->
```text

 Missed ────────────────────────────────────────────────────────────

 ❯ src/plain.rs  2 missed in 1 function

  ✗ MISSED  no test checks guarded() at x == 0 · mdb89abdf
   ┌─ src/plain.rs:5
   │
 4 │ pub fn guarded(x: i32) -> i32 {
 5 │     if x < 0 {
   │          ^ < changed to <=
 6 │         std::process::abort();
   │
   = mutant  - if x < 0 {
             + if x <= 0 {
   = also    ✗ no test needs x < 0 to be true in guarded() · m476f7baf
             + if false {
   = note: every test calls guarded() away from the boundary
           inputs seen: 3 (none is 0)
           covered by run::guards (its 1 covering test still passes)
   = help: test guarded() with x = 0 and assert the result

   done when  mutastic verify mdb89abdf,m476f7baf fixtures/unsafety passes
   reproduce  mutastic run --only=mdb89abdf,m476f7baf fixtures/unsafety
   or ignore  mutastic ignore <id> --reason="<why>" fixtures/unsafety for each of mdb89abdf, m476f7baf

 ❯ src/raw.rs  5 missed in 2 functions

  ✗ MISSED  no test checks positive() at x == 0 · m97f387f6
   ┌─ src/raw.rs:5
   │
 4 │ pub fn positive(x: i32) -> bool {
 5 │     x > 0
   │       ^ > changed to >=
 6 │ }
   │
   = mutant  - x > 0
             + x >= 0
   = also    ✗ no test checks positive() returning false here · m2fcb19d0
             + true
   = also    ✗ no test needs x > 0 to be false in positive() · mbff60c78
             + true
   = note: every test calls positive() away from the boundary
           inputs seen: 4 (none is 0)
           this code is near unsafe code (called from unsafe code): a change no test notices here can break what the unsafe code relies on, so check that code with Miri too
           covered by run::halves (its 1 covering test still passes)
   = help: test positive() with x = 0 and assert the result

   done when  mutastic verify m97f387f6,m2fcb19d0,mbff60c78 fixtures/unsafety passes
   reproduce  mutastic run --only=m97f387f6,m2fcb19d0,mbff60c78 fixtures/unsafety
   or ignore  mutastic ignore <id> --reason="<why>" fixtures/unsafety for each of m97f387f6, m2fcb19d0, mbff60c78

  ✗ MISSED  no test checks checked() at x == 0 · m68f8c8e3
    ┌─ src/raw.rs:24
    │
 23 │ pub fn checked(x: i32) -> i32 {
 24 │     if x < 0 {
    │          ^ < changed to <=
 25 │         std::process::abort();
    │
    = mutant  - if x < 0 {
              + if x <= 0 {
    = also    ✗ no test needs x < 0 to be true in checked() · mbb05698b
              + if false {
    = note: every test calls checked() away from the boundary
            inputs seen: 3 (none is 0)
            this code is near unsafe code (in a module with unsafe code): a change no test notices here can break what the unsafe code relies on, so check that code with Miri too
            covered by run::checks (its 1 covering test still passes)
    = help: test checked() with x = 0 and assert the result

    done when  mutastic verify m68f8c8e3,mbb05698b fixtures/unsafety passes
    reproduce  mutastic run --only=m68f8c8e3,mbb05698b fixtures/unsafety
    or ignore  mutastic ignore <id> --reason="<why>" fixtures/unsafety for each of m68f8c8e3, mbb05698b

 Not covered (no test reaches these lines)
   ○ src/raw.rs:25 checked() (1 mutant)
   ○ src/plain.rs:6 guarded() (1 mutant)

 Next: mutastic verify <id> fixtures/unsafety · mutastic run --only=<id> fixtures/unsafety · mutastic explain <id> fixtures/unsafety

     Files  0 clean | 2 with misses (2)
   Mutants  ✓ 5 caught · ✗ 7 missed · ○ 2 not covered (14) · 3 crashed where a crash proves nothing
  Left out  not mutated: unsafe (4), in or around unsafe blocks; --unsafe=enclosing or --unsafe=all includes them
  Worst at  raw::positive() 3 ✗ · raw::checked() 2 ✗ · plain::guarded() 2 ✗
   Crashes  1 caught only because the test process crashed, not by a test's check
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  15 (90 without coverage selection)
  Start at  11:03:14
  Duration  334ms (setup 20ms, build 279ms, coverage 3ms, mutants 10ms, checking 6ms)
     Tests  6 on rustc 1.99.1 · 4 workers
   Reports  fixtures/unsafety/target/mutastic/mutation.json · fixtures/unsafety/target/mutastic/mutation.html
     Score  35.7%   FAIL  Below the threshold of 80: 7 missed, 2 not covered.  · 41.7% where tests reach
```

## A test that fails on its own

A catch is only the mutant's doing if the test passes without it. The coverage pass checks that
before any mutant, but a test can pass then and still fail later on code with no mutant in: it
trips on something an earlier run of it, or of another test, left behind, most often a file it
writes and never removes. Against every mutant it reaches, it fails, and each failure would look
like a catch.

So once every mutant has its result, each test that caught one runs again, once, on its own with
no mutant in. One that fails there passed among the others but fails on its own, so its catches
weren't the mutants'. A test that runs out of time there isn't counted, as load can do that.

Its catches don't count. It's left out, as a flaky test is, and each mutant it reached runs again
without it: the other tests that reach the mutant decide whether it's caught or missed, and if none
does, it's not covered. The mutant keeps its id, and its card says which test's catch didn't count
(`… failed against it, but fails on its own with no mutant in too: that catch doesn't count`).

The run doesn't pass while such a test is there, whatever the score, and exits with status 3, as
for a test that fails with no mutant in. `verify` and `gate check` check their catches the same
way, so neither passes on a false catch. The output names the test as flaky, as one that `passes
among the others but fails on its own`, and says the run can't pass until it does. In
`--format=jsonl`, `run-finished` lists it in `flaky`, its entry in `flakyEvidence` has
`"failsAlone": true`, and each mutant run again is sent again as a `mutant-revised` event with its
last result, which replaces its `mutant-finished`; one whose catch didn't count names the test as
`falseCatchBy`.

When every catch holds, nothing changes but the time: one run of each test that caught something.

## The score

The score is the share of the mutants that ran which the tests noticed:

```text
score = (caught + timed out) / (every mutant − skipped − crashed where a crash proves nothing) × 100
```

A mutant no test reaches counts against the score, as one no test checks does. A run passes when
the score reaches the threshold, which `--threshold` sets (its default is in `mutastic --help`); a
run with nothing to score says so, rather than claiming 100%.

When some code is out of every test's reach, the summary also gives the score of the code the tests
do reach (`where tests reach` beside the score, and `coveredScore` in the event stream): how well the tests
check what they run, apart from what they don't. The threshold is always on the whole score. The
code no test reaches is listed once per function, with the lines its mutants are on, since one test
that calls the function reaches them all, and where mutastic finds one, `try:` gives an assertion
to start it with: see [Code no test reaches](suggested-tests.md#code-no-test-reaches).

## Skipped mutants

A mutant is skipped when it doesn't compile: the build withdrew it, and its reason is the compiler's
error, `unviable: doesn't compile (E0308: mismatched types)`. A change that doesn't compile is one
no test needs to catch, so it's left out of the score. See
[How a run works](how-a-run-works.md#a-mutant-that-doesnt-compile-is-rolled-back).

A skipped mutant's reason starts with its kind, and the summary counts them by kind,
`2 skipped (1 arid, 1 unviable)`:

| Kind | Means | Example |
|---|---|---|
| `unviable:` | It doesn't compile | `unviable: doesn't compile (E0308: mismatched types)` |
| `arid:` | Code whose change no test should have to notice | `arid: only writes output`, for a `println!` or `log::warn!` statement removed; `arid: only feeds a log line`, for a `let` that only output macros read; `arid: only decides whether output is written`, for `if log_enabled!(Level::Debug)` around nothing but output; `arid: only calls audit::record (<reason>)`, for a call an [`output:` rule](../reference/configuration.md) names; `arid: only checks the value is there`, for an `x.unwrap();` statement removed; `arid: only names a thread`, for a `thread::Builder`'s name; `arid: only sizes a collection's first allocation`, for a number given to `Vec::with_capacity`; [each rule](https://docs.mutastic.io/reference/arid/) |
| `equivalent:` | No test can tell it from the code | a `?` that only passes on a formatter's error |
| `user:` | The project's own decision, in `mutastic.yaml` or `.mutastic/ignores.yaml` | ``user: written by a code generator (rule `skip: shop::generated::**`)`` |
| `twin:` | It always does what another mutant at its place does, and that one runs for it ([Twins](#twins)) | `twin: same as m2e329a72`, for `n < 10`, which `is_small` returns, made `true` by `mutastic/remove-conditional`: the same code `mutastic/return-value` makes |

Code mutastic doesn't make mutants for at all, such as test code and code inside `unsafe`, isn't in
the count. The summary names what was left out around unsafe code (`not mutated: unsafe`, with how
many), and code under a `cfg` this build doesn't have (``not built here: `cfg(windows)` ``, with how
many), so you know what the score doesn't cover.

### Twins

Two mutants at one place can always do the same. Running both would cost a second build and run,
and count one gap twice. So only one of them runs, and the other is skipped as `twin: same as
<id>`, naming the one that runs. Its result is that one's, and the place counts once in the score.

- **The same code.** A function that ends in `n > 0` gets `true` from `mutastic/return-value` and
  from `mutastic/remove-conditional`. It's one change, made twice.
- **A comparison of an unsigned value with 0.** The value can't be below 0, so `n > 0` made `n >=
  0` is always true, as `n > 0` made `true` is.

A twin keeps its id, and the id keeps working. `verify`, `run --only` and `explain` run the one
it's the same as, and say so: `mutastic: m729a5380 is the same as m2e329a72, which is still
missed`. `mutastic ignore` records the decision for that one, and an ignore you made of either
holds for both. A run on changed lines that has a twin runs the one it's the same as too.

The one that runs is the first of them in `defaults`, then `stronger`, then `extended`, in the
order `mutastic mutants` lists them. A run whose operators leave it out runs the twin as it would
any mutant.

## What `verify` says

`mutastic verify` runs only the mutants named, and passes when each is settled. One line each says
how it went:

| It prints | Means | Passes |
|---|---|---|
| `is caught by tests::discount_at_the_total_edge` | a test failed | yes |
| `is caught by run::totals, through a panic: attempt to subtract with overflow` | a test failed through a panic the language raised | yes |
| `is caught only because the test process crashed (SIGABRT): no test's check caught it` | no test failed: the process died, in safe code | yes |
| `is caught by a timeout` | the tests ran out of time | yes |
| `is still missed` | every test that reaches it passed | no |
| `isn't reached by any test` | no test reaches it | no |
| `is skipped (unviable: …), so it isn't run and needs no test` | it doesn't compile | yes |
| `crashes, and no test can be said to catch it (…)` | it crashed where a crash proves nothing | yes |
| `is the same as m2e329a72, which is still missed` | a twin: the one it's the same as ran for it, and its fate follows | as that one |

See [`mutastic verify`](../reference/commands.md#verify) for the command itself.

## Exit status

| Status | When |
|---|---|
| 0 | The run passed, or every mutant `verify` named is caught, skipped, or crashed where a crash proves nothing |
| 1 | The score is under the threshold, or a mutant `verify` named is missed or not covered |
| 2 | mutastic couldn't run: not a Cargo package, a build that fails for a reason no mutant explains, a mistake in the command |
| 3 | A test fails with no mutant in, so the run stops; or a test that caught mutants fails on its own, with no mutant in, so its catches can't be trusted. Leaving it out with a reason, under `exclude-tests:` in `mutastic.yaml`, is the way past it |
