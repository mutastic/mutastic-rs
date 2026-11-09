# Calls and writes

Operators that remove a statement: a call whose result nothing uses, an assignment, a write to a
field. They ask whether your tests check what that statement does, not just that the function
returns.

| Operator | Changes | Set |
|---|---|---|
| [`mutastic/remove-void-call`](#remove-void-call) | a call that returns nothing, removed | defaults |
| [`mutastic/remove-discarded-call`](#remove-discarded-call) | a call whose result is thrown away, removed | defaults |
| [`mutastic/remove-assignment`](#remove-assignment) | an assignment to a variable that already has a value, removed | defaults |
| [`mutastic/remove-field-write`](#remove-field-write) | a write to a field, through a reference, or to an element, removed | stronger |
| [`mutastic/remove-struct-field`](#remove-struct-field) | a field set in a struct literal with `..base`, left to `base` | stronger |

Each card below comes from `mutastic run fixtures/operators` with these operators, on
`src/calls.rs`.

## Remove void call

`mutastic/remove-void-call`, in the defaults. A call statement that returns nothing is removed: a
function or method the crate declares with no return type, or one of std's that returns `()`, such
as `push`, `sort`, `clear`, `extend` or `drop`.

**The bug it stands for:** a step left out: a call removed in a refactor, never added, or skipped on
one path. It costs in side effects, which are often the point of the code: saving a record, writing
an audit entry, releasing a lock. If removing the call passes every test, nothing checks the step
happens.

A statement that only writes output, `println!`, `eprintln!`, `dbg!` or a logger's `log::warn!`
or `tracing::info!`, is skipped as arid, `arid: only writes output`, as the Java edition skips a
call to a logger: no test should have to notice it. It's counted in the summary's
`skipped (1 arid)`, so a change that only adds a log line says why nothing ran, and it's out of the
score. A `mutate:` rule takes the skip back, where output is a contract. A `write!` goes to a writer
the code chose, which a test can read, so it isn't skipped.

<!-- from: fixtures/operators/src/calls.rs -->
```rust
impl Cart {
    pub fn add(&mut self, item: &str) {
        self.items.push(item.to_string());
        self.history.push(format!("added {item}"));
    }
```

<!-- from: fixtures/operators/src/calls.rs -->
```rust
#[test]
fn adding_an_item_counts_it() {
    let mut cart = Cart::default();
    cart.add("tea");
    assert_eq!(cart.count(), 1);
}
```

The test counts the items, so removing the first `push` is caught. Nothing reads the history:

<!-- output: operators/calls/remove-void-call -->
```text
 [✗ MISSED] no test checks what self.history.push(format!("added {item}")); does in add()
    ┌─ src/calls.rs:14
    │
 13 │         self.items.push(item.to_string());
 14 │         self.history.push(format!("added {item}"));
    │         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ removed
 15 │     }
    │
    = mutant  - self.history.push(format!("added {item}"));
              + (nothing: the line is gone)
    = note: covered by calls::tests::adding_an_item_counts_it (its 1 covering test still passes)
    = help: assert what self.history.push(format!("added {item}")); changes, in a test of add()

    done when  mutastic verify m9dd52d50 fixtures/operators passes
    reproduce  mutastic run --only=m9dd52d50 fixtures/operators
```

**The test that catches it:** one that checks the effect: here, the history, not only the count.

<!-- added to: fixtures/operators/src/calls.rs -->
```rust
#[test]
fn adding_an_item_is_recorded() {
    let mut cart = Cart::default();
    cart.add("tea");
    assert_eq!(cart.history, vec!["added tea".to_string()]);
}
```

**Edition notes:** the Java edition has the same id, for a method that returns `void`.

## Remove discarded call

`mutastic/remove-discarded-call`, in the defaults. A call whose result is thrown away is removed:
`map.insert(k, v);`, `w.flush()?;`, `let _ = f();`. A bare `x.unwrap();` is left alone, as its only
effect is the panic.

**The bug it stands for:** the same as a removed void call: a step left out. Many calls that do
something also return something nobody needs: `insert` returns what was there, `write` how much it
wrote. These hide best, since a call written for its effect reads like a query.

<!-- from: fixtures/operators/src/calls.rs -->
```rust
/// Whether this is the first time `name` is seen.
pub fn first_visit(seen: &mut HashMap<String, u32>, name: &str) -> bool {
    let first = !seen.contains_key(name);
    seen.insert(name.to_string(), 1);
    first
}
```

<!-- from: fixtures/operators/src/calls.rs -->
```rust
#[test]
fn a_first_visit() {
    let mut seen = HashMap::new();
    assert!(first_visit(&mut seen, "ann"));
}
```

No test visits twice, so nothing needs the name remembered:

<!-- output: operators/calls/remove-discarded-call -->
```text
 [✗ MISSED] no test checks what seen.insert(name.to_string(), 1); does in first_visit()
    ┌─ src/calls.rs:25
    │
 24 │     let first = !seen.contains_key(name);
 25 │     seen.insert(name.to_string(), 1);
    │     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ removed
 26 │     first
    │
    = mutant  - seen.insert(name.to_string(), 1);
              + (nothing: the line is gone)
    = note: covered by calls::tests::a_first_visit (its 1 covering test still passes)
    = help: assert what seen.insert(name.to_string(), 1); changes, in a test of first_visit()

    done when  mutastic verify m691cb348 fixtures/operators passes
    reproduce  mutastic run --only=m691cb348 fixtures/operators
```

**The test that catches it:** one that checks the effect: that a second visit isn't a first.

<!-- added to: fixtures/operators/src/calls.rs -->
```rust
#[test]
fn a_second_visit() {
    let mut seen = HashMap::new();
    first_visit(&mut seen, "ann");
    assert!(!first_visit(&mut seen, "ann"));
}
```

**Edition notes:** the Java edition has the same id, for `add`, `put`, `append` and the like.

## Remove assignment

`mutastic/remove-assignment`, in the defaults. `total = total + tip;` becomes `let _ = total +
tip;`: the value is still worked out, and thrown away. Never the first value of a `let x;`, which
the code couldn't compile without.

**The bug it stands for:** a step left out of a calculation: a running total that isn't updated on
one path, a value worked out and then not kept. The code still runs and still returns something,
only from a value that's out of date.

<!-- from: fixtures/operators/src/calls.rs -->
```rust
pub fn with_tip(bill: u32, tip: u32) -> u32 {
    let mut total = bill;
    total = total + tip;
    total
}
```

<!-- from: fixtures/operators/src/calls.rs -->
```rust
#[test]
fn no_tip() {
    assert_eq!(with_tip(10, 0), 10);
}
```

<!-- output: operators/calls/remove-assignment -->
```text
 [✗ MISSED] no test checks what total = total + tip; does in with_tip()
    ┌─ src/calls.rs:31
    │
 30 │     let mut total = bill;
 31 │     total = total + tip;
    │     ^^^^^ removed
 32 │     total
    │
    = mutant  - total = total + tip;
              + let _ = total + tip;
    = note: inputs seen: bill = 10; tip = 0
            covered by calls::tests::no_tip (its 1 covering test still passes)
    = help: assert what total = total + tip; changes, in a test of with_tip()

            // in mod calls::tests
            #[test]
            fn with_tip_with_bill_10_and_tip_10() {
                assert_eq!(with_tip(10, 10), 20); // today 20 · with this mutant 10
            }
            check today's value is the one you intend: the test pins what the code does now

    done when  mutastic verify mb171c131 fixtures/operators passes
    reproduce  mutastic run --only=mb171c131 fixtures/operators
```

**The test that catches it:** one whose result depends on the new value, not one where the earlier
value happens to be right.

<!-- added to: fixtures/operators/src/calls.rs -->
```rust
#[test]
fn a_tip_is_added() {
    assert_eq!(with_tip(10, 2), 12);
}
```

**Edition notes:** the Java edition has the same id, for a local variable that already has a value.

## Remove field write

`mutastic/remove-field-write`, in the `stronger` set. A write to a field (`self.status = s;`),
through a reference (`*p = v;`) or to an element (`v[i] = x;`) becomes `let _ = s;`.

**The bug it stands for:** state that never changes: an update left out, or a write lost in a
refactor, so the value goes on saying what it said before. Each call looks fine alone; what's wrong
only shows in the next one.

<!-- from: fixtures/operators/src/calls.rs -->
```rust
impl Order {
    /// Marks the order paid, and gives what was due.
    pub fn pay(&mut self) -> u32 {
        self.paid = true;
        self.total
    }
}
```

<!-- from: fixtures/operators/src/calls.rs -->
```rust
#[test]
fn paying_gives_the_total() {
    let mut order = Order {
        paid: false,
        total: 30,
    };
    assert_eq!(order.pay(), 30);
}
```

<!-- output: operators/calls/remove-field-write -->
```text
 [✗ MISSED] no test checks what self.paid = true; does in pay()
    ┌─ src/calls.rs:43
    │
 42 │     pub fn pay(&mut self) -> u32 {
 43 │         self.paid = true;
    │         ^^^^^^^^^ removed
 44 │         self.total
    │
    = mutant  - self.paid = true;
              + let _ = true;
    = note: covered by calls::tests::paying_gives_the_total (its 1 covering test still passes)
    = help: assert what self.paid = true; changes, in a test of pay()

    done when  mutastic verify m17121fd7 fixtures/operators passes
    reproduce  mutastic run --only=m17121fd7 fixtures/operators
```

**The test that catches it:** one that makes the change and then looks at the field.

<!-- added to: fixtures/operators/src/calls.rs -->
```rust
#[test]
fn paying_marks_the_order_paid() {
    let mut order = Order {
        paid: false,
        total: 30,
    };
    order.pay();
    assert!(order.paid);
}
```

**Edition notes:** the Java edition has the same id, for a write to a field.

## Remove struct field

`mutastic/remove-struct-field`, in the `stronger` set, and only in Rust. In a struct literal that
ends in `..base`, a field set before it is left out, so it takes `base`'s value: `S { a, b, ..base }`
becomes `S { a, ..base }`.

**The bug it stands for:** a field left at the value it came with, in an update that was meant to
change it.

<!-- from: fixtures/operators/src/calls.rs -->
```rust
pub fn muted(settings: &Settings) -> Settings {
    Settings {
        muted: true,
        ..settings.clone()
    }
}
```

<!-- from: fixtures/operators/src/calls.rs -->
```rust
#[test]
fn muting_keeps_the_volume() {
    let loud = Settings {
        volume: 9,
        muted: false,
    };
    assert_eq!(muted(&loud).volume, 9);
}
```

The test checks what muting keeps, but not that it mutes:

<!-- output: operators/calls/remove-struct-field -->
```text
 [✗ MISSED] tests pass even without muted: true in muted()
    ┌─ src/calls.rs:56
    │
 55 │     Settings {
 56 │         muted: true,
    │         ^^^^^^^^^^^ left out
 57 │         ..settings.clone()
    │
    = mutant  - muted: true,
              + (nothing: the line is gone)
    = note: covered by calls::tests::muting_keeps_the_volume (its 1 covering test still passes)
    = help: use inputs that muted: true makes a difference to, and assert what muted() does

    done when  mutastic verify m0af1aacd fixtures/operators passes
    reproduce  mutastic run --only=m0af1aacd fixtures/operators
```

**The test that catches it:** one that checks the field the update sets.

<!-- added to: fixtures/operators/src/calls.rs -->
```rust
#[test]
fn muting_mutes() {
    let loud = Settings {
        volume: 9,
        muted: false,
    };
    assert!(muted(&loud).muted);
}
```

**Edition notes:** the Java edition has no such operator, as Java has no struct update syntax.
