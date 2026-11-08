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
  ✗ [MISSED]  tests pass even without .trim() in username()
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

## Swap arguments

`mutastic/swap-arguments`, in the `stronger` set. A call to a crate function or method whose last
two parameters have one type gets those two arguments swapped: `transfer(amount, from, to)` becomes
`transfer(amount, to, from)`.

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
  ✗ [MISSED]  tests pass even when 0, 100 becomes 100, 0 in is_percent()
    ┌─ src/arguments.rs:12
    │
 11 │ pub fn is_percent(value: i32) -> bool {
 12 │     between(value, 0, 100)
    │                    ^^^^^^ changed to 100, 0
 13 │ }
    │
    = mutant  - between(value, 0, 100)
              + between(value, 100, 0)
    = note: inputs seen: value = 150
            covered by arguments::tests::too_much_is_not_a_percent (its 1 covering test still passes)
    = help: use inputs where 0, 100 and 100, 0 give different results, and assert what is_percent() does

    done when  mutastic verify m1a3c85a8 fixtures/operators passes
    reproduce  mutastic run --only=m1a3c85a8 fixtures/operators
```

## Null parameter

`mutastic/null-parameter`, in the `stronger` set. A parameter of type `Option<T>` is set to `None`
as the function starts, as if every caller passed `None`.

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
  ✗ [MISSED]  tests pass even when coupon is None from the start of price_with()
    ┌─ src/arguments.rs:23
    │
 22 │ 
 23 │ pub fn price_with(base: u32, coupon: Option<u32>) -> u32 {
    │                              ^^^^^^ now None
 24 │     base - coupon.unwrap_or(0)
    │
    = mutant  - pub fn price_with(base: u32, coupon: Option<u32>) -> u32 {
              + pub fn price_with(base: u32, coupon: Option<u32>) -> u32 { let coupon: Option<u32> = None;
    = note: inputs seen: base = 10
            covered by arguments::tests::full_price_without_a_coupon (its 1 covering test still passes)
    = help: add a test where the coupon given changes what price_with() does, and assert it

    done when  mutastic verify m51d65a09 fixtures/operators passes
    reproduce  mutastic run --only=m51d65a09 fixtures/operators
```

## Drop vararg

`mutastic/drop-vararg`, in the `extended` set. The last value of an array or a `vec!` built for a
call is left out: `f(&[a, b, c])` becomes `f(&[a, b])`, and `f(vec![a, b, c])` becomes `f(vec![a,
b])`. Where the length is part of the type, the build withdraws the mutant, which is why it's in
`extended`.

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
  ✗ [MISSED]  tests pass even without c in best_of_three()
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

    done when  mutastic verify m6bcfe468 fixtures/operators passes
    reproduce  mutastic run --only=m6bcfe468 fixtures/operators
```

## Default parameter

`mutastic/default-parameter`, in the `extended` set, and only in Rust. A parameter is set to its
type's `Default::default()` as the function starts: for std's numbers, `bool`, `char`, `String` and
collections, and for a type the crate plainly derives or implements `Default` for. An `Option` is
null parameter's.

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
  ✗ [MISSED]  tests pass even when excited is Default::default() from the start of greet()
    ┌─ src/arguments.rs:27
    │
 26 │ 
 27 │ pub fn greet(name: String, excited: bool) -> String {
    │                            ^^^^^^^ now Default::default()
 28 │     if excited {
    │
    = mutant  - pub fn greet(name: String, excited: bool) -> String {
              + pub fn greet(name: String, excited: bool) -> String { let excited: bool = Default::default();
    = note: covered by arguments::tests::a_calm_greeting (its 1 covering test still passes)
    = help: add a test where the excited given changes what greet() does, and assert it

    done when  mutastic verify m68aafb6e fixtures/operators passes
    reproduce  mutastic run --only=m68aafb6e fixtures/operators
```
