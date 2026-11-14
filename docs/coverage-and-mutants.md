# Coverage and mutants

A test can run every line of your code, and both sides of every `if`, without checking what any of
it does. Coverage scores it the same as a test that checks everything, so the report is green
while the tests would pass on code that's wrong. mutastic asks the question coverage can't: it
puts a small bug in the code, runs the tests, and tells you when they don't notice.

This page measures one small crate both ways, with tests that check too little and tests that
check the results. Coverage can't tell them apart, and mutastic can. Then it shows where mutants
reach past branch coverage even for good tests, and when `--operators=stronger` finds more than
the defaults.

## Three ways to measure tests

- **Line coverage** asks which lines ran. A line no test runs is untested, for sure. A line that
  ran may be untested too: running it isn't checking it.
- **Branch coverage** asks which way each decision went: whether the tests took each `if` both
  ways. It finds the side of a condition no test takes. It still doesn't look at what the tests
  assert.
- **Mutation testing** asks whether the tests would notice a bug. mutastic makes one small change
  to the code at a time, a mutant, and runs the tests that reach it. A mutant they catch is code
  they check. One they miss is a gap, on a line coverage counts as tested.

Coverage is cheap, and it tells you where tests never go. Only mutants tell you whether the tests
check what they reach.

## The example

The `checkout` fixture works out what an order costs:

<!-- from: fixtures/checkout/src/lib.rs -->
```rust
/// 10% off orders of 100 or more.
pub fn discount(total: u32) -> u32 {
    if total >= 100 {
        return total / 10;
    }
    0
}

/// What `quantity` items at `price` each cost, less any discount, which goes on the receipt.
pub fn pay(quantity: u32, price: u32, receipt: &mut Vec<String>) -> u32 {
    let total = quantity * price;
    let off = discount(total);
    receipt.push(format!("discount {off}"));
    total - off
}
```

## Two sets of tests, the same coverage

Its tests run every line, and take the `if` both ways: a big order gets the discount and a small
one doesn't. But one checks only that a big order costs less than 250, and the other checks
nothing at all:

<!-- from: fixtures/checkout/tests/checkout.rs -->
```rust
#[test]
fn big_orders_get_a_discount() {
    let mut receipt = Vec::new();
    assert!(pay(1, 250, &mut receipt) < 250);
}

#[test]
fn small_orders_pay_full_price() {
    pay(1, 50, &mut Vec::new());
}
```

These check the results: what each order costs, and what goes on the receipt.

<!-- changed in: fixtures/checkout/tests/checkout.rs -->
```rust
#[test]
fn big_orders_get_a_discount() {
    let mut receipt = Vec::new();
    assert_eq!(pay(1, 250, &mut receipt), 225);
    assert_eq!(receipt, ["discount 25"]);
}

#[test]
fn small_orders_pay_full_price() {
    let mut receipt = Vec::new();
    assert_eq!(pay(1, 50, &mut receipt), 50);
    assert_eq!(receipt, ["discount 0"]);
}
```

[`cargo llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) measures their coverage:

```sh
cargo llvm-cov --manifest-path fixtures/checkout/Cargo.toml --lcov --summary-only --remap-path-prefix
```

With the tests that check too little:

<!-- output: coverage-and-mutants/llvm-cov-weak -->
```text
SF:src/lib.rs
FNF:2
FNH:2
BRF:0
BRH:0
LF:12
LH:12
end_of_record
```

With the tests that check the results:

<!-- output: coverage-and-mutants/llvm-cov-strong -->
```text
SF:src/lib.rs
FNF:2
FNH:2
BRF:0
BRH:0
LF:12
LH:12
end_of_record
```

The same, to the last line. In lcov's terms, `LF` is the lines found and `LH` the lines hit, and
`FNF` and `FNH` the same for functions: every one ran, both times. `BRF` and `BRH`, the branches,
are 0 because branch coverage in Rust needs a nightly compiler (`cargo +nightly llvm-cov
--branch`).

## What mutastic sees

The tests that check too little:

```sh
mutastic run fixtures/checkout --format=concise
```

<!-- output: coverage-and-mutants/weak -->
```text

 Missed ────────────────────────────────────────────────────────────
 ✗ src/lib.rs:6 no test checks discount() at total == 100 ma4286e9f,ma7d1f59d
 ✗ src/lib.rs:9 no test checks discount() returning 0 here m669a0afd
 ✗ src/lib.rs:14 tests pass even when * becomes / in pay() m3590a884
 ✗ src/lib.rs:16 no test checks what receipt.push(format!("discount {off}")); does in pay() ma1e58306
 ✗ src/lib.rs:17 no test checks what pay() returns here mc509a35e

 Next: mutastic verify <id> fixtures/checkout · mutastic run --only=<id> fixtures/checkout · mutastic explain <id> fixtures/checkout

     Files  0 clean | 1 with misses (1)
   Mutants  ✓ 4 caught · ✗ 6 missed (10)
  Worst at  discount() 3 ✗ · pay() 3 ✗
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  17 (20 without coverage selection)
  Start at  12:04:30
  Duration  1.1s (setup 408ms, build 625ms, coverage 4ms, mutants 12ms, checking 23ms)
     Tests  2 on rustc 1.94.0 · 4 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  40.0%  [FAIL] Below the threshold of 80: 6 missed, 0 not covered.
```

Each mutant under `Missed` is a bug those tests let through, on code that coverage counts as
tested. With the tests that check the results, mutastic catches all of them but one, and that one
is the first of the next section.

## Where mutants go further than branches

### Boundaries

The tests that check the results take the `if` both ways, at 50 and 250. Branch coverage asks for
nothing more. But no test is at 100, so `>=` and `>` give the same answer on every test, and the
off-by-one bug passes:

```sh
mutastic run fixtures/checkout
```

<!-- output: coverage-and-mutants/strong -->
```text

 Missed ────────────────────────────────────────────────────────────

 ❯ src/lib.rs  1 missed in 1 function

 [✗ MISSED] no test checks discount() at total == 100
   ┌─ src/lib.rs:6
   │
 5 │ pub fn discount(total: u32) -> u32 {
 6 │     if total >= 100 {
   │              ^^ >= changed to >
 7 │         return total / 10;
   │
   = mutant  - if total >= 100 {
             + if total > 100 {
   = note: every test calls discount() away from the boundary
           inputs seen: 50, 250 (none is 100)
           covered by checkout::big_orders_get_a_discount, checkout::small_orders_pay_full_price (all 2 covering tests still pass)
   = help: test discount() with total = 100 and assert the result

           // in a new #[cfg(test)] mod tests in src/lib.rs
           #[test]
           fn discount_at_the_total_edge() {
               assert_eq!(super::discount(100), 10); // today 10 · with this mutant 0
           }
           check today's value is the one you intend: the test pins what the code does now

   done when  mutastic verify ma4286e9f fixtures/checkout passes
   reproduce  mutastic run --only=ma4286e9f fixtures/checkout
   or ignore  mutastic ignore ma4286e9f --reason="<why>" fixtures/checkout

 Next: mutastic verify <id> fixtures/checkout · mutastic run --only=<id> fixtures/checkout · mutastic explain <id> fixtures/checkout

     Files  0 clean | 1 with misses (1)
   Mutants  ✓ 9 caught · ✗ 1 missed (10)
  Worst at  discount() 1 ✗
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  17 (20 without coverage selection)
  Start at  12:04:32
  Duration  473ms (setup 228ms, build 174ms, coverage 4ms, mutants 18ms, checking 24ms)
     Tests  2 on rustc 1.94.0 · 4 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  90.0%  [PASS] 1 missed, above the threshold of 80.
```

The card asks for a test at 100. Here's that one, through `pay()` as the others go, and one just
below the edge, so the edge is pinned from both sides:

<!-- added to: fixtures/checkout/tests/checkout.rs -->
```rust
#[test]
fn orders_of_100_get_a_discount() {
    assert_eq!(pay(1, 100, &mut Vec::new()), 90);
}

#[test]
fn orders_just_under_100_pay_full_price() {
    assert_eq!(pay(1, 99, &mut Vec::new()), 99);
}
```

Now the run catches every mutant:

```sh
mutastic run fixtures/checkout
```

<!-- output: coverage-and-mutants/edges -->
```text

     Files  1 clean (1)
   Mutants  ✓ 10 caught (10)
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  34 (40 without coverage selection)
  Start at  12:04:33
  Duration  454ms (setup 237ms, build 168ms, coverage 4ms, mutants 12ms)
     Tests  4 on rustc 1.94.0 · 4 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  100.0%  [PASS] Every mutant was caught.
```

### Arithmetic

Every test buys one item, and 1 / 250 is 0. With `*` changed to `/`, `pay()` charges nothing, and
nothing is less than 250, so the test that checks too little still passes. Coverage counted the
line; nothing checked what it worked out.

<!-- output: coverage-and-mutants/arithmetic -->
```text
 [✗ MISSED] tests pass even when * becomes / in pay()
    ┌─ src/lib.rs:14
    │
 13 │ pub fn pay(quantity: u32, price: u32, receipt: &mut Vec<String>) -> u32 {
 14 │     let total = quantity * price;
    │                          ^ * changed to /
 15 │     let off = discount(total);
    │
    = mutant  - let total = quantity * price;
              + let total = quantity / price;
    = note: inputs seen: price = 50, 250; quantity = 1
            covered by checkout::big_orders_get_a_discount, checkout::small_orders_pay_full_price (all 2 covering tests still pass)
    = help: use inputs where quantity * price and quantity / price differ, and assert the result

    done when  mutastic verify m3590a884 fixtures/checkout passes
    reproduce  mutastic run --only=m3590a884 fixtures/checkout
```

### Return values

A test that checks only which way a value went passes for a wrong value too. `pay()` returning 0
is less than 250:

<!-- output: coverage-and-mutants/returns -->
```text
 [✗ MISSED] no test checks what pay() returns here
    ┌─ src/lib.rs:17
    │
 16 │     receipt.push(format!("discount {off}"));
 17 │     total - off
    │     ^^^^^^^^^^^ now returns 0
 18 │ }
    │
    = mutant  - total - off
              + 0
    = note: tests reach this return, but none fails when it returns 0
            inputs seen: price = 50, 250; quantity = 1
            covered by checkout::big_orders_get_a_discount, checkout::small_orders_pay_full_price (all 2 covering tests still pass)
    = help: assert the value pay() returns on this path, with inputs where it isn't 0

    done when  mutastic verify mc509a35e fixtures/checkout passes
    reproduce  mutastic run --only=mc509a35e fixtures/checkout
```

### Removed calls

The receipt is what the customer sees, and neither test looks at it. With the line that writes it
gone, both pass:

<!-- output: coverage-and-mutants/calls -->
```text
 [✗ MISSED] no test checks what receipt.push(format!("discount {off}")); does in pay()
    ┌─ src/lib.rs:16
    │
 15 │     let off = discount(total);
 16 │     receipt.push(format!("discount {off}"));
    │     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ removed
 17 │     total - off
    │
    = mutant  - receipt.push(format!("discount {off}"));
    = note: inputs seen: price = 50, 250; quantity = 1
            covered by checkout::big_orders_get_a_discount, checkout::small_orders_pay_full_price (all 2 covering tests still pass)
    = help: assert what receipt.push(format!("discount {off}")); changes, in a test of pay()

    done when  mutastic verify ma1e58306 fixtures/checkout passes
    reproduce  mutastic run --only=ma1e58306 fixtures/checkout
```

The tests that check the results catch all three: they assert what `pay()` returns and what's on
the receipt.

## When the defaults aren't enough

A run uses the `defaults` set of operators: the changes most likely to find a gap worth a test.
`--operators=stronger` adds more (see [Operators](operators.md)). Before you pay for a run,
`mutastic mutants` lists what it would make, and runs no test:

```sh
mutastic mutants fixtures/checkout --operators=stronger
```

<!-- output: coverage-and-mutants/stronger-mutants -->
```text
m669a0afd src/lib.rs:9:5 mutastic/return-value returns 1 instead in discount
ma4286e9f src/lib.rs:6:14 mutastic/boundary >= becomes > in discount
ma7d1f59d src/lib.rs:6:8 mutastic/remove-conditional total >= 100 is always true in discount
m0f966c12 src/lib.rs:6:8 mutastic/remove-conditional total >= 100 is always false in discount
m5d1ed654 src/lib.rs:6:14 mutastic/other-comparison >= becomes < in discount
m11accdd0 src/lib.rs:6:14 mutastic/comparison >= becomes == in discount
m690ba6b7 src/lib.rs:6:14 mutastic/other-comparison >= becomes <= in discount
m0f0e5084 src/lib.rs:6:14 mutastic/other-comparison >= becomes != in discount
me72201ca src/lib.rs:6:17 mutastic/constant-value 100 becomes 101 in discount
m5c8c3354 src/lib.rs:6:17 mutastic/constant-value 100 becomes 99 in discount
m6be83c47 src/lib.rs:6:17 mutastic/constant-value 100 becomes 0 in discount
m1b68d277 src/lib.rs:6:17 mutastic/constant-value 100 becomes 1 in discount
m6061aa93 src/lib.rs:7:16 mutastic/return-value returns 0 instead in discount
mf4c6bfb8 src/lib.rs:7:22 mutastic/math / becomes * in discount
macebcc29 src/lib.rs:7:22 mutastic/other-arithmetic / becomes + in discount
m5c7b95a4 src/lib.rs:7:22 mutastic/other-arithmetic / becomes - in discount
mf2ae8a59 src/lib.rs:7:22 mutastic/other-arithmetic / becomes % in discount
mdece90bd src/lib.rs:7:16 mutastic/keep-operand total / 10 becomes total in discount
m0081b732 src/lib.rs:7:24 mutastic/constant-value 10 becomes 11 in discount
mc7fcffe2 src/lib.rs:7:24 mutastic/constant-value 10 becomes 9 in discount
m9103d5c4 src/lib.rs:7:24 mutastic/constant-value 10 becomes 0 in discount
m668fb60f src/lib.rs:7:24 mutastic/constant-value 10 becomes 1 in discount
mc509a35e src/lib.rs:17:5 mutastic/return-value returns 0 instead in pay
m3590a884 src/lib.rs:14:26 mutastic/math * becomes / in pay
m29b08789 src/lib.rs:14:26 mutastic/other-arithmetic * becomes + in pay
m3bbb410a src/lib.rs:14:26 mutastic/other-arithmetic * becomes - in pay
ma7d5b2b0 src/lib.rs:14:26 mutastic/other-arithmetic * becomes % in pay
mf0c43a93 src/lib.rs:14:17 mutastic/keep-operand quantity * price becomes quantity in pay
md6d95190 src/lib.rs:14:17 mutastic/keep-operand quantity * price becomes price in pay
m784fc10e src/lib.rs:15:15 mutastic/omit-call discount(total) becomes total in pay
ma1e58306 src/lib.rs:16:5 mutastic/remove-void-call receipt.push(format!("discount {off}")); is removed in pay
mee18531c src/lib.rs:17:11 mutastic/math - becomes + in pay
m515e777e src/lib.rs:17:11 mutastic/other-arithmetic - becomes * in pay
mbf8a1505 src/lib.rs:17:11 mutastic/other-arithmetic - becomes / in pay
m1127474d src/lib.rs:17:11 mutastic/other-arithmetic - becomes % in pay
m2e132563 src/lib.rs:17:5 mutastic/keep-operand total - off becomes total in pay
m4d187d19 src/lib.rs:17:5 mutastic/keep-operand total - off becomes off in pay
37 mutants
```

With every test so far in, the defaults catch everything. `stronger` finds one more:

```sh
mutastic run fixtures/checkout --operators=stronger
```

<!-- output: coverage-and-mutants/stronger -->
```text

 Missed ────────────────────────────────────────────────────────────

 ❯ src/lib.rs  1 missed in 1 function

 [✗ MISSED] tests pass even when quantity * price becomes price in pay()
    ┌─ src/lib.rs:14
    │
 13 │ pub fn pay(quantity: u32, price: u32, receipt: &mut Vec<String>) -> u32 {
 14 │     let total = quantity * price;
    │                 ^^^^^^^^^^^^^^^^ changed to price
 15 │     let off = discount(total);
    │
    = mutant  - let total = quantity * price;
              + let total = price;
    = note: inputs seen: price = 50, 99, 100, 250; quantity = 1
            covered by checkout::big_orders_get_a_discount, checkout::orders_just_under_100_pay_full_price, checkout::orders_of_100_get_a_discount, checkout::small_orders_pay_full_price (all 4 covering tests still pass)
    = help: use inputs where quantity * price and price differ, and assert what pay() does

    done when  mutastic verify md6d95190 fixtures/checkout passes
    reproduce  mutastic run --only=md6d95190 fixtures/checkout
    or ignore  mutastic ignore md6d95190 --reason="<why>" fixtures/checkout

 Next: mutastic verify <id> fixtures/checkout · mutastic run --only=<id> fixtures/checkout · mutastic explain <id> fixtures/checkout

     Files  0 clean | 1 with misses (1)
   Mutants  ✓ 36 caught · ✗ 1 missed (37)
  Worst at  pay() 1 ✗
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  126 (148 without coverage selection)
  Start at  12:04:34
  Duration  843ms (setup 396ms, build 368ms, coverage 4ms, mutants 42ms, checking 0ms)
     Tests  4 on rustc 1.94.0 · 4 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  97.3%  [PASS] 1 missed, above the threshold of 80.
```

Every test buys one item, so `quantity * price` and `price` are the same on every test: a bug that
drops the quantity would ship. The defaults only try `*` to `/`, and the tests catch that one,
since 1 / 250 is 0. A test that buys more than one closes the gap:

<!-- added to: fixtures/checkout/tests/checkout.rs -->
```rust
#[test]
fn several_items_cost_their_price_each() {
    assert_eq!(pay(3, 20, &mut Vec::new()), 60);
}
```

```sh
mutastic verify md6d95190 fixtures/checkout
```

<!-- output: coverage-and-mutants/stronger-verify -->
```text
mutastic: md6d95190 is caught by checkout::several_items_cost_their_price_each
```

## Which to use

- **Coverage finds what no test runs.** That's worth knowing, and cheap to measure. mutastic lists
  it too, under `Not covered`.
- **A coverage gate says the tests ran the code. mutastic's score says they checked it.** A score
  can't be raised by calling code without checking it, which a coverage number can.
- **The defaults first, `stronger` when they're all caught.** It finds gaps like the one above, at
  the cost of more mutants to run.

Writing tests first doesn't close these gaps by itself: [Working test-first](test-first.md) shows a
test written first passing on code that's wrong, and mutastic finding it in the cycle.
