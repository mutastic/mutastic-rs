# Arguments

Operators that change what a function is given, or what a value goes through on its way: a call
left out, arguments swapped or dropped, a parameter replaced. They ask whether your tests would
notice a value arriving wrong.

| Operator | Changes | Set |
|---|---|---|
| [`mutastic/omit-call`](#omit-call) | a call that returns what it's given, left out: `s.trim()` to `s` | stronger |
| [`mutastic/swap-arguments`](#swap-arguments) | a call's last two arguments of one type, swapped | stronger |
| [`mutastic/null-parameter`](#null-parameter) | an `Option` parameter, `None` from the start of the function | stronger |
| [`mutastic/drop-vararg`](#drop-vararg) | the last value of an array or `vec!` passed to a call, left out | extended |
| [`mutastic/default-parameter`](#default-parameter) | a parameter, its type's default from the start of the function | extended |

Each card below comes from `mutastic run fixtures/operators` with these operators, on
`src/arguments.rs`.

## Omit call

`mutastic/omit-call`, in the `stronger` set. A call that gives back the kind of value it's given is
left out: `s.trim()` becomes `s`, `x.abs()` becomes `x`, and the same for `to_lowercase`, `round`
and std's other such methods; a builder's `(self) -> Self` step; and a crate function that returns
its one argument's type, `escape(text)` becoming `text`. Some of these change the type, as
`to_lowercase` turns a `&str` into a `String`; the build withdraws those as unviable.

**The bug it stands for:** the step that was never written: the `trim` that isn't on a pasted value,
the escape that isn't around user input. The code reads as finished without it, and on clean input,
which is what tests tend to have, it makes no difference.

<!-- from: fixtures/operators/src/arguments.rs -->
```rust
pub fn username(input: &str) -> String {
    input.trim().to_lowercase()
}
```

<!-- from: fixtures/operators/src/arguments.rs -->
```rust
#[test]
fn usernames_are_lowercase() {
    assert_eq!(username("Ann"), "ann");
}
```

No test gives a name with spaces round it:

<!-- output: operators/arguments/omit-call -->
```text
 [✗ MISSED] tests pass even without .trim() in username()
   ┌─ src/arguments.rs:4
   │
 3 │ pub fn username(input: &str) -> String {
 4 │     input.trim().to_lowercase()
   │           ^^^^ left out
 5 │ }
   │
   = mutant  - input.trim().to_lowercase()
             + input.to_lowercase()
   = note: covered by arguments::tests::usernames_are_lowercase (its 1 covering test still passes)
   = help: use inputs that .trim() makes a difference to, and assert what username() does

   done when  mutastic verify m3b69ecd2 fixtures/operators passes
   reproduce  mutastic run --only=m3b69ecd2 fixtures/operators
```

**The test that catches it:** one with a value the call changes: here, spaces to trim.

<!-- added to: fixtures/operators/src/arguments.rs -->
```rust
#[test]
fn spaces_are_not_part_of_a_username() {
    assert_eq!(username(" Ann "), "ann");
}
```

**Edition notes:** the Java edition has this operator too, `mutastic/omit-call`, for `String`'s
methods, `Math.abs` and your own.

## Swap arguments

`mutastic/swap-arguments`, in the `stronger` set. A call to a crate function or method whose last
two parameters have one type gets those two arguments swapped: `transfer(amount, from, to)` becomes
`transfer(amount, to, from)`.

**The bug it stands for:** two arguments the wrong way round. When they have the same type the
compiler has nothing to say, and a test whose values give the same answer either way has nothing to
say either. It costs in anything with a direction: a range's start and end, a transfer's source and
destination.

<!-- from: fixtures/operators/src/arguments.rs -->
```rust
pub fn between(value: i32, low: i32, high: i32) -> bool {
    low <= value && value <= high
}

pub fn is_percent(value: i32) -> bool {
    between(value, 0, 100)
}
```

<!-- from: fixtures/operators/src/arguments.rs -->
```rust
#[test]
fn too_much_is_not_a_percent() {
    assert!(!is_percent(150));
}
```

With the range turned round, nothing is a percent, and 150 still isn't:

<!-- output: operators/arguments/swap-arguments -->
```text
 [✗ MISSED] tests pass even when 0, 100 becomes 100, 0 in is_percent()
    ┌─ src/arguments.rs:12
    │
 11 │ pub fn is_percent(value: i32) -> bool {
 12 │     between(value, 0, 100)
    │                    ^^^^^^ 0, 100 changed to 100, 0
 13 │ }
    │
    = mutant  - between(value, 0, 100)
              + between(value, 100, 0)
    = note: inputs seen: value = 150
            covered by arguments::tests::too_much_is_not_a_percent (its 1 covering test still passes)
    = help: use inputs where 0, 100 and 100, 0 give different results, and assert what is_percent() does

            // in mod arguments::tests
            #[test]
            fn is_percent_with_value_0() {
                assert!(is_percent(0)); // today true · with this mutant false
            }
            check today's value is the one you intend: the test pins what the code does now

    done when  mutastic verify m1a3c85a8 fixtures/operators passes
    reproduce  mutastic run --only=m1a3c85a8 fixtures/operators
```

**The test that catches it:** one that passes values whose order shows: here, a percentage that is
one.

<!-- added to: fixtures/operators/src/arguments.rs -->
```rust
#[test]
fn fifty_is_a_percent() {
    assert!(is_percent(50));
}
```

**Edition notes:** the Java edition has this operator too, `mutastic/swap-arguments`, for any call's
last two arguments of one type.

## Null parameter

`mutastic/null-parameter`, in the `stronger` set. A parameter of type `Option<T>` is set to `None`
as the function starts, as if every caller passed `None`.

**The bug it stands for:** a value that's handed in and doesn't matter as it should: a parameter
ignored on one path. It costs in code with many inputs where tests pin only some.

<!-- from: fixtures/operators/src/arguments.rs -->
```rust
pub fn price_with(base: u32, coupon: Option<u32>) -> u32 {
    base - coupon.unwrap_or(0)
}
```

<!-- from: fixtures/operators/src/arguments.rs -->
```rust
#[test]
fn full_price_without_a_coupon() {
    assert_eq!(price_with(10, None), 10);
}
```

<!-- output: operators/arguments/null-parameter -->
```text
 [✗ MISSED] tests pass even when coupon is None from the start of price_with()
    ┌─ src/arguments.rs:23
    │
 22 │ 
 23 │ pub fn price_with(base: u32, coupon: Option<u32>) -> u32 {
    │                              ^^^^^^ now None
 24 │     base - coupon.unwrap_or(0)
    │
    = mutant  - pub fn price_with(base: u32, coupon: Option<u32>) -> u32 {
              -     base - coupon.unwrap_or(0)
              - }
              + pub fn price_with(base: u32, coupon: Option<u32>) -> u32 { let coupon: Option<u32> = None;
              +     base - coupon.unwrap_or(0)
              + }
    = note: inputs seen: base = 10
            covered by arguments::tests::full_price_without_a_coupon (its 1 covering test still passes)
    = help: add a test where the coupon given changes what price_with() does, and assert it

    done when  mutastic verify m51d65a09 fixtures/operators passes
    reproduce  mutastic run --only=m51d65a09 fixtures/operators
```

**The test that catches it:** one whose result depends on the parameter: here, a coupon.

<!-- added to: fixtures/operators/src/arguments.rs -->
```rust
#[test]
fn a_coupon_comes_off() {
    assert_eq!(price_with(10, Some(3)), 7);
}
```

**Edition notes:** the Java edition has this operator too, `mutastic/null-parameter`, for any
parameter holding an object, which becomes `null`.

## Drop vararg

`mutastic/drop-vararg`, in the `extended` set. The last value of an array or a `vec!` built for a
call is left out: `f(&[a, b, c])` becomes `f(&[a, b])`, and `f(vec![a, b, c])` becomes `f(vec![a,
b])`. Where the length is part of the type, the build withdraws the mutant, which is why it's in
`extended`.

**The bug it stands for:** a value left out of a list: the role not in the set, the last column not
in the query. It costs in lists built in one expression, where one element's absence shows only for
inputs that need it.

<!-- from: fixtures/operators/src/arguments.rs -->
```rust
pub fn largest(values: &[u32]) -> u32 {
    values.iter().copied().max().unwrap_or(0)
}

pub fn best_of_three(a: u32, b: u32, c: u32) -> u32 {
    largest(&[a, b, c])
}
```

<!-- from: fixtures/operators/src/arguments.rs -->
```rust
#[test]
fn the_best_comes_first() {
    assert_eq!(best_of_three(9, 2, 1), 9);
}
```

<!-- output: operators/arguments/drop-vararg -->
```text
 [✗ MISSED] tests pass even without c in best_of_three()
    ┌─ src/arguments.rs:20
    │
 19 │ pub fn best_of_three(a: u32, b: u32, c: u32) -> u32 {
 20 │     largest(&[a, b, c])
    │                   ^^^ left out
 21 │ }
    │
    = mutant  - largest(&[a, b, c])
              + largest(&[a, b])
    = note: inputs seen: a = 9; b = 2; c = 1
            covered by arguments::tests::the_best_comes_first (its 1 covering test still passes)
    = help: use inputs that c makes a difference to, and assert what best_of_three() does

            // in mod arguments::tests
            #[test]
            fn best_of_three_with_a_9_and_b_9_and_c_10() {
                assert_eq!(best_of_three(9, 9, 10), 10); // today 10 · with this mutant 9
            }
            check today's value is the one you intend: the test pins what the code does now

    done when  mutastic verify m6bcfe468 fixtures/operators passes
    reproduce  mutastic run --only=m6bcfe468 fixtures/operators
```

**The test that catches it:** one that depends on the last value, not only on the others.

<!-- added to: fixtures/operators/src/arguments.rs -->
```rust
#[test]
fn the_best_comes_last() {
    assert_eq!(best_of_three(1, 2, 9), 9);
}
```

**Edition notes:** the Java edition has this operator too, `mutastic/drop-vararg`, in `stronger`,
for an array built for a varargs call.

## Default parameter

`mutastic/default-parameter`, in the `extended` set, and only in Rust. A parameter is set to its
type's `Default::default()` as the function starts: for std's numbers, `bool`, `char`, `String` and
collections, and for a type the crate plainly derives or implements `Default` for. An `Option` is
null parameter's.

**The bug it stands for:** a value that's handed in and makes no difference: a flag ignored, an
option the function never reads on the path tests take.

<!-- from: fixtures/operators/src/arguments.rs -->
```rust
pub fn greet(name: String, excited: bool) -> String {
    if excited {
        format!("Hi {name}!")
    } else {
        format!("Hi {name}")
    }
}
```

<!-- from: fixtures/operators/src/arguments.rs -->
```rust
#[test]
fn a_calm_greeting() {
    assert_eq!(greet("Ann".into(), false), "Hi Ann");
}
```

`name` as an empty string is caught. `excited` as `false` isn't, as the only test passes `false`:

<!-- output: operators/arguments/default-parameter -->
```text
 [✗ MISSED] tests pass even when excited is Default::default() from the start of greet()
    ┌─ src/arguments.rs:27
    │
 26 │ 
 27 │ pub fn greet(name: String, excited: bool) -> String {
    │                            ^^^^^^^ now Default::default()
 28 │     if excited {
    │
    = mutant  - pub fn greet(name: String, excited: bool) -> String {
              -     if excited {
              -         format!("Hi {name}!")
              -     } else {
              -         format!("Hi {name}")
              -     }
              - …
              + pub fn greet(name: String, excited: bool) -> String { let excited: bool = Default::default();
              +     if excited {
              +         format!("Hi {name}!")
              +     } else {
              +         format!("Hi {name}")
              +     }
              + …
    = note: covered by arguments::tests::a_calm_greeting (its 1 covering test still passes)
    = help: add a test where the excited given changes what greet() does, and assert it

    done when  mutastic verify m68aafb6e fixtures/operators passes
    reproduce  mutastic run --only=m68aafb6e fixtures/operators
```

**The test that catches it:** one where the value given changes the result.

<!-- added to: fixtures/operators/src/arguments.rs -->
```rust
#[test]
fn an_excited_greeting() {
    assert_eq!(greet("Ann".into(), true), "Hi Ann!");
}
```

**Edition notes:** Rust only. The Java edition's nearest is [null parameter](#null-parameter).
