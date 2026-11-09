# Arithmetic

Operators that change a calculation: an operator, a number, a sign, or the bits. They ask whether
your tests use inputs where the answer would come out differently.

| Operator | Changes | Set |
|---|---|---|
| [`mutastic/math`](#math) | `+` and `-`, `*` and `/`, and their `checked_`, `wrapping_`, `saturating_` and `overflowing_` methods | defaults |
| [`mutastic/arithmetic-replacement`](#arithmetic-replacement) | `+` to `*` | stronger |
| [`mutastic/other-arithmetic`](#other-arithmetic) | an arithmetic operator to each of the others | stronger |
| [`mutastic/keep-operand`](#keep-operand) | `a + b` to `a`, and to `b` | stronger |
| [`mutastic/remove-negation`](#remove-negation) | `-x` to `x` | stronger |
| [`mutastic/constant-value`](#constant-value) | a number to the ones next to it, `0`, `1` and `-1` | stronger |
| [`mutastic/bitwise`](#bitwise) | `&`, `\|` and `^`; `<<` and `>>` | stronger |
| [`library/unchecked-arithmetic`](#unchecked-arithmetic) | `checked_add` to a `wrapping_add` that can't fail | stronger |

Each card below comes from `mutastic run fixtures/operators` with these operators, on
`src/arithmetic.rs`. A line with several missed mutants gets one card, written about the first, with
the others under `also`.

Overflow checks are always on in mutastic's build, so an arithmetic mutant that overflows panics,
and the test that runs it fails: see [Profiles](../concepts/profiles.md).

## Math

`mutastic/math`, in the defaults. `+` becomes `-`, `*` becomes `/`, and the other way round, in
compound assignments too (`+=` and `-=`). So do the methods: `checked_add` and `checked_sub`,
`checked_mul` and `checked_div`, and likewise for `wrapping_`, `saturating_` and `overflowing_`.

<!-- from: fixtures/operators/src/arithmetic.rs -->
```rust
pub fn line_total(price: u32, quantity: u32) -> u32 {
    price * quantity
}
```

The test buys one, and `30 * 1` is `30 / 1`:

<!-- from: fixtures/operators/src/arithmetic.rs -->
```rust
#[test]
fn one_of_each() {
    assert_eq!(line_total(30, 1), 30);
}
```

<!-- output: operators/arithmetic/math -->
```text
 [✗ MISSED] tests pass even when * becomes / in line_total() · m7c2deaec
   ┌─ src/arithmetic.rs:4
   │
 3 │ pub fn line_total(price: u32, quantity: u32) -> u32 {
 4 │     price * quantity
   │           ^ changed to /
 5 │ }
   │
   = mutant  - price * quantity
             + price / quantity
   = also    ✗ tests pass even when price * quantity becomes price in line_total() · m9fbcbdb2
             + price
   = note: inputs seen: price = 30; quantity = 1
           covered by arithmetic::tests::one_of_each (its 1 covering test still passes)
   = help: use inputs where price * quantity and price / quantity differ, and assert the result

           // in mod arithmetic::tests
           #[test]
           fn line_total_with_price_30_and_quantity_30() {
               assert_eq!(line_total(30, 30), 900); // today 900 · with m7c2deaec 1
           }
           check today's value is the one you intend: the test pins what the code does now

   done when  mutastic verify m7c2deaec,m9fbcbdb2 fixtures/operators passes
   reproduce  mutastic run --only=m7c2deaec,m9fbcbdb2 fixtures/operators
```

## Arithmetic replacement

`mutastic/arithmetic-replacement`, in the `stronger` set. `a + b` becomes `a * b`, and `+=` becomes
`*=`, but not `+= 1`, which would only stop a counter.

<!-- from: fixtures/operators/src/arithmetic.rs -->
```rust
pub fn with_fee(price: u32, fee: u32) -> u32 {
    price + fee
}
```

<!-- from: fixtures/operators/src/arithmetic.rs -->
```rust
#[test]
fn a_fee_is_added() {
    assert_eq!(with_fee(2, 2), 4);
}
```

<!-- output: operators/arithmetic/arithmetic-replacement -->
```text
 [✗ MISSED] tests pass even when + becomes * in with_fee()
   ┌─ src/arithmetic.rs:8
   │
 7 │ pub fn with_fee(price: u32, fee: u32) -> u32 {
 8 │     price + fee
   │           ^ changed to *
 9 │ }
   │
   = mutant  - price + fee
             + price * fee
   = note: inputs seen: fee = 2; price = 2
           covered by arithmetic::tests::a_fee_is_added (its 1 covering test still passes)
   = help: use inputs where price + fee and price * fee differ, and assert the result

           // in mod arithmetic::tests
           #[test]
           fn with_fee_with_price_2_and_fee_0() {
               assert_eq!(with_fee(2, 0), 2); // today 2 · with this mutant 0
           }
           check today's value is the one you intend: the test pins what the code does now

   done when  mutastic verify m2159bc78 fixtures/operators passes
   reproduce  mutastic run --only=m2159bc78 fixtures/operators
```

## Other arithmetic

`mutastic/other-arithmetic`, in the `stronger` set. Each of `+ - * / %` becomes each of the others
that math and arithmetic replacement don't make: `+` to `/` and `%`; `-` to `*`, `/` and `%`; `*`
and `/` to `+`, `-` and `%`; `%` to the four others. Compound assignments too.

<!-- from: fixtures/operators/src/arithmetic.rs -->
```rust
pub fn per_person(bill: u32, people: u32) -> u32 {
    bill / people
}
```

<!-- from: fixtures/operators/src/arithmetic.rs -->
```rust
#[test]
fn a_bill_is_shared() {
    assert_eq!(per_person(4, 2), 2);
}
```

`4 / 2` and `4 - 2` are both 2:

<!-- output: operators/arithmetic/other-arithmetic -->
```text
 [✗ MISSED] tests pass even when / becomes - in per_person() · mea866977
    ┌─ src/arithmetic.rs:12
    │
 11 │ pub fn per_person(bill: u32, people: u32) -> u32 {
 12 │     bill / people
    │          ^ changed to -
 13 │ }
    │
    = mutant  - bill / people
              + bill - people
    = also    ✗ tests pass even when bill / people becomes people in per_person() · m82fdb48b
              + people
    = note: inputs seen: bill = 4; people = 2
            covered by arithmetic::tests::a_bill_is_shared (its 1 covering test still passes)
    = help: use inputs where bill / people and bill - people differ, and assert the result

            // in mod arithmetic::tests
            #[test]
            fn per_person_with_bill_4_and_people_4() {
                assert_eq!(per_person(4, 4), 1); // today 1 · with mea866977 0
            }
            check today's value is the one you intend: the test pins what the code does now

    done when  mutastic verify mea866977,m82fdb48b fixtures/operators passes
    reproduce  mutastic run --only=mea866977,m82fdb48b fixtures/operators
```

## Keep operand

`mutastic/keep-operand`, in the `stronger` set. A calculation becomes one of its values: `price +
fee` becomes `price`, and `fee`. The other side is still worked out when working it out could do
something, such as call a function. A constant beside a value is never what's kept.

<!-- from: fixtures/operators/src/arithmetic.rs -->
```rust
pub fn after_refund(paid: u32, refund: u32) -> u32 {
    paid - refund
}
```

<!-- from: fixtures/operators/src/arithmetic.rs -->
```rust
#[test]
fn nothing_refunded() {
    assert_eq!(after_refund(10, 0), 10);
}
```

A refund of 0 can't tell `paid - refund` from `paid`, nor from `paid + refund`, so the line's card
is math's, with keep operand's under it:

<!-- output: operators/arithmetic/keep-operand -->
```text
 [✗ MISSED] tests pass even when - becomes + in after_refund() · m4e284c1b
    ┌─ src/arithmetic.rs:16
    │
 15 │ pub fn after_refund(paid: u32, refund: u32) -> u32 {
 16 │     paid - refund
    │          ^ changed to +
 17 │ }
    │
    = mutant  - paid - refund
              + paid + refund
    = also    ✗ tests pass even when paid - refund becomes paid in after_refund() · mb46190a0
              + paid
    = note: inputs seen: paid = 10; refund = 0
            covered by arithmetic::tests::nothing_refunded (its 1 covering test still passes)
    = help: use inputs where paid - refund and paid + refund differ, and assert the result

            // in mod arithmetic::tests
            #[test]
            fn after_refund_with_paid_10_and_refund_10() {
                assert_eq!(after_refund(10, 10), 0); // today 0 · with m4e284c1b 20
            }
            check today's value is the one you intend: the test pins what the code does now

    done when  mutastic verify m4e284c1b,mb46190a0 fixtures/operators passes
    reproduce  mutastic run --only=m4e284c1b,mb46190a0 fixtures/operators
```

## Remove negation

`mutastic/remove-negation`, in the `stronger` set. `-x` becomes `x`.

<!-- from: fixtures/operators/src/arithmetic.rs -->
```rust
pub fn reversed(balance: i32) -> i32 {
    -balance
}
```

<!-- from: fixtures/operators/src/arithmetic.rs -->
```rust
#[test]
fn an_empty_balance_reversed() {
    assert_eq!(reversed(0), 0);
}
```

<!-- output: operators/arithmetic/remove-negation -->
```text
 [✗ MISSED] tests pass even when -balance becomes balance in reversed()
    ┌─ src/arithmetic.rs:20
    │
 19 │ pub fn reversed(balance: i32) -> i32 {
 20 │     -balance
    │     ^ changed to balance
 21 │ }
    │
    = mutant  - -balance
              + balance
    = note: inputs seen: balance = 0
            covered by arithmetic::tests::an_empty_balance_reversed (its 1 covering test still passes)
    = help: use inputs where -balance and balance differ, and assert what reversed() does

            // in mod arithmetic::tests
            #[test]
            fn reversed_with_balance_1() {
                assert_eq!(reversed(1), -1); // today -1 · with this mutant 1
            }
            check today's value is the one you intend: the test pins what the code does now

    done when  mutastic verify me0933ca7 fixtures/operators passes
    reproduce  mutastic run --only=me0933ca7 fixtures/operators
```

## Constant value

`mutastic/constant-value`, in the `stronger` set. A whole number becomes the numbers next to it,
`0` and `1`, and `-1` where the type is plainly signed. Never a number outside its type's range, and
never one that only sizes something, sets a wait, or starts a counter at `0` or `1`.

<!-- from: fixtures/operators/src/arithmetic.rs -->
```rust
/// 2 to send anything, and 3 for each kilo.
pub fn postage(kilos: u32) -> u32 {
    kilos * 3 + 2
}
```

<!-- from: fixtures/operators/src/arithmetic.rs -->
```rust
#[test]
fn a_letter_costs_the_minimum() {
    assert_eq!(postage(0), 2);
}
```

With `kilos` at 0, nothing about the price per kilo shows, so every change to `kilos * 3` gets past
the test: the operator's, the constant's, and keep operand's, on one card:

<!-- output: operators/arithmetic/constant-value -->
```text
 [✗ MISSED] tests pass even when * becomes / in postage() · m0f04c581
    ┌─ src/arithmetic.rs:25
    │
 24 │ pub fn postage(kilos: u32) -> u32 {
 25 │     kilos * 3 + 2
    │           ^ changed to /
 26 │ }
    │
    = mutant  - kilos * 3 + 2
              + kilos / 3 + 2
    = also    ✗ tests pass even when * becomes % in postage() · mc5b1b361
              + kilos % 3 + 2
    = also    ✗ tests pass even when kilos * 3 becomes kilos in postage() · mebd4cb5e
              + kilos + 2
    = also    ✗ tests pass even when 3 becomes 4 in postage() · mbed3c12d
              + kilos * 4 + 2
    = also    ✗ tests pass even when 3 becomes 2 in postage() · ma0fa708a
              + kilos * 2 + 2
    = also    ✗ tests pass even when 3 becomes 0 in postage() · m9d8f7916
              + kilos * 0 + 2
    = also    ✗ tests pass even when 3 becomes 1 in postage() · mb750003e
              + kilos * 1 + 2
    = note: inputs seen: kilos = 0
            covered by arithmetic::tests::a_letter_costs_the_minimum (its 1 covering test still passes)
    = help: use inputs where kilos * 3 and kilos / 3 differ, and assert the result

            // in mod arithmetic::tests
            #[test]
            fn postage_with_kilos_3() {
                assert_eq!(postage(3), 11); // today 11 · with m0f04c581 3
            }
            check today's value is the one you intend: the test pins what the code does now

    done when  mutastic verify m0f04c581,mc5b1b361,mebd4cb5e,mbed3c12d,ma0fa708a,m9d8f7916,mb750003e fixtures/operators passes
    reproduce  mutastic run --only=m0f04c581,mc5b1b361,mebd4cb5e,mbed3c12d,ma0fa708a,m9d8f7916,mb750003e fixtures/operators
```

## Bitwise

`mutastic/bitwise`, in the `stronger` set, and only in Rust. `&` becomes `|` and `^`, `|` becomes
`&` and `^`, `^` becomes `&` and `|`, and `<<` and `>>` swap. Compound assignments too, except that
`&=` and `|=` never become `^=`.

<!-- from: fixtures/operators/src/arithmetic.rs -->
```rust
pub fn has_flag(flags: u8, flag: u8) -> bool {
    flags & flag != 0
}
```

<!-- from: fixtures/operators/src/arithmetic.rs -->
```rust
#[test]
fn a_flag_that_is_set() {
    assert!(has_flag(3, 1));
}
```

<!-- output: operators/arithmetic/bitwise -->
```text
 [✗ MISSED] tests pass even when & becomes | in has_flag() · ma32c7819
    ┌─ src/arithmetic.rs:29
    │
 28 │ pub fn has_flag(flags: u8, flag: u8) -> bool {
 29 │     flags & flag != 0
    │           ^ changed to |
 30 │ }
    │
    = mutant  - flags & flag != 0
              + flags | flag != 0
    = also    ✗ tests pass even when & becomes ^ in has_flag() · maace5f2b
              + flags ^ flag != 0
    = note: inputs seen: flag = 1; flags = 3
            covered by arithmetic::tests::a_flag_that_is_set (its 1 covering test still passes)
    = help: use inputs where & and | give different results, and assert what has_flag() does

            // in mod arithmetic::tests
            #[test]
            fn has_flag_with_flags_0_and_flag_1() {
                assert!(!has_flag(0, 1)); // today false · with ma32c7819 true
            }
            check today's value is the one you intend: the test pins what the code does now

    done when  mutastic verify ma32c7819,maace5f2b fixtures/operators passes
    reproduce  mutastic run --only=ma32c7819,maace5f2b fixtures/operators
```

## Unchecked arithmetic

`library/unchecked-arithmetic`, in the `stronger` set. A checked operation becomes one that can't
fail: `a.checked_add(b)` becomes `Some(a.wrapping_add(b))`, and `saturating_sub` becomes
`wrapping_sub`. It goes unnoticed when no test reaches the overflow the check is there for.

<!-- from: fixtures/operators/src/arithmetic.rs -->
```rust
/// `None` when the points would overflow.
pub fn add_points(points: u32, more: u32) -> Option<u32> {
    points.checked_add(more)
}
```

<!-- from: fixtures/operators/src/arithmetic.rs -->
```rust
#[test]
fn points_add_up() {
    assert_eq!(add_points(1, 2), Some(3));
}
```

<!-- output: operators/arithmetic/unchecked-arithmetic -->
```text
 [✗ MISSED] tests pass even when points.checked_add(more) becomes Some(points.wrapping_add(more)) in add_points()
    ┌─ src/arithmetic.rs:34
    │
 33 │ pub fn add_points(points: u32, more: u32) -> Option<u32> {
 34 │     points.checked_add(more)
    │            ^^^^^^^^^^^ changed to Some(points.wrapping_add(more))
 35 │ }
    │
    = mutant  - points.checked_add(more)
              + Some(points.wrapping_add(more))
    = note: inputs seen: more = 2; points = 1
            covered by arithmetic::tests::points_add_up (its 1 covering test still passes)
    = help: use inputs where points.checked_add(more) and Some(points.wrapping_add(more)) differ, and assert what add_points() does

    done when  mutastic verify mcc88a618 fixtures/operators passes
    reproduce  mutastic run --only=mcc88a618 fixtures/operators
```
