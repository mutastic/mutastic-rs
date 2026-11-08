# std's methods and the strings in the code

Operators that swap a method from std for its counterpart, or change a string or a pattern the code
holds. They ask whether your tests would notice the wrong end, the wrong direction, or the wrong
text.

| Operator | Changes | Set |
|---|---|---|
| [`library/swap-string`](#swap-string) | `starts_with` and `ends_with`, `to_uppercase` and `to_lowercase`, `trim_start` and `trim_end`, `strip_prefix` and `strip_suffix` | defaults |
| [`library/swap-math`](#swap-math) | `min` and `max`, `floor` and `ceil` | defaults |
| [`library/swap-index-of`](#swap-index-of) | `find` and `rfind`, `position` and `rposition`, `split_once` and `rsplit_once` | stronger |
| [`library/reverse-ordering`](#reverse-ordering) | `a.cmp(&b)` to `b.cmp(&a)`; `.reverse()` left out | stronger |
| [`mutastic/string-constant`](#string-constant) | a string to `""`, and `""` to `"mutastic"` | stronger |
| [`mutastic/regex-constant`](#regex-constant) | a regular expression to one that differs and still compiles | extended |
| [`bigdecimal/remove-step`](#remove-step) | `round_dp(2)` and the like, left out | extended |

The first five cards come from `mutastic run fixtures/operators` with these operators, on
`src/library.rs`.

## Swap string

`library/swap-string`, in the defaults. A `str` method becomes its counterpart: `starts_with` and
`ends_with`, `to_uppercase` and `to_lowercase` (and their ASCII forms), `trim_start` and `trim_end`
(and their `_matches` forms), `strip_prefix` and `strip_suffix`.

<!-- from: fixtures/operators/src/library.rs -->
```rust
pub fn is_comment(line: &str) -> bool {
    line.starts_with('#')
}
```

<!-- from: fixtures/operators/src/library.rs -->
```rust
#[test]
fn plain_lines_are_not_comments() {
    assert!(!is_comment("hello"));
}
```

<!-- output: operators/library/swap-string -->
```text
  ✗ [MISSED]  tests pass even when starts_with becomes ends_with in is_comment()
   ┌─ src/library.rs:4
   │
 3 │ pub fn is_comment(line: &str) -> bool {
 4 │     line.starts_with('#')
   │          ^^^^^^^^^^^ changed to ends_with
 5 │ }
   │
   = mutant  - line.starts_with('#')
             + line.ends_with('#')
   = note: covered by library::tests::plain_lines_are_not_comments (its 1 covering test still passes)
   = help: use inputs where starts_with and ends_with give different results, and assert what is_comment() does

   done when  mutastic verify m739423e6 fixtures/operators passes
   reproduce  mutastic run --only=m739423e6 fixtures/operators
```

## Swap math

`library/swap-math`, in the defaults. `a.min(b)` and `a.max(b)` swap, as do `cmp::min` and
`cmp::max`, `min_by` and `max_by` (and their `_key` forms), and `floor` and `ceil`.

<!-- from: fixtures/operators/src/library.rs -->
```rust
/// Never more than half off.
pub fn capped_discount(percent: u32) -> u32 {
    percent.min(50)
}
```

<!-- from: fixtures/operators/src/library.rs -->
```rust
#[test]
fn half_off_at_most() {
    assert_eq!(capped_discount(50), 50);
}
```

At exactly the cap, `min` and `max` agree:

<!-- output: operators/library/swap-math -->
```text
  ✗ [MISSED]  tests pass even when min becomes max in capped_discount()
    ┌─ src/library.rs:9
    │
  8 │ pub fn capped_discount(percent: u32) -> u32 {
  9 │     percent.min(50)
    │             ^^^ changed to max
 10 │ }
    │
    = mutant  - percent.min(50)
              + percent.max(50)
    = note: inputs seen: percent = 50
            covered by library::tests::half_off_at_most (its 1 covering test still passes)
    = help: use inputs where min and max give different results, and assert what capped_discount() does

    done when  mutastic verify m5bb85f22 fixtures/operators passes
    reproduce  mutastic run --only=m5bb85f22 fixtures/operators
```

## Swap index of

`library/swap-index-of`, in the `stronger` set. A search from one end becomes the search from the
other: `find` and `rfind`, `position` and `rposition`, `split_once` and `rsplit_once`,
`match_indices` and `rmatch_indices`.

<!-- from: fixtures/operators/src/library.rs -->
```rust
pub fn extension(file: &str) -> Option<&str> {
    file.find('.').map(|dot| &file[dot + 1..])
}
```

<!-- from: fixtures/operators/src/library.rs -->
```rust
#[test]
fn a_text_file() {
    assert_eq!(extension("notes.txt"), Some("txt"));
}
```

With one dot in the name, both ends find the same one. A test of `archive.tar.gz` would tell them
apart:

<!-- output: operators/library/swap-index-of -->
```text
  ✗ [MISSED]  tests pass even when find becomes rfind in extension()
    ┌─ src/library.rs:13
    │
 12 │ pub fn extension(file: &str) -> Option<&str> {
 13 │     file.find('.').map(|dot| &file[dot + 1..])
    │          ^^^^ changed to rfind
 14 │ }
    │
    = mutant  - file.find('.').map(|dot| &file[dot + 1..])
              + file.rfind('.').map(|dot| &file[dot + 1..])
    = note: covered by library::tests::a_text_file (its 1 covering test still passes)
    = help: use inputs where find and rfind give different results, and assert what extension() does

    done when  mutastic verify m1c9a050a fixtures/operators passes
    reproduce  mutastic run --only=m1c9a050a fixtures/operators
```

## Reverse ordering

`library/reverse-ordering`, in the `stronger` set, and only in Rust. A comparison of two values
turns round, `a.cmp(&b)` becoming `b.cmp(&a)` (and `partial_cmp` too), and a `.reverse()` on an
ordering is left out. It goes unnoticed when no test checks the order something is sorted in.

<!-- from: fixtures/operators/src/library.rs -->
```rust
pub fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort_by(|a, b| a.cmp(b));
    names
}
```

<!-- from: fixtures/operators/src/library.rs -->
```rust
#[test]
fn sorting_keeps_every_name() {
    let names = vec!["cy".to_string(), "al".to_string(), "bo".to_string()];
    assert_eq!(sorted(names).len(), 3);
}
```

<!-- output: operators/library/reverse-ordering -->
```text
  ✗ [MISSED]  tests pass even when a.cmp(b) becomes b.cmp(a) in sorted()
    ┌─ src/library.rs:17
    │
 16 │ pub fn sorted(mut names: Vec<String>) -> Vec<String> {
 17 │     names.sort_by(|a, b| a.cmp(b));
    │                            ^^^ changed to b.cmp(a)
 18 │     names
    │
    = mutant  - names.sort_by(|a, b| a.cmp(b));
              + names.sort_by(|a, b| b.cmp(a));
    = note: covered by library::tests::sorting_keeps_every_name (its 1 covering test still passes)
    = help: use inputs where a.cmp(b) and b.cmp(a) differ, and assert what sorted() does

    done when  mutastic verify m6a7d7d94 fixtures/operators passes
    reproduce  mutastic run --only=m6a7d7d94 fixtures/operators
```

## String constant

`mutastic/string-constant`, in the `stronger` set. A string in the code becomes `""`, and `""`
becomes `"mutastic"`. Not a string given straight to `Err`, `expect`, `context`, `with_context` or
`wrap_err`, which is a message for people; not a returned one, which is return value's; and not one
inside a macro such as `format!`, whose tokens mutastic leaves alone.

<!-- from: fixtures/operators/src/library.rs -->
```rust
pub fn header(columns: &[&str]) -> String {
    columns.join(", ")
}
```

<!-- from: fixtures/operators/src/library.rs -->
```rust
#[test]
fn one_column() {
    assert_eq!(header(&["name"]), "name");
}
```

With one column there's nothing to separate:

<!-- output: operators/library/string-constant -->
```text
  ✗ [MISSED]  tests pass even when ", " becomes "" in header()
    ┌─ src/library.rs:22
    │
 21 │ pub fn header(columns: &[&str]) -> String {
 22 │     columns.join(", ")
    │                  ^^^^ changed to ""
 23 │ }
    │
    = mutant  - columns.join(", ")
              + columns.join("")
    = note: covered by library::tests::one_column (its 1 covering test still passes)
    = help: use inputs where ", " and "" differ, and assert what header() does

    done when  mutastic verify m55b8a89c fixtures/operators passes
    reproduce  mutastic run --only=m55b8a89c fixtures/operators
```

## Regex constant

`mutastic/regex-constant`, in the `extended` set. The pattern given to `Regex::new` becomes ones
that differ and still compile: an anchor dropped, a class turned round. It goes unnoticed when no
test gives the regular expression text it should reject.

The fixtures depend on no crates, so there's no card here from the `regex` crate itself. The truth
fixture has a stand-in `Regex` that mutastic knows by its path, and `mutastic mutants` shows what
it makes:

<!-- from: fixtures/truth/src/lib.rs -->
```rust
let re = Regex::new(r"^\d+$").unwrap();
```

<!-- output: operators/library/regex-constant -->
```text
md8834f6e src/lib.rs:535:25 mutastic/regex-constant r"^\d+$" becomes r"\d+$" in patterns
mf7d51f60 src/lib.rs:535:25 mutastic/regex-constant r"^\d+$" becomes r"^\d+" in patterns
mf617b484 src/lib.rs:535:25 mutastic/regex-constant r"^\d+$" becomes r"^\D+$" in patterns
3 mutants
```

## Remove step

`bigdecimal/remove-step`, in the `extended` set: the Java edition's `BigDecimal` operator, for
`rust_decimal`. A rounding or scaling step is left out: `x.round_dp(2)`, `round_sf`, `normalize()`
and `trunc_with_scale` become `x`. It goes unnoticed when no test uses a value the step would change.

As with regular expressions, the truth fixture has a stand-in `Decimal`, known by its method's
name:

<!-- from: fixtures/truth/src/lib.rs -->
```rust
let price = Decimal(x as i64 * 137).round_dp(1);
```

<!-- output: operators/library/remove-step -->
```text
ma0a93565 src/lib.rs:536:41 bigdecimal/remove-step .round_dp(1) is left out in patterns
1 mutants
```
