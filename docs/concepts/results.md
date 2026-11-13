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
  "durationMs": 4
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
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  35.7%   FAIL  Below the threshold of 80: 7 missed, 2 not covered.  · 41.7% where tests reach
```

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
that calls the function reaches them all.

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
| `arid:` | Code whose change no test should have to notice | `arid: only writes output`, for a `println!` or `log::warn!` statement removed |
| `equivalent:` | No test can tell it from the code | a `?` that only passes on a formatter's error |
| `user:` | The project's own decision, in `mutastic.yaml` or `.mutastic/ignores.yaml` | ``user: written by a code generator (rule `skip: shop::generated::**`)`` |

Code mutastic doesn't make mutants for at all, such as test code and code inside `unsafe`, isn't in
the count. The summary names what was left out around unsafe code (`not mutated: unsafe`, with how
many), and code under a `cfg` this build doesn't have (``not built here: `cfg(windows)` ``, with how
many), so you know what the score doesn't cover.

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

See [`mutastic verify`](../reference/commands.md#verify) for the command itself.

## Exit status

| Status | When |
|---|---|
| 0 | The run passed, or every mutant `verify` named is caught, skipped, or crashed where a crash proves nothing |
| 1 | The score is under the threshold, or a mutant `verify` named is missed or not covered |
| 2 | mutastic couldn't run: not a Cargo package, a test that fails with no mutant in, a build that fails for a reason no mutant explains |
