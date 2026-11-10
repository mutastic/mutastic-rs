# Conditions

Operators that change a comparison, a condition, or how two conditions combine. Together they ask
whether your tests look at both sides of every decision, and at its edge.

| Operator | Changes | Set |
|---|---|---|
| [`mutastic/boundary`](#boundary) | `<` and `<=`, `>` and `>=`; `lo..hi` and `lo..=hi` | defaults |
| [`mutastic/remove-conditional`](#remove-conditional) | a condition to always `true`, and to always `false` | defaults |
| [`mutastic/logical-connector`](#logical-connector) | `&&` and `\|\|` | defaults |
| [`mutastic/comparison`](#comparison) | a comparison to the one that differs on one side: `<` to `!=` | stronger |
| [`mutastic/other-comparison`](#other-comparison) | a comparison to the others; a condition to its opposite | stronger |

Each card below comes from `mutastic run fixtures/operators` with these operators, on
`src/conditions.rs`.

## Boundary

`mutastic/boundary`, in the defaults. `<` becomes `<=`, `>=` becomes `>`, and the other way round.
In a `for` loop, a slice (`&s[lo..hi]`) or a `contains`, `lo..hi` becomes `lo..=hi`, and back. It
finds the off-by-one at the edge of a range, which tests that only try values far from the edge
never notice.

**The bug it stands for:** the off-by-one. `>=` and `>` read almost the same, and reviews rarely
stop on them, so the wrong one gets written and gets through. It costs wherever a rule has a limit:
free delivery from 50, a discount from 10 items, a lock after 3 failures, the last day a refund is
allowed. Someone who spends exactly 50 because the site said "free delivery from 50" is the case the
rule exists for.

<!-- from: fixtures/operators/src/conditions.rs -->
```rust
/// Free delivery from 50 up.
pub fn free_delivery(total: u32) -> bool {
    total >= 50
}
```

The test tries an order on each side, and neither at the edge:

<!-- from: fixtures/operators/src/conditions.rs -->
```rust
#[test]
fn small_orders_pay_for_delivery() {
    assert!(!free_delivery(20));
    assert!(free_delivery(80));
}
```

<!-- output: operators/conditions/boundary -->
```text
 [✗ MISSED] no test checks free_delivery() at total == 50
   ┌─ src/conditions.rs:5
   │
 4 │ pub fn free_delivery(total: u32) -> bool {
 5 │     total >= 50
   │           ^^ >= changed to >
 6 │ }
   │
   = mutant  - total >= 50
             + total > 50
   = note: every test calls free_delivery() away from the boundary
           inputs seen: 20, 80 (none is 50)
           covered by conditions::tests::small_orders_pay_for_delivery (its 1 covering test still passes)
   = help: test free_delivery() with total = 50 and assert the result

           // in mod conditions::tests
           #[test]
           fn free_delivery_with_total_50() {
               assert!(free_delivery(50)); // today true · with this mutant false
           }
           check today's value is the one you intend: the test pins what the code does now

   done when  mutastic verify m27535e4c fixtures/operators passes
   reproduce  mutastic run --only=m27535e4c fixtures/operators
```

**The test that catches it:** one at the edge. Totals of 20 and 80 pass either way; only 50 tells
`>=` from `>`. A boundary miss is also a question for whoever owns the rule: should 50 get free
delivery? The test records the answer.

<!-- added to: fixtures/operators/src/conditions.rs -->
```rust
#[test]
fn delivery_is_free_from_exactly_50() {
    assert!(free_delivery(50));
}
```

**Edition notes:** the Java edition has this operator too, `mutastic/boundary`, for `<`, `<=`, `>`
and `>=`; it has no ranges to move.

## Remove conditional

`mutastic/remove-conditional`, in the defaults. A condition stops deciding: the code it guards
always runs, or never does. Each condition gets two mutants, one each way. It reaches `if`, `while`,
a match arm's guard, and `let Some(x) = e` in an `if let`, `while let` or `let`-`else`, where the
pattern is made never to match. It goes unnoticed when no test needs the condition to be true one
time and false another.

**The bug it stands for:** a check that doesn't matter as much as its author thought, or that's the
wrong check entirely. It costs in guards: a permission check, a validation, an "only if it's
changed" before a write. If the code behaves the same with the guard always passing, no test has
ever been refused by it; if it behaves the same with the guard never passing, no test has ever
needed it to let something through.

<!-- from: fixtures/operators/src/conditions.rs -->
```rust
pub fn greeting(member: bool) -> &'static str {
    if member {
        "welcome back"
    } else {
        "welcome"
    }
}
```

<!-- from: fixtures/operators/src/conditions.rs -->
```rust
#[test]
fn members_are_welcomed_back() {
    assert_eq!(greeting(true), "welcome back");
}
```

`if false` makes the test fail, so that mutant is caught; `if true` gets past it:

<!-- output: operators/conditions/remove-conditional -->
```text
 [✗ MISSED] no test needs member to be false in greeting()
    ┌─ src/conditions.rs:14
    │
 13 │ pub fn greeting(member: bool) -> &'static str {
 14 │     if member {
    │        ^^^^^^ now always true
 15 │         "welcome back"
    │
    = mutant  - if member {
              + if true {
    = note: covered by conditions::tests::members_are_welcomed_back (its 1 covering test still passes)
    = help: add a test where member is false, and assert what greeting() does then

    done when  mutastic verify m62a2d487 fixtures/operators passes
    reproduce  mutastic run --only=m62a2d487 fixtures/operators
```

**The test that catches it:** one for each answer the condition can give. The card names the one no
test has tried: here, someone who isn't a member.

<!-- added to: fixtures/operators/src/conditions.rs -->
```rust
#[test]
fn others_are_welcomed() {
    assert_eq!(greeting(false), "welcome");
}
```

**Edition notes:** the Java edition has this operator too, `mutastic/remove-conditional`, for every
condition the bytecode jumps on, `&&` and `||`'s sides among them.

## Logical connector

`mutastic/logical-connector`, in the defaults. `&&` becomes `||`, and `||` becomes `&&`. The two
give different answers exactly when one side holds and the other doesn't, so it goes unnoticed when
no test has one side true and the other false.

**The bug it stands for:** `&&` where `||` was meant, or the other way round: a rule that needs both
conditions written as either, a discount for members *or* big orders written as *and*. It costs in
eligibility and permission rules, and validation that should reject on any of several problems.

<!-- from: fixtures/operators/src/conditions.rs -->
```rust
pub fn can_ship(paid: bool, in_stock: bool) -> bool {
    paid && in_stock
}
```

<!-- from: fixtures/operators/src/conditions.rs -->
```rust
#[test]
fn ships_when_paid_and_in_stock() {
    assert!(can_ship(true, true));
    assert!(!can_ship(false, false));
}
```

<!-- output: operators/conditions/logical-connector -->
```text
 [✗ MISSED] no test checks can_ship() with one of paid and in_stock true and the other false
    ┌─ src/conditions.rs:22
    │
 21 │ pub fn can_ship(paid: bool, in_stock: bool) -> bool {
 22 │     paid && in_stock
    │          ^^ && changed to ||
 23 │ }
    │
    = mutant  - paid && in_stock
              + paid || in_stock
    = note: covered by conditions::tests::ships_when_paid_and_in_stock (its 1 covering test still passes)
    = help: add a test where one of paid and in_stock is true and the other false, and assert what can_ship() does then

    done when  mutastic verify m069e3be4 fixtures/operators passes
    reproduce  mutastic run --only=m069e3be4 fixtures/operators
```

**The test that catches it:** one where the two sides disagree, each way round: paid but out of
stock, and in stock but unpaid.

<!-- added to: fixtures/operators/src/conditions.rs -->
```rust
#[test]
fn neither_alone_is_enough() {
    assert!(!can_ship(true, false));
    assert!(!can_ship(false, true));
}
```

**Edition notes:** the Java edition has this operator too, `mutastic/logical-connector`. It reads
`&&` from the bytecode's jumps, so it leaves alone a condition of three or more tests, which this
edition, reading the source, doesn't.

## Comparison

`mutastic/comparison`, in the `stronger` set. A comparison becomes the one that gives the same
answer at its edge and on one side of it, and the other answer on the other side:

| Written | Becomes | Differs where |
|---|---|---|
| `a < b` | `a != b` | `a > b` |
| `a <= b` | `a == b` | `a < b` |
| `a > b` | `a != b` | `a < b` |
| `a >= b` | `a == b` | `a > b` |
| `a == b` | `a <= b`, and `a >= b` | `a < b`, and `a > b` |
| `a != b` | `a < b`, and `a > b` | `a > b`, and `a < b` |

**The bug it stands for:** a comparison that's right at the edge and wrong past it: `==` where `>=`
was meant, so a rule holds for exactly 100 and stops at 101. It costs in the same rules as a
boundary, on the side tests skip: tests often try a value below a limit and one exactly at it, and
never one well past it.

`==` and `!=` get these only beside a number, a character, a length or a cast, where an order is
plain from the code. With boundary and remove conditional, they're the smallest set of changes
that's enough for a comparison: tests that catch the edge moved, the side changed, and the
condition always true or false catch every other way it could be written wrong.

<!-- from: fixtures/operators/src/conditions.rs -->
```rust
/// Time to restock once stock falls below the low mark.
pub fn needs_restock(stock: u32, low: u32) -> bool {
    stock < low
}
```

The test tries stock below the mark and at it, so the boundary's mutant is caught, but never above
it:

<!-- from: fixtures/operators/src/conditions.rs -->
```rust
#[test]
fn restock_when_low() {
    assert!(needs_restock(2, 5));
    assert!(!needs_restock(5, 5));
}
```

<!-- output: operators/conditions/comparison -->
```text
 [✗ MISSED] no test checks needs_restock() with stock > low
    ┌─ src/conditions.rs:10
    │
  9 │ pub fn needs_restock(stock: u32, low: u32) -> bool {
 10 │     stock < low
    │           ^ < changed to !=
 11 │ }
    │
    = mutant  - stock < low
              + stock != low
    = note: inputs seen: low = 5; stock = 2, 5
            covered by conditions::tests::restock_when_low (its 1 covering test still passes)
    = help: add a test where stock > low, and assert what needs_restock() does then

            // in mod conditions::tests
            #[test]
            fn needs_restock_with_stock_5_and_low_2() {
                assert!(!needs_restock(5, 2)); // today false · with this mutant true
            }
            check today's value is the one you intend: the test pins what the code does now

    done when  mutastic verify m0afcf739 fixtures/operators passes
    reproduce  mutastic run --only=m0afcf739 fixtures/operators
```

**The test that catches it:** one on the side where the two disagree, which the card names: here,
stock above the low mark.

<!-- added to: fixtures/operators/src/conditions.rs -->
```rust
#[test]
fn plenty_in_stock_needs_no_restock() {
    assert!(!needs_restock(8, 5));
}
```

It's in `stronger` because on real code much of what it misses is noise: a loop counter that counts
up to its end, where `i < n` and `i != n` stop at the same place.

**Edition notes:** the Java edition has this operator too, `mutastic/comparison`, with the same
changes, and makes them only between numbers, as only numbers have an order in its bytecode.

## Other comparison

`mutastic/other-comparison`, in the `stronger` set. A comparison becomes each of the others the two
operators above don't make (`<` becomes `>`, `>=` or `==`), `if c` becomes `if !c`, and
`matches!(..)` becomes `!matches!(..)`. It goes unnoticed when no test checks one side of a
decision at all. A test that catches boundary, comparison and remove conditional catches these too,
so they'd only add to the score in the defaults.

**The bug it stands for:** a condition the wrong way round: a missing `!`, `==` for `!=`, a check
that guards the wrong case. It costs where code decides which way to go: whether an order is paid
before it ships, whether a user may see a page. Getting it backwards usually means doing the wrong
thing in exactly the case that matters, and a permission check whose opposite no test notices is one
nobody has proved refuses anyone.

<!-- from: fixtures/operators/src/conditions.rs -->
```rust
/// Saturday is 6 and Sunday 7.
pub fn is_weekend(day: u8) -> bool {
    matches!(day, 6 | 7)
}
```

This test runs the function and prints what it says, but checks nothing:

<!-- from: fixtures/operators/src/conditions.rs -->
```rust
#[test]
fn weekend_days_are_known() {
    let saturday = is_weekend(6);
    println!("saturday: {saturday}");
}
```

<!-- output: operators/conditions/other-comparison -->
```text
 [✗ MISSED] tests pass even when matches!(day, 6 | 7) becomes !matches!(day, 6 | 7) in is_weekend()
    ┌─ src/conditions.rs:27
    │
 26 │ pub fn is_weekend(day: u8) -> bool {
 27 │     matches!(day, 6 | 7)
    │     ^^^^^^^^^^^^^^^^^^^^ changed to !matches!(day, 6 | 7)
 28 │ }
    │
    = mutant  - matches!(day, 6 | 7)
              + !matches!(day, 6 | 7)
    = note: inputs seen: day = 6
            covered by conditions::tests::weekend_days_are_known (its 1 covering test still passes)
    = help: use inputs where matches!(day, 6 | 7) and !matches!(day, 6 | 7) differ, and assert what is_weekend() does

            // in mod conditions::tests
            #[test]
            fn is_weekend_with_day_6() {
                assert!(is_weekend(6)); // today true · with this mutant false
            }
            check today's value is the one you intend: the test pins what the code does now

    done when  mutastic verify m4f55c224 fixtures/operators passes
    reproduce  mutastic run --only=m4f55c224 fixtures/operators
```

**The test that catches it:** one that checks the answer on each side: a weekend day, and a weekday.

<!-- added to: fixtures/operators/src/conditions.rs -->
```rust
#[test]
fn saturday_is_the_weekend_and_wednesday_isnt() {
    assert!(is_weekend(6));
    assert!(!is_weekend(3));
}
```

**Edition notes:** the Java edition has this operator too, `mutastic/other-comparison`, and turns a
`Predicate`'s `and` and `or`, `negate()` and `Predicate.not` round with `predicate/*` operators of
its own; here, `&&` and `!` inside a closure are logical connector's and this one's.
