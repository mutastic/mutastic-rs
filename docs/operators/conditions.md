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
   │           ^^ changed to >
 6 │ }
   │
   = mutant  - total >= 50
             + total > 50
   = note: every test calls free_delivery() away from the boundary
           inputs seen: 20, 80 (none is 50)
           covered by conditions::tests::small_orders_pay_for_delivery (its 1 covering test still passes)
   = help: test free_delivery() with total = 50 and assert the result

   done when  mutastic verify m27535e4c fixtures/operators passes
   reproduce  mutastic run --only=m27535e4c fixtures/operators
```

## Remove conditional

`mutastic/remove-conditional`, in the defaults. A condition stops deciding: the code it guards
always runs, or never does. Each condition gets two mutants, one each way. It reaches `if`, `while`,
a match arm's guard, and `let Some(x) = e` in an `if let`, `while let` or `let`-`else`, where the
pattern is made never to match. It goes unnoticed when no test needs the condition to be true one
time and false another.

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

## Logical connector

`mutastic/logical-connector`, in the defaults. `&&` becomes `||`, and `||` becomes `&&`. The two
give different answers exactly when one side holds and the other doesn't, so it goes unnoticed when
no test has one side true and the other false.

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
    │          ^^ changed to ||
 23 │ }
    │
    = mutant  - paid && in_stock
              + paid || in_stock
    = note: covered by conditions::tests::ships_when_paid_and_in_stock (its 1 covering test still passes)
    = help: add a test where one of paid and in_stock is true and the other false, and assert what can_ship() does then

    done when  mutastic verify m069e3be4 fixtures/operators passes
    reproduce  mutastic run --only=m069e3be4 fixtures/operators
```

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
    │           ^ changed to !=
 11 │ }
    │
    = mutant  - stock < low
              + stock != low
    = note: inputs seen: low = 5; stock = 2, 5
            covered by conditions::tests::restock_when_low (its 1 covering test still passes)
    = help: add a test where stock > low, and assert what needs_restock() does then

    done when  mutastic verify m0afcf739 fixtures/operators passes
    reproduce  mutastic run --only=m0afcf739 fixtures/operators
```

It's in `stronger` because on real code much of what it misses is noise: a loop counter that counts
up to its end, where `i < n` and `i != n` stop at the same place.

## Other comparison

`mutastic/other-comparison`, in the `stronger` set. A comparison becomes each of the others the two
operators above don't make (`<` becomes `>`, `>=` or `==`), `if c` becomes `if !c`, and
`matches!(..)` becomes `!matches!(..)`. It goes unnoticed when no test checks one side of a
decision at all. A test that catches boundary, comparison and remove conditional catches these too,
so they'd only add to the score in the defaults.

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

    done when  mutastic verify m4f55c224 fixtures/operators passes
    reproduce  mutastic run --only=m4f55c224 fixtures/operators
```
