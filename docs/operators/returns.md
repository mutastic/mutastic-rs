# Returns and errors

Operators that change what a function gives back, or the error it gives. They ask whether your
tests look at a function's result at all, and at its failures as well as its successes.

| Operator | Changes | Set |
|---|---|---|
| [`mutastic/return-value`](#return-value) | a returned value to another of its type: `0`, the other `bool`, `None`, an empty one | defaults |
| [`mutastic/unwrap-try`](#unwrap-try) | `f()?` to `f().unwrap()` | defaults |
| [`mutastic/replace-body`](#replace-body) | a function's whole body to one value, or to nothing | extended |
| [`mutastic/remove-throw`](#remove-throw) | `return Err(e);` or `panic!(..);` removed, so the code carries on | extended |

Each card below comes from `mutastic run fixtures/operators` with these operators, on
`src/returns.rs`.

## Return value

`mutastic/return-value`, in the defaults. A returned value, the tail of a function or a `return`,
becomes another value of its type, as plain from the signature:

| Returns | Becomes |
|---|---|
| a whole number | `0`, or `1` where it was `0` |
| a float | `0.0`, or `1.0` |
| `bool` | the other one |
| `String`, `&str` | `String::new()`, `""` |
| `Option<T>` | `None` |
| `Vec`, `HashMap` and std's other collections, `PathBuf` | an empty one |
| `&[T]` | `&[]` |
| `Cow<str>` | `Cow::Borrowed("")` |
| `Box<T>`, `Rc<T>`, `Arc<T>` | `Box::new(..)`, `Rc::new(..)` or `Arc::new(..)` of a value for `T` from this table |
| `Result<T, E>` | `Ok(..)` of one of these values, and `Ok(())` for `Result<()>` |

**The bug it stands for:** a function that doesn't return what it worked out. This is less a typo
than a sign: tests that call the function but never look at its answer. The value is the one a
function of that kind most plausibly gets wrong: a check that answers the wrong way, a count of
nothing, a search that finds nothing.

A `fmt` function's `Ok(())` is left alone: what it writes is the result.

With `extended`, it also makes `Default::default()` for a type that plainly derives or implements
`Default`, and an `Err(..)` that the same function returns elsewhere, when it mentions nothing
local.

<!-- from: fixtures/operators/src/returns.rs -->
```rust
/// A point for every 10 spent.
pub fn loyalty_points(spent: u32) -> u32 {
    spent / 10
}
```

<!-- from: fixtures/operators/src/returns.rs -->
```rust
#[test]
fn small_spends_earn_nothing() {
    assert_eq!(loyalty_points(5), 0);
}
```

The only test spends too little to earn a point, so a function that always returns 0 passes it:

<!-- output: operators/returns/return-value -->
```text
 [✗ MISSED] no test checks what loyalty_points() returns here
   ┌─ src/returns.rs:5
   │
 4 │ pub fn loyalty_points(spent: u32) -> u32 {
 5 │     spent / 10
   │     ^^^^^^^^^^ now returns 0
 6 │ }
   │
   = mutant  - spent / 10
             + 0
   = note: tests reach this return, but none fails when it returns 0
           inputs seen: spent = 5
           covered by returns::tests::small_spends_earn_nothing (its 1 covering test still passes)
   = help: assert the value loyalty_points() returns on this path, with inputs where it isn't 0

           // in mod returns::tests
           #[test]
           fn loyalty_points_with_spent_10() {
               assert_eq!(loyalty_points(10), 1); // today 1 · with this mutant 0
           }
           check today's value is the one you intend: the test pins what the code does now

   done when  mutastic verify m5dafbb78 fixtures/operators passes
   reproduce  mutastic run --only=m5dafbb78 fixtures/operators
```

**The test that catches it:** one that asserts a result other than the mutant's: here, a spend that
earns points.

<!-- added to: fixtures/operators/src/returns.rs -->
```rust
#[test]
fn a_point_for_every_10() {
    assert_eq!(loyalty_points(35), 3);
}
```

**Edition notes:** the Java edition has this operator too, `mutastic/return-value`, with Java's
empty values (`0`, `""`, `List.of()`, `Optional.empty()`) and `null` for anything else.

## Unwrap try

`mutastic/unwrap-try`, in the defaults, and only in Rust. `f()?` becomes `f().unwrap()`: the
error that `?` would hand back to the caller panics instead. It goes unnoticed when no test makes
that call fail, so the error path is never run. Line coverage can't see that: the line runs either
way.

**The bug it stands for:** an error path never taken: a `?` whose error no test makes happen. Code
that handles a bad input by returning the error is only as good as the test that sends one.

<!-- from: fixtures/operators/src/returns.rs -->
```rust
pub fn parse_quantity(text: &str) -> Result<u32, std::num::ParseIntError> {
    let quantity: u32 = text.trim().parse()?;
    Ok(quantity)
}
```

<!-- from: fixtures/operators/src/returns.rs -->
```rust
#[test]
fn spaces_are_ignored() {
    assert_eq!(parse_quantity(" 3 "), Ok(3));
}
```

<!-- output: operators/returns/unwrap-try -->
```text
 [✗ MISSED] no test makes this ? return its error from parse_quantity()
    ┌─ src/returns.rs:14
    │
 13 │ pub fn parse_quantity(text: &str) -> Result<u32, std::num::ParseIntError> {
 14 │     let quantity: u32 = text.trim().parse()?;
    │                                            ^ ? changed to .unwrap()
 15 │     Ok(quantity)
    │
    = mutant  - let quantity: u32 = text.trim().parse()?;
              + let quantity: u32 = text.trim().parse().unwrap();
    = note: covered by returns::tests::spaces_are_ignored (its 1 covering test still passes)
    = help: add a test where the expression before ? fails, and assert that parse_quantity() returns the error

    done when  mutastic verify m18c28709 fixtures/operators passes
    reproduce  mutastic run --only=m18c28709 fixtures/operators
```

**The test that catches it:** one where the expression before `?` fails, asserting the error comes
back.

<!-- added to: fixtures/operators/src/returns.rs -->
```rust
#[test]
fn a_word_is_not_a_quantity() {
    assert!(parse_quantity("three").is_err());
}
```

A `?` that only passes on a formatter's error is skipped as equivalent: inside the `fmt` of
`Display`, `Debug` or another formatting trait, and anywhere a `?` is on a write to a `Formatter`
the function takes (`write!(f, ..)?`, `f.write_str(..)?`, `helper(f, ..)?`). That error comes from
the writer under the formatter, and what tests format into, a `String`, never fails, so no test can
take that path. The mutant keeps its id, with its reason in the events (`"skipKind": "equivalent"`),
and stays out of the score:

<!-- from: fixtures/operators/src/returns.rs -->
```rust
impl std::fmt::Display for Pieces {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)?;
        f.write_str(" pcs")
    }
}
```

<!-- output: operators/returns/unwrap-try-skipped -->
```text
{
  "schema": "mutastic.events/v1",
  "edition": "rust",
  "reason": "mutant-finished",
  "id": "m7e54e30e",
  "fingerprint": "7e54e30e49188313",
  "status": "Ignored",
  "operator": "mutastic/unwrap-try",
  "location": {
    "file": "src/returns.rs",
    "line": 30,
    "column": 32,
    "class": "returns::<Pieces as std::fmt::Display>",
    "method": "fmt"
  },
  "mutation": {
    "original": "?",
    "replacement": ".unwrap()",
    "description": "`?` becomes `.unwrap()`",
    "applied": {
      "level": "source",
      "text": "write!(f, \"{}\", self.0)? -> (write!(f, \"{}\", self.0)).unwrap()"
    }
  },
  "statusReason": "equivalent: differs only when the formatter fails. That can't happen because inside `fmt` its error comes from the writer under it, which no test can make fail.",
  "skipKind": "equivalent",
  "coveredBy": [],
  "testsRun": 0,
  "durationMs": 0
}
```

**Edition notes:** Rust only: Java has no `?`.

## Replace body

`mutastic/replace-body`, in the `extended` set. A function's whole body becomes one value
return value would give, or `{}` when it returns nothing. Not `new`, `fmt`, `hash`, or a getter that
returns one field. It goes unnoticed when no test can tell the function from one that does nothing.

**The bug it stands for:** a function that's wrong from top to bottom, or does nothing, and nobody
would know: not a slip in one line, but code no test depends on. It costs in the code everyone
assumes is tested because coverage says it runs.

<!-- from: fixtures/operators/src/returns.rs -->
```rust
/// Six digits.
pub fn is_valid_code(code: &str) -> bool {
    code.len() == 6 && code.chars().all(|c| c.is_ascii_digit())
}
```

<!-- from: fixtures/operators/src/returns.rs -->
```rust
#[test]
fn a_good_code() {
    assert!(is_valid_code("123456"));
}
```

<!-- output: operators/returns/replace-body -->
```text
 [✗ MISSED] tests pass even when is_valid_code() does nothing but return true
    ┌─ src/returns.rs:9
    │
  8 │ /// Six digits.
  9 │ pub fn is_valid_code(code: &str) -> bool {
    │        ^^^^^^^^^^^^^ only returns true
 10 │     code.len() == 6 && code.chars().all(|c| c.is_ascii_digit())
    │
    = mutant  - /// Six digits.
              - pub fn is_valid_code(code: &str) -> bool {
              -     code.len() == 6 && code.chars().all(|c| c.is_ascii_digit())
              - }
              + /// Six digits.
              + pub fn is_valid_code(code: &str) -> bool { true }
    = note: covered by returns::tests::a_good_code (its 1 covering test still passes)
    = help: add a test where is_valid_code() returns something other than true, and assert it

    done when  mutastic verify m4f13eb3c fixtures/operators passes
    reproduce  mutastic run --only=m4f13eb3c fixtures/operators
```

**The test that catches it:** one that calls the function and asserts a result other than that
value: here, a code that isn't valid.

<!-- added to: fixtures/operators/src/returns.rs -->
```rust
#[test]
fn a_letter_spoils_a_code() {
    assert!(!is_valid_code("12a456"));
}
```

It's in `extended` because it overlaps return value: a test that catches one mostly catches the
other.

**Edition notes:** the Java edition has this operator too, `mutastic/replace-body`, and leaves alone
constructors and trivial getters and setters.

## Remove throw

`mutastic/remove-throw`, in the `extended` set. A `return Err(e);`, a `panic!(..);` or a
`bail!(..);` is removed, and the code carries on past it. Not when it's the last statement of a block
whose value is used. It goes unnoticed when a test checks that there's an error, but not which.

**The bug it stands for:** a check that doesn't stop anything: bad input carries on into the code
that assumed it had been refused, and what comes out is wrong, or a different error from the one the
caller handles.

<!-- from: fixtures/operators/src/returns.rs -->
```rust
pub fn quantity(text: &str) -> Result<u32, String> {
    if text.is_empty() {
        return Err("no quantity given".into());
    }
    text.parse().map_err(|_| format!("{text} isn't a number"))
}
```

<!-- from: fixtures/operators/src/returns.rs -->
```rust
#[test]
fn an_empty_quantity_is_an_error() {
    assert!(quantity("").is_err());
}
```

Without the early return, `"".parse()` fails too, and the test still gets an error:

<!-- output: operators/returns/remove-throw -->
```text
 [✗ MISSED] no test checks what return Err("no quantity given".into()); does in quantity()
    ┌─ src/returns.rs:20
    │
 19 │     if text.is_empty() {
 20 │         return Err("no quantity given".into());
    │         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ removed
 21 │     }
    │
    = mutant  - return Err("no quantity given".into());
    = note: covered by returns::tests::an_empty_quantity_is_an_error (its 1 covering test still passes)
    = help: assert what return Err("no quantity given".into()); changes, in a test of quantity()

    done when  mutastic verify ma1f287d8 fixtures/operators passes
    reproduce  mutastic run --only=ma1f287d8 fixtures/operators
```

**The test that catches it:** one where it fails, asserting that it's this error, not one that
happens to be there anyway.

<!-- added to: fixtures/operators/src/returns.rs -->
```rust
#[test]
fn no_quantity_is_refused_as_such() {
    assert_eq!(quantity(""), Err("no quantity given".to_string()));
}
```

**Edition notes:** the Java edition has this operator too, `mutastic/remove-throw`, for `throw`, in
`extended` and experimental.
