# Iterators

Operators that change an iterator chain: a stage left out, or one consumer swapped for its
opposite. They ask whether your tests would notice the chain doing less, or answering the other
question.

| Operator | Changes | Set |
|---|---|---|
| [`stream/swap-match`](#swap-match) | `any` and `all` | defaults |
| [`stream/remove-stage`](#remove-stage) | `.filter(p)`, `.skip(n)`, `.take(n)`, `.skip_while(p)`, `.take_while(p)`, `.step_by(n)` or `.rev()`, left out | defaults |
| [`stream/remove-other-stage`](#remove-other-stage) | `.inspect(f)` or `.map(f)`, left out | extended |

The ids are the Java edition's, where these are stream operators. Each card below comes from
`mutastic run fixtures/operators` with these operators, on `src/iterators.rs`.

## Swap match

`stream/swap-match`, in the defaults. `it.any(p)` becomes `it.all(p)`, and the other way round.
With one item, or items that all agree, the two give the same answer.

<!-- from: fixtures/operators/src/iterators.rs -->
```rust
/// Whether any invoice is more than 30 days late.
pub fn any_overdue(days_late: &[u32]) -> bool {
    days_late.iter().any(|days| *days > 30)
}
```

<!-- from: fixtures/operators/src/iterators.rs -->
```rust
#[test]
fn one_late_invoice() {
    assert!(any_overdue(&[45]));
}
```

<!-- output: operators/iterators/swap-match -->
```text
 [✗ MISSED] tests pass even when any becomes all in any_overdue()
   ┌─ src/iterators.rs:5
   │
 4 │ pub fn any_overdue(days_late: &[u32]) -> bool {
 5 │     days_late.iter().any(|days| *days > 30)
   │                      ^^^ changed to all
 6 │ }
   │
   = mutant  - days_late.iter().any(|days| *days > 30)
             + days_late.iter().all(|days| *days > 30)
   = note: covered by iterators::tests::one_late_invoice (its 1 covering test still passes)
   = help: use inputs where any and all give different results, and assert what any_overdue() does

   done when  mutastic verify m2f11fb5d fixtures/operators passes
   reproduce  mutastic run --only=m2f11fb5d fixtures/operators
```

## Remove stage

`stream/remove-stage`, in the defaults. A stage is left out of a chain that plainly starts as an
iterator, up to its consumer or the `for` loop it feeds: `.filter(p)`, `.skip(n)`, `.take(n)`,
`.skip_while(p)`, `.take_while(p)`, `.step_by(n)` and `.rev()`. Not `.take(n)` on an iterator that
never ends, which would only hang, nor `.rev()` before a consumer that ignores order, such as
`sum`.

<!-- from: fixtures/operators/src/iterators.rs -->
```rust
/// The total of the orders that are paid.
pub fn paid_total(orders: &[(u32, bool)]) -> u32 {
    orders
        .iter()
        .filter(|(_, paid)| *paid)
        .map(|(total, _)| total)
        .sum()
}
```

<!-- from: fixtures/operators/src/iterators.rs -->
```rust
#[test]
fn a_paid_order() {
    assert_eq!(paid_total(&[(10, true)]), 10);
}
```

No test has an unpaid order to leave out:

<!-- output: operators/iterators/remove-stage -->
```text
 [✗ MISSED] tests pass even without .filter(|(_, paid)| *paid) in paid_total()
    ┌─ src/iterators.rs:12
    │
 11 │         .iter()
 12 │         .filter(|(_, paid)| *paid)
    │          ^^^^^^ left out
 13 │         .map(|(total, _)| total)
    │
    = mutant  - .filter(|(_, paid)| *paid)
              + (nothing: the line is gone)
    = note: covered by iterators::tests::a_paid_order (its 1 covering test still passes)
    = help: use inputs that .filter(|(_, paid)| *paid) makes a difference to, and assert what paid_total() does

    done when  mutastic verify ma6f92819 fixtures/operators passes
    reproduce  mutastic run --only=ma6f92819 fixtures/operators
```

## Remove other stage

`stream/remove-other-stage`, in the `extended` set. `.inspect(f)` or `.map(f)` is left out of an
iterator chain, as remove stage leaves out its stages. Leaving out a `map` usually changes the
item's type, and the build withdraws those mutants: in `paid_total` above, the `map` that picks the
total is withdrawn.

<!-- from: fixtures/operators/src/iterators.rs -->
```rust
/// The total, with each value written to `log` as it's added.
pub fn logged_total(values: &[u32], log: &mut Vec<u32>) -> u32 {
    values.iter().inspect(|value| log.push(**value)).sum()
}
```

<!-- from: fixtures/operators/src/iterators.rs -->
```rust
#[test]
fn totals_add_up() {
    let mut log = Vec::new();
    assert_eq!(logged_total(&[1, 2], &mut log), 3);
}
```

<!-- output: operators/iterators/remove-other-stage -->
```text
 [✗ MISSED] tests pass even without .inspect(|value| log.push(**value)) in logged_total()
    ┌─ src/iterators.rs:19
    │
 18 │ pub fn logged_total(values: &[u32], log: &mut Vec<u32>) -> u32 {
 19 │     values.iter().inspect(|value| log.push(**value)).sum()
    │                   ^^^^^^^ left out
 20 │ }
    │
    = mutant  - values.iter().inspect(|value| log.push(**value)).sum()
              + values.iter().sum()
    = note: covered by iterators::tests::totals_add_up (its 1 covering test still passes)
    = help: use inputs that .inspect(|value| log.push(**value)) makes a difference to, and assert what logged_total() does

    done when  mutastic verify m23f62e2d fixtures/operators passes
    reproduce  mutastic run --only=m23f62e2d fixtures/operators
```
