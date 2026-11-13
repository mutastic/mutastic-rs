# Test-first with a refactor

Two more cycles on the rule [Working test-first](test-first.md) started, each adding a tier, until
the code says the same thing three times. Then the refactor, done: mutastic on the cycle's lines
first, the change with the suite green, and mutastic on the lines it changed.
[Working test-first](https://docs.mutastic.io/concepts/test-first/), under Concepts on the docs
site, says why mutastic comes where it does.

The spec, a tier at a time: loyal customers get 5 off orders of 30 or more, 10 off orders of 100 or
more, and 20 off orders of 250 or more.

## Before you start

Start where [Working test-first](test-first.md) ends: `loyalty_credit` in `fixtures/pricing`,
with its three tests, and that cycle committed. Each cycle here ends with a commit too:
`--since=HEAD` runs the mutants on the lines changed since the last one.

## 10 off from 100

A test for the new tier, at the end of the `tests` module:

<!-- added to: fixtures/pricing/src/lib.rs -->
```rust
// 10 off orders of 100 or more.
#[test]
fn loyal_customers_get_ten_off_from_100() {
    assert_eq!(Pricing::new().loyalty_credit(150), 10);
}
```

It fails, as it should: the code gives 5.

```sh
mutastic run fixtures/pricing
```

<!-- output: test-first-refactor/red-100 -->
```text
mutastic: the tests fail without any mutant: tests::loyal_customers_get_ten_off_from_100; their output is in target/mutastic/tests.log
```

The least code that passes is a second `if`, above the first, as the higher tier wins. In place of
`loyalty_credit`:

<!-- changed in: fixtures/pricing/src/lib.rs -->
```rust
/// For loyal customers: 5 off orders of 30 or more, and 10 off from 100.
pub fn loyalty_credit(&self, total: u32) -> u32 {
    if total >= 100 {
        return 10;
    }
    if total >= 30 {
        return 5;
    }
    0
}
```

The whole suite passes. mutastic on the cycle's lines:

```sh
mutastic run fixtures/pricing --since=HEAD
```

<!-- output: test-first-refactor/run-100 -->
```text

 Missed ────────────────────────────────────────────────────────────

 ❯ src/lib.rs  1 missed in 1 function

  ✗ MISSED  no test checks loyalty_credit() at total == 100
    ┌─ src/lib.rs:42
    │
 41 │     pub fn loyalty_credit(&self, total: u32) -> u32 {
 42 │         if total >= 100 {
    │                  ^^ >= changed to >
 43 │             return 10;
    │
    = mutant  - if total >= 100 {
              + if total > 100 {
    = note: every test calls loyalty_credit() away from the boundary
            inputs seen: 29, 30, 50, 150 (none is 100)
            covered by tests::loyal_customers_get_five_off, tests::loyal_customers_get_ten_off_from_100, tests::loyalty_credit_at_the_total_edge, tests::loyalty_credit_just_below_the_total_edge (all 4 covering tests still pass)
    = help: test loyalty_credit() with total = 100 and assert the result

            // in mod tests
            #[test]
            fn loyalty_credit_at_the_total_edge_2() {
                assert_eq!(Pricing::new().loyalty_credit(100), 10); // today 10 · with this mutant 5
            }
            check today's value is the one you intend: the test pins what the code does now

    done when  mutastic verify m0d64ba6c fixtures/pricing passes
    reproduce  mutastic run --only=m0d64ba6c fixtures/pricing
    or ignore  mutastic ignore m0d64ba6c --reason="<why>" fixtures/pricing

 Next: mutastic verify <id> fixtures/pricing · mutastic run --only=<id> fixtures/pricing · mutastic explain <id> fixtures/pricing

     Files  0 clean | 1 with misses (1)
   Mutants  ✓ 3 caught · ✗ 1 missed (4)
   Changed  10 lines in 1 file since HEAD
  Worst at  Pricing::loyalty_credit() 1 ✗
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  13 (28 without coverage selection)
  Start at  12:17:40
  Duration  418ms (setup 29ms, build 358ms, coverage 4ms, mutants 4ms, checking 6ms)
     Tests  7 on rustc 1.99.1 · 4 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  75.0%   FAIL  Below the threshold of 80: 1 missed, 0 not covered.
```

The new test is at 150, so nothing checks the edge at 100. The new lines' other mutants are caught
by the tests there are: the one at 50 wants 5, not 10, and the new one wants 10, not 5 or 0.

The card's test, with its value checked against the spec: an order of exactly 100 gets 10 off. The
card calls it `loyalty_credit_at_the_total_edge_2`, as the edge at 30 has the first name, so name it
for its edge:

<!-- added to: fixtures/pricing/src/lib.rs -->
```rust
#[test]
fn loyalty_credit_at_the_100_edge() {
    assert_eq!(Pricing::new().loyalty_credit(100), 10);
}
```

```sh
mutastic verify m0d64ba6c fixtures/pricing
```

<!-- output: test-first-refactor/verify-100 -->
```text
mutastic: m0d64ba6c is caught by tests::loyalty_credit_at_the_100_edge
```

Two `if`s are not yet a pattern, so there's nothing to refactor. Commit.

## 20 off from 250

The same cycle, a tier up. The test:

<!-- added to: fixtures/pricing/src/lib.rs -->
```rust
// 20 off orders of 250 or more.
#[test]
fn loyal_customers_get_twenty_off_from_250() {
    assert_eq!(Pricing::new().loyalty_credit(300), 20);
}
```

```sh
mutastic run fixtures/pricing
```

<!-- output: test-first-refactor/red-250 -->
```text
mutastic: the tests fail without any mutant: tests::loyal_customers_get_twenty_off_from_250; their output is in target/mutastic/tests.log
```

A third `if`, above the others:

<!-- changed in: fixtures/pricing/src/lib.rs -->
```rust
/// For loyal customers: 5 off orders of 30 or more, 10 off from 100, and 20 off from 250.
pub fn loyalty_credit(&self, total: u32) -> u32 {
    if total >= 250 {
        return 20;
    }
    if total >= 100 {
        return 10;
    }
    if total >= 30 {
        return 5;
    }
    0
}
```

```sh
mutastic run fixtures/pricing --since=HEAD
```

<!-- output: test-first-refactor/run-250 -->
```text

 Missed ────────────────────────────────────────────────────────────

 ❯ src/lib.rs  1 missed in 1 function

  ✗ MISSED  no test checks loyalty_credit() at total == 250
    ┌─ src/lib.rs:42
    │
 41 │     pub fn loyalty_credit(&self, total: u32) -> u32 {
 42 │         if total >= 250 {
    │                  ^^ >= changed to >
 43 │             return 20;
    │
    = mutant  - if total >= 250 {
              + if total > 250 {
    = note: every test calls loyalty_credit() away from the boundary
            inputs seen: 29, 30, 50, 100, 150, 300 (none is 250)
            covered by tests::loyal_customers_get_five_off, tests::loyal_customers_get_ten_off_from_100, tests::loyal_customers_get_twenty_off_from_250, tests::loyalty_credit_at_the_100_edge, tests::loyalty_credit_at_the_total_edge, tests::loyalty_credit_just_below_the_total_edge (all 6 covering tests still pass)
    = help: test loyalty_credit() with total = 250 and assert the result

            // in mod tests
            #[test]
            fn loyalty_credit_at_the_total_edge_2() {
                assert_eq!(Pricing::new().loyalty_credit(250), 20); // today 20 · with this mutant 10
            }
            check today's value is the one you intend: the test pins what the code does now

    done when  mutastic verify m97fbc9b1 fixtures/pricing passes
    reproduce  mutastic run --only=m97fbc9b1 fixtures/pricing
    or ignore  mutastic ignore m97fbc9b1 --reason="<why>" fixtures/pricing

 Next: mutastic verify <id> fixtures/pricing · mutastic run --only=<id> fixtures/pricing · mutastic explain <id> fixtures/pricing

     Files  0 clean | 1 with misses (1)
   Mutants  ✓ 3 caught · ✗ 1 missed (4)
   Changed  10 lines in 1 file since HEAD
  Worst at  Pricing::loyalty_credit() 1 ✗
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  19 (36 without coverage selection)
  Start at  12:17:42
  Duration  339ms (setup 25ms, build 285ms, coverage 6ms, mutants 2ms, checking 6ms)
     Tests  9 on rustc 1.99.1 · 4 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  75.0%   FAIL  Below the threshold of 80: 1 missed, 0 not covered.
```

The same gap, at the new edge. An order of exactly 250 gets 20 off:

<!-- added to: fixtures/pricing/src/lib.rs -->
```rust
#[test]
fn loyalty_credit_at_the_250_edge() {
    assert_eq!(Pricing::new().loyalty_credit(250), 20);
}
```

```sh
mutastic verify m97fbc9b1 fixtures/pricing
```

<!-- output: test-first-refactor/verify-250 -->
```text
mutastic: m97fbc9b1 is caught by tests::loyalty_credit_at_the_250_edge
```

Don't commit yet: this cycle has a refactor in it.

## Refactor

The three `if`s say one thing three times: from this total up, this credit, the highest tier first.
A fourth tier would be a fourth copy. The tiers are data, so the refactor is to hold them as data, a
table, read by one loop.

### Start from green

A refactor starts where the cycle is done: the suite passes, and every mutant on the cycle's lines
is caught.

```sh
mutastic run fixtures/pricing --since=HEAD
```

<!-- output: test-first-refactor/before -->
```text

     Files  1 clean (1)
   Mutants  ✓ 4 caught (4)
   Changed  15 lines in 1 file since HEAD
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  23 (40 without coverage selection)
  Start at  12:17:43
  Duration  327ms (setup 21ms, build 283ms, coverage 6ms, mutants 2ms)
     Tests  10 on rustc 1.99.1 · 4 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  100.0%   PASS  Every mutant was caught.
```

The four are the new tier's: its `>=` changed to `>`, its condition always true and always false,
and its `20` returned as `0`.

### Change the structure

In place of `loyalty_credit`, the table and the loop:

<!-- changed in: fixtures/pricing/src/lib.rs -->
```rust
/// The loyalty tiers, highest first: from this total up, this credit.
const LOYALTY_TIERS: [(u32, u32); 3] = [(250, 20), (100, 10), (30, 5)];

/// For loyal customers: 5 off orders of 30 or more, 10 off from 100, and 20 off from 250.
pub fn loyalty_credit(&self, total: u32) -> u32 {
    for (from, credit) in Self::LOYALTY_TIERS {
        if total >= from {
            return credit;
        }
    }
    0
}
```

No test changes: a refactor changes structure only, and the tests are what say it kept the
behaviour. The whole suite passes.

### Run mutastic again

```sh
mutastic run fixtures/pricing --since=HEAD
```

<!-- output: test-first-refactor/after -->
```text

     Files  1 clean (1)
   Mutants  ✓ 4 caught (4)
   Changed  19 lines in 1 file since HEAD
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  27 (40 without coverage selection)
  Start at  12:17:43
  Duration  339ms (setup 22ms, build 291ms, coverage 6ms, mutants 4ms)
     Tests  10 on rustc 1.99.1 · 4 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  100.0%   PASS  Every mutant was caught.
```

The table and the loop are new since `HEAD`, so these are the new shape's mutants. The report,
`mutation.html`, shows each on its line:

- `total >= from` changed to `total > from`: one mutant for the edge of every tier, where the `if`s
  had one each.
- `total >= from` always true, and always false.
- `credit` returned as `0`.

The table's numbers have no mutants, as the numbers in the `if`s had none: the tests at each tier
and its edge are what check them. The loop gives none either. It does depend on the table's order,
highest first, as the `if`s did on theirs, and the tests pin that too: with the lowest tier first,
an order of 150 would get 5, and the test that wants 10 fails.

All four are caught. The tests still pin every behaviour of the new shape, with none of them
changed: the refactor is safe. Commit; the commit is the next cycle's `HEAD`.

The ids aren't the ones before the refactor. An id is a hash of what its mutant is, and every
mutant here is on code that's new (see [Mutant ids](concepts/ids.md)). Compare what's caught, not
ids.

## Had a mutant been missed

Before the refactor, the tests caught every mutant on the cycle's lines. A mutant missed after it
means one of two things. The refactor wasn't only structure: it added behaviour no test asks for,
such as a default for a case the `if`s never had. Or the tests stopped pinning something: a test
was changed or taken out with the refactor, which a refactor never needs.

Either way, don't write a test for the miss there and then. Undo that part of the change. If the
behaviour is one you want, add it test first, in a cycle of its own, where its test fails until the
code does it.
