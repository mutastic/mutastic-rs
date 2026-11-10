# Control flow

Operators that change which arm of a `match` runs, or how a loop goes on. They ask whether your
tests reach every arm with a value that matters, and every way out of a loop.

| Operator | Changes | Set |
|---|---|---|
| [`mutastic/remove-switch-arm`](#remove-switch-arm) | a `match` arm stops matching, so its values go to a later arm | stronger |
| [`mutastic/swap-break`](#swap-break) | `break` and `continue` | extended |

Each card below comes from `mutastic run fixtures/operators` with these operators, on
`src/control.rs`.

## Remove switch arm

`mutastic/remove-switch-arm`, in the `stronger` set (`extended` in the Java edition). An arm stops
matching, so its values go on to the next arm that matches. mutastic does it with a guard the
switch decides, and only for an arm that comes before an unguarded catch-all, so there's always an
arm for the values to go to, and not for an arm that binds a variable. A test reaches the mutant
only when it passes a value that arm matches.

**The bug it stands for:** a case that's missing or never worked, so its values get another arm's
behaviour: the wrong rate, the wrong message. Tests tend to try one or two arms, and the others can
be wrong for a long time, as the catch-all gives an answer that looks fine.

<!-- from: fixtures/operators/src/control.rs -->
```rust
pub fn delivery_days(zone: u8) -> u32 {
    match zone {
        1 => 1,
        2 => 3,
        _ => 7,
    }
}
```

One test checks zones 1 and 9 exactly. Another reaches zone 2, but only checks that it's within a
week, as the catch-all's 7 is too:

<!-- from: fixtures/operators/src/control.rs -->
```rust
#[test]
fn every_zone_within_a_week() {
    for zone in 1..=3 {
        assert!(delivery_days(zone) <= 7);
    }
}
```

<!-- output: operators/control/remove-switch-arm -->
```text
 [✗ MISSED] tests pass even when delivery_days() has no arm for 2
   ┌─ src/control.rs:6
   │
 5 │         1 => 1,
 6 │         2 => 3,
   │         ^ arm removed
 7 │         _ => 7,
   │
   = mutant  - 2 => 3,
   = note: inputs seen: zone = 1, 2, 3
           covered by control::tests::every_zone_within_a_week (its 1 covering test still passes)
   = help: add a test where delivery_days() takes the 2 arm, and assert what it does there

           // in mod control::tests
           #[test]
           fn delivery_days_with_zone_2() {
               assert_eq!(delivery_days(2), 3); // today 3 · with this mutant 7
           }
           check today's value is the one you intend: the test pins what the code does now

   done when  mutastic verify mc9468457 fixtures/operators passes
   reproduce  mutastic run --only=mc9468457 fixtures/operators
```

**The test that catches it:** one with a value that reaches the arm, asserting what it does.

<!-- added to: fixtures/operators/src/control.rs -->
```rust
#[test]
fn zone_two_takes_three_days() {
    assert_eq!(delivery_days(2), 3);
}
```

**Edition notes:** the Java edition has this operator too, `mutastic/remove-switch-arm`, in
`extended`, for a `switch`, whose arm's values go to the default.

## Swap break

`mutastic/swap-break`, in the `extended` set, and only in Rust. `break` becomes `continue`, and
`continue` becomes `break`, where it has no label and no value. It goes unnoticed when the loop
ends the same way on every input the tests try.

**The bug it stands for:** a loop that stops where it should go on, or goes on where it should stop:
the first match wanted and the last one found. Input with one match can't tell them apart.

<!-- from: fixtures/operators/src/control.rs -->
```rust
pub fn first_negative(values: &[i32]) -> Option<usize> {
    let mut found = None;
    for (index, value) in values.iter().enumerate() {
        if *value < 0 {
            found = Some(index);
            break;
        }
    }
    found
}
```

<!-- from: fixtures/operators/src/control.rs -->
```rust
#[test]
fn finds_a_negative() {
    assert_eq!(first_negative(&[4, -1, 2]), Some(1));
}
```

With `continue`, the loop finds the last negative, not the first, and with one negative there's no
difference:

<!-- output: operators/control/swap-break -->
```text
 [✗ MISSED] tests pass even when break becomes continue in first_negative()
    ┌─ src/control.rs:16
    │
 15 │             found = Some(index);
 16 │             break;
    │             ^^^^^ break changed to continue
 17 │         }
    │
    = mutant  - break;
              + continue;
    = note: covered by control::tests::finds_a_negative (its 1 covering test still passes)
    = help: use inputs where break and continue give different results, and assert what first_negative() does

    done when  mutastic verify mde47e51e fixtures/operators passes
    reproduce  mutastic run --only=mde47e51e fixtures/operators
```

**The test that catches it:** one with a second match after the first.

<!-- added to: fixtures/operators/src/control.rs -->
```rust
#[test]
fn the_first_negative_of_two() {
    assert_eq!(first_negative(&[4, -1, -2]), Some(1));
}
```

**Edition notes:** Rust only.
