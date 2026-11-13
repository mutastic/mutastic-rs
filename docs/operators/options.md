# `Option` and `Result`

Operators that change how an `Option` or a `Result` is asked about or unwrapped. They ask whether
your tests try the `None` and the `Some`, the `Err` and the `Ok`, and the default as well as the
value.

| Operator | Changes | Set |
|---|---|---|
| [`optional/swap-present`](#swap-present) | `is_some` and `is_none`, `is_ok` and `is_err` | stronger |
| [`optional/empty`](#empty) | `Some(x)` to `None` | stronger |
| [`optional/always-default`](#always-default) | `o.unwrap_or(d)` to `d` | stronger |
| [`optional/no-default`](#no-default) | `o.unwrap_or(d)` to `o.unwrap()` | stronger |
| [`optional/remove-filter`](#remove-filter) | `o.filter(p)` to `o` | extended |

The ids are the Java edition's, for `Optional`. Four of them are in `extended` there, and in
`stronger` here, because `Option` and `Result` are core Rust, not a newer library. Each card below
comes from `mutastic run fixtures/operators` with these operators, on `src/options.rs`.

## Swap present

`optional/swap-present`, in the `stronger` set. `is_some()` and `is_none()` swap, and so do `is_ok()`
and `is_err()`. mutastic matches these by name, so on a type of your own that has `is_none` but no
`is_some`, the build withdraws the mutant.

**The bug it stands for:** a check the wrong way round, so that the code handles a missing value as
if it were there. It costs on the path a test rarely tries: the customer who isn't found, the
setting that isn't set.

<!-- from: fixtures/operators/src/options.rs -->
```rust
pub fn display_name(name: &str, nickname: Option<&str>) -> String {
    if nickname.is_none() {
        return name.to_string();
    }
    nickname.unwrap_or(name).to_string()
}
```

<!-- from: fixtures/operators/src/options.rs -->
```rust
#[test]
fn no_nickname() {
    assert_eq!(display_name("Ann", None), "Ann");
}
```

With no nickname, the early return and the `unwrap_or` give the same name, so the test can't tell
which way the check went:

<!-- output: operators/options/swap-present -->
```text
  ✗ MISSED  tests pass even when is_none becomes is_some in display_name()
   ┌─ src/options.rs:4
   │
 3 │ pub fn display_name(name: &str, nickname: Option<&str>) -> String {
 4 │     if nickname.is_none() {
   │                 ^^^^^^^ is_none changed to is_some
 5 │         return name.to_string();
   │
   = mutant  - if nickname.is_none() {
             + if nickname.is_some() {
   = note: covered by options::tests::no_nickname (its 1 covering test still passes)
   = help: use inputs where is_none and is_some give different results, and assert what display_name() does

   done when  mutastic verify maedc1a33 fixtures/operators passes
   reproduce  mutastic run --only=maedc1a33 fixtures/operators
```

**The test that catches it:** one with a value and one without, asserting what each does.

<!-- added to: fixtures/operators/src/options.rs -->
```rust
#[test]
fn a_nickname_is_shown() {
    assert_eq!(display_name("Ann", Some("Annie")), "Annie");
}
```

**Edition notes:** the Java edition has this operator too, `optional/swap-present`, in `extended`,
for `Optional`'s `isPresent` and `isEmpty`.

## Empty

`optional/empty`, in the `stronger` set. `Some(x)` becomes `None`, where it isn't the function's
return value, which return value covers.

**The bug it stands for:** code that gives back nothing where it should give a value, which callers
treat as "not found" without complaint. An empty result looks just like a real absence.

<!-- from: fixtures/operators/src/options.rs -->
```rust
/// 20 to a page unless a size is chosen.
pub fn page_size(chosen: u32) -> u32 {
    let custom = if chosen > 0 { Some(chosen) } else { None };
    custom.unwrap_or(20)
}
```

<!-- from: fixtures/operators/src/options.rs -->
```rust
#[test]
fn a_chosen_page_size() {
    assert_eq!(page_size(20), 20);
}
```

The test chose the size that's also the default:

<!-- output: operators/options/empty -->
```text
  ✗ MISSED  tests pass even when Some(chosen) becomes None in page_size()
    ┌─ src/options.rs:12
    │
 11 │ pub fn page_size(chosen: u32) -> u32 {
 12 │     let custom = if chosen > 0 { Some(chosen) } else { None };
    │                                  ^^^^ changed to None
 13 │     custom.unwrap_or(20)
    │
    = mutant  - let custom = if chosen > 0 { Some(chosen) } else { None };
              + let custom = if chosen > 0 { None } else { None };
    = note: inputs seen: chosen = 20
            covered by options::tests::a_chosen_page_size (its 1 covering test still passes)
    = help: use inputs where Some(chosen) and None differ, and assert what page_size() does

            // in mod options::tests
            #[test]
            fn page_size_with_chosen_1() {
                assert_eq!(page_size(1), 1); // today 1 · with this mutant 20
            }
            check today's value is the one you intend: the test pins what the code does now

    done when  mutastic verify m328e06d3 fixtures/operators passes
    reproduce  mutastic run --only=m328e06d3 fixtures/operators
```

**The test that catches it:** one that gets the value out, with a value unlike the default.

<!-- added to: fixtures/operators/src/options.rs -->
```rust
#[test]
fn a_page_of_fifty() {
    assert_eq!(page_size(50), 50);
}
```

**Edition notes:** the Java edition has this operator too, `optional/empty`, in `extended`, for
`Optional.of` and `ofNullable`.

## Always default

`optional/always-default`, in the `stronger` set. The value is never used, only the default:
`o.unwrap_or(d)` becomes `d`, `unwrap_or_default()` becomes `Default::default()`,
`unwrap_or_else(f)` becomes `f()`, and `map_or(d, f)` becomes `d`.

**The bug it stands for:** a default that's always used, so the configured value is ignored. It
costs in configuration: a setting that does nothing until someone's production value matters.

<!-- from: fixtures/operators/src/options.rs -->
```rust
pub fn port(configured: Option<u16>) -> u16 {
    configured.unwrap_or(8080)
}
```

<!-- from: fixtures/operators/src/options.rs -->
```rust
#[test]
fn the_configured_port() {
    assert_eq!(port(Some(8080)), 8080);
}
```

<!-- output: operators/options/always-default -->
```text
  ✗ MISSED  tests pass even when configured.unwrap_or(8080) becomes 8080 in port() · m9e594423
    ┌─ src/options.rs:17
    │
 16 │ pub fn port(configured: Option<u16>) -> u16 {
 17 │     configured.unwrap_or(8080)
    │     ^^^^^^^^^^^^^^^^^^^^^^^^^^ changed to 8080
 18 │ }
    │
    = mutant  - configured.unwrap_or(8080)
              + 8080
    = also    ✗ tests pass even when configured.unwrap_or(8080) becomes configured.unwrap() in port() · ma9fa63c7
              + configured.unwrap()
    = note: covered by options::tests::the_configured_port (its 1 covering test still passes)
    = help: use inputs where configured.unwrap_or(8080) and 8080 differ, and assert what port() does

    done when  mutastic verify m9e594423,ma9fa63c7 fixtures/operators passes
    reproduce  mutastic run --only=m9e594423,ma9fa63c7 fixtures/operators
```

**The test that catches it:** a value that isn't the default, and none at all.

<!-- added to: fixtures/operators/src/options.rs -->
```rust
#[test]
fn a_port_of_ones_own() {
    assert_eq!(port(Some(9090)), 9090);
    assert_eq!(port(None), 8080);
}
```

**Edition notes:** the Java edition has this operator too, `optional/always-default`, in `extended`,
for `Optional.orElse`.

## No default

`optional/no-default`, in the `stronger` set. The default is never used, only the value:
`o.unwrap_or(d)`, `unwrap_or_else(f)` and `unwrap_or_default()` become `o.unwrap()`, which panics
on `None`. It goes unnoticed when no test gets the default. In `port` above it's the second mutant
on the card; here it's on its own.

**The bug it stands for:** no default at all, so a missing value panics. It costs in configuration
and input: a start-up failure when a setting is left out, a panic on an empty list.

<!-- from: fixtures/operators/src/arguments.rs -->
```rust
pub fn largest(values: &[u32]) -> u32 {
    values.iter().copied().max().unwrap_or(0)
}
```

No test calls `largest` with no values, so nothing needs the 0:

<!-- output: operators/options/no-default -->
```text
  ✗ MISSED  tests pass even when values.iter().copied().max().unwrap_or(0) becomes values.iter().copied().max().unwrap() in largest()
    ┌─ src/arguments.rs:16
    │
 15 │ pub fn largest(values: &[u32]) -> u32 {
 16 │     values.iter().copied().max().unwrap_or(0)
    │     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ changed to values.iter().copied().max().unwrap()
 17 │ }
    │
    = mutant  - values.iter().copied().max().unwrap_or(0)
              + values.iter().copied().max().unwrap()
    = note: covered by arguments::tests::the_best_comes_first (its 1 covering test still passes)
    = help: use inputs where values.iter().copied().max().unwrap_or(0) and values.iter().copied().max().unwrap() differ, and assert what largest() does

    done when  mutastic verify m07e8bfcc fixtures/operators passes
    reproduce  mutastic run --only=m07e8bfcc fixtures/operators
```

**The test that catches it:** one with nothing there.

<!-- added to: fixtures/operators/src/arguments.rs -->
```rust
#[test]
fn nothing_to_compare() {
    assert_eq!(largest(&[]), 0);
}
```

**Edition notes:** the Java edition has this operator too, `optional/no-default`, in `extended`, for
`Optional.orElse` and `orElseGet`, which become `get()`.

## Remove filter

`optional/remove-filter`, in the `extended` set. `o.filter(p)` becomes `o`, on a value that isn't
plainly an iterator, nor used as one.

**The bug it stands for:** a check that's missing, or wrong, in a chain: the value is used when it
shouldn't be. It costs in rules that hide in one call: only a plausible age, only a discount that's
still valid.

<!-- from: fixtures/operators/src/options.rs -->
```rust
pub fn plausible_age(age: Option<u32>) -> Option<u32> {
    age.filter(|years| *years < 150)
}
```

<!-- from: fixtures/operators/src/options.rs -->
```rust
#[test]
fn an_ordinary_age() {
    assert_eq!(plausible_age(Some(30)), Some(30));
}
```

<!-- output: operators/options/remove-filter -->
```text
  ✗ MISSED  tests pass even without .filter(|years| *years < 150) in plausible_age()
    ┌─ src/options.rs:21
    │
 20 │ pub fn plausible_age(age: Option<u32>) -> Option<u32> {
 21 │     age.filter(|years| *years < 150)
    │         ^^^^^^ left out
 22 │ }
    │
    = mutant  - age.filter(|years| *years < 150)
              + age
    = note: covered by options::tests::an_ordinary_age (its 1 covering test still passes)
    = help: use inputs that .filter(|years| *years < 150) makes a difference to, and assert what plausible_age() does

    done when  mutastic verify m5a861387 fixtures/operators passes
    reproduce  mutastic run --only=m5a861387 fixtures/operators
```

**The test that catches it:** one with a value the filter must turn away.

<!-- added to: fixtures/operators/src/options.rs -->
```rust
#[test]
fn an_age_too_great_is_refused() {
    assert_eq!(plausible_age(Some(200)), None);
}
```

**Edition notes:** the Java edition has this operator too, `optional/remove-filter`, for
`Optional.filter`.
