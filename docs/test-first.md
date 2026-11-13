# Working test-first

One cycle, on `fixtures/pricing`, with every command and what it prints: a test from the spec, the
least code that passes it, mutastic on the cycle's lines, the tests its cards ask for, and a
commit. [Working test-first](https://docs.mutastic.io/concepts/test-first/) says why mutastic
comes where it does, and why tests written first still need checking.

The spec for this cycle: loyal customers get 5 off orders of 30 or more.

## Before you start

Run these from the root of a clone of mutastic's repository, as in
[Getting started](getting-started.md). Start from a clean tree, and commit at the end of each
cycle, not before: `--since=HEAD` runs the mutants on the lines changed since the last commit,
uncommitted and new files included, so the commit marks where the cycle started.

## Write one failing test

In the `tests` module at the end of `fixtures/pricing/src/lib.rs`:

<!-- added to: fixtures/pricing/src/lib.rs -->
```rust
// Loyal customers get 5 off orders of 30 or more.
#[test]
fn loyal_customers_get_five_off() {
    assert_eq!(Pricing::new().loyalty_credit(50), 5);
}
```

And just enough code for it to compile, at the end of `impl Pricing`:

<!-- added to: fixtures/pricing/src/lib.rs -->
```rust
/// 5 off orders of 30 or more, for loyal customers.
pub fn loyalty_credit(&self, total: u32) -> u32 {
    0
}
```

The test fails, so mutastic won't run yet: a test that fails with no mutant in would look as if it
caught every mutant.

```sh
mutastic run fixtures/pricing
```

<!-- output: test-first/red -->
```text
mutastic: the tests fail without any mutant: tests::loyal_customers_get_five_off; their output is in target/mutastic/tests.log
```

## Make it pass with the least code

<!-- changed in: fixtures/pricing/src/lib.rs -->
```rust
/// 5 off orders of 30 or more, for loyal customers.
pub fn loyalty_credit(&self, total: u32) -> u32 {
    if total >= 30 {
        return 5;
    }
    0
}
```

The whole suite passes.

## Run mutastic on the cycle

```sh
mutastic run fixtures/pricing --since=HEAD
```

<!-- output: test-first/run -->
```text

 Missed ────────────────────────────────────────────────────────────

 ❯ src/lib.rs  2 missed in 1 function

  ✗ MISSED  no test checks loyalty_credit() at total == 30 · m525ce1d2
    ┌─ src/lib.rs:42
    │
 41 │     pub fn loyalty_credit(&self, total: u32) -> u32 {
 42 │         if total >= 30 {
    │                  ^^ >= changed to >
 43 │             return 5;
    │
    = mutant  - if total >= 30 {
              + if total > 30 {
    = also    ✗ no test needs total >= 30 to be false in loyalty_credit() · m076d6006
              + if true {
    = note: every test calls loyalty_credit() away from the boundary
            inputs seen: 50 (none is 30)
            covered by tests::loyal_customers_get_five_off (its 1 covering test still passes)
    = help: one test for each mutant

            test loyalty_credit() with total = 30 and assert the result · m525ce1d2
            // in mod tests
            #[test]
            fn loyalty_credit_at_the_total_edge() {
                assert_eq!(Pricing::new().loyalty_credit(30), 5); // today 5 · with m525ce1d2 0
            }

            add a test where total >= 30 is false, and assert what loyalty_credit() does then · m076d6006
            // in mod tests
            #[test]
            fn loyalty_credit_just_below_the_total_edge() {
                assert_eq!(Pricing::new().loyalty_credit(29), 0); // today 0 · with m076d6006 5
            }
            check today's value is the one you intend: the test pins what the code does now

    done when  mutastic verify m525ce1d2,m076d6006 fixtures/pricing passes
    reproduce  mutastic run --only=m525ce1d2,m076d6006 fixtures/pricing
    or ignore  mutastic ignore <id> --reason="<why>" fixtures/pricing for each of m525ce1d2, m076d6006

 Not covered (no test reaches these lines)
   ○ src/lib.rs:45 loyalty_credit() (1 mutant)

 Next: mutastic verify <id> fixtures/pricing · mutastic run --only=<id> fixtures/pricing · mutastic explain <id> fixtures/pricing

     Files  0 clean | 1 with misses (1)
   Mutants  ✓ 2 caught · ✗ 2 missed · ○ 1 not covered (5)
   Changed  14 lines in 1 file since HEAD
  Worst at  Pricing::loyalty_credit() 2 ✗
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  4 (16 without coverage selection)
  Start at  11:56:11
  Duration  428ms (setup 22ms, build 371ms, coverage 4ms, mutants 2ms, checking 6ms)
     Tests  4 on rustc 1.99.1 · 4 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  40.0%   FAIL  Below the threshold of 80: 2 missed, 1 not covered.  · 50.0% where tests reach
```

The one test is at 50, so nothing checks the edge at 30, nothing checks an order below it, and no
test reaches the last `0` at all: the code ran ahead of its tests. The card suggests a test for
each.

## Write the card's tests

Check each value against the spec before you keep it: an order of exactly 30 gets 5 off, and one of
29 gets nothing. Here both are what the spec asks for. Had one not been, the test would keep the
spec's value and fail on today's code: that's a bug found, and the cycle goes back to green. See
[Suggested tests](concepts/suggested-tests.md).

<!-- added to: fixtures/pricing/src/lib.rs -->
```rust
#[test]
fn loyalty_credit_at_the_total_edge() {
    assert_eq!(Pricing::new().loyalty_credit(30), 5);
}

#[test]
fn loyalty_credit_just_below_the_total_edge() {
    assert_eq!(Pricing::new().loyalty_credit(29), 0);
}
```

`verify` is the card's red and green: it fails until a test catches each mutant, and passes now.

```sh
mutastic verify m525ce1d2,m076d6006 fixtures/pricing
```

<!-- output: test-first/verify -->
```text
mutastic: m525ce1d2 is caught by tests::loyalty_credit_at_the_total_edge
mutastic: m076d6006 is caught by tests::loyalty_credit_just_below_the_total_edge
```

## A mutant you can't catch

A mutant no test could catch, because its case can't happen or no caller can see the change, is
proposed for a person to ignore, with its reason (see
[Equivalent mutants](https://docs.mutastic.io/concepts/equivalence/)). Don't change the code so
the mutant can't exist: that hides what it found.

## Refactor, run again, commit

Change the structure if it needs it, with the suite green, and run the cycle again:

```sh
mutastic run fixtures/pricing --since=HEAD
```

<!-- output: test-first/again -->
```text

     Files  1 clean (1)
   Mutants  ✓ 5 caught (5)
   Changed  24 lines in 1 file since HEAD
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  12 (30 without coverage selection)
  Start at  11:56:12
  Duration  401ms (setup 32ms, build 345ms, coverage 4ms, mutants 4ms)
     Tests  6 on rustc 1.99.1 · 4 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  100.0%   PASS  Every mutant was caught.
```

Then commit. The commit is the next cycle's `HEAD`.

## One card at a time

`mutastic next --since=HEAD` gives the cycle's missed mutants one at a time, the one most worth a
test first, in place of the whole run: see [Working through misses](concepts/next.md).

## Agents

A coding agent runs the same cycle through the [agent format](concepts/output.md#agent) and
[the gate](reference/commands.md#gate-open), whose one rule for it is the agent format's `rules:`
line: add or change tests only, and never change production code to make a mutant go away.
