# Operators

An operator is a kind of change; each mutant is one operator applied at one place. mutastic for
Rust uses the Java edition's ids wherever the meaning is the same, so a rule, a script or an agent
that names `mutastic/boundary` means one thing in both editions, and adds a few ids of Rust's own.

A run uses `defaults` unless `--operators` says otherwise: `--operators=stronger` adds the
`stronger` set to the defaults, `--operators=extended` adds both, and a list of ids separated by
commas picks exactly those. Code inside `unsafe`, and safe functions holding an `unsafe` block, get
no mutants unless `--unsafe=enclosing` or `--unsafe=all` asks (see [Unsafe code](concepts/unsafe.md)).

## By family

Each family has a page with an example of each operator, from a real run on
`fixtures/operators`, and the card it gives:

| Family | Operators |
|---|---|
| [Conditions](operators/conditions.md) | boundary, remove-conditional, logical-connector, comparison, other-comparison |
| [Arithmetic](operators/arithmetic.md) | math, arithmetic-replacement, other-arithmetic, keep-operand, remove-negation, constant-value, bitwise, library/unchecked-arithmetic |
| [Returns and errors](operators/returns.md) | return-value, unwrap-try, replace-body, remove-throw |
| [Calls and writes](operators/calls.md) | remove-void-call, remove-discarded-call, remove-assignment, remove-field-write, remove-struct-field |
| [Arguments](operators/arguments.md) | omit-call, swap-arguments, null-parameter, drop-vararg, default-parameter |
| [std's methods and strings](operators/library.md) | library/swap-string, swap-math, swap-index-of, reverse-ordering; string-constant, regex-constant, bigdecimal/remove-step |
| [Iterators](operators/iterators.md) | stream/swap-match, remove-stage, remove-other-stage |
| [`Option` and `Result`](operators/options.md) | optional/swap-present, empty, always-default, no-default, remove-filter |
| [`match` and loops](operators/control.md) | remove-switch-arm, swap-break |

Every mutant here compiles whenever the code it changes did, or is withdrawn by the build: a site's
mutants and its original code are arms of one `match`, so a change that gives the expression another
type is rejected by the compiler and reported as unviable, never run.

## The table

Status is **implemented** or **deferred**; the deferred ones are listed again below, with the
reason.

| Id | Before → after | Set | Java equivalent | Status |
|---|---|---|---|---|
| `mutastic/boundary` | `a < b` → `a <= b`, `a >= b` → `a > b`; `lo..hi` ↔ `lo..=hi` in a `for`, a slice (`&s[lo..hi]`) or a `contains` | defaults | `mutastic/boundary` | implemented |
| `mutastic/remove-conditional` | `if c`, `while c`, a match guard → `true`, and `false`; `let Some(x) = e` (`let`-`else`, `if let`, `while let`) → `e.filter(\|_\| false)` | defaults | `mutastic/remove-conditional` | implemented |
| `mutastic/logical-connector` | `a && b` ↔ `a \|\| b` | defaults | `mutastic/logical-connector` | implemented |
| `mutastic/math` | `+` ↔ `-`, `*` ↔ `/`, and `+=` ↔ `-=`, `*=` ↔ `/=`; `checked_add` ↔ `checked_sub`, `checked_mul` ↔ `checked_div`, likewise `wrapping_`, `saturating_`, `overflowing_` | defaults | `mutastic/math` | implemented |
| `mutastic/return-value` | a returned value → `0` (or `1`), `0.0` (or `1.0`), the other `bool`, `String::new()`, `""`, `None`, `Vec::new()`, `HashMap::new()` and the other collections, `PathBuf::new()`, `&[]`, `Cow::Borrowed("")`, `Box::new(..)`/`Rc`/`Arc` of the inner value, `Ok(..)` of it (`Ok(())` for `Result<()>`, not in `fmt`) | defaults | `mutastic/return-value` | implemented |
| `mutastic/remove-void-call` | `v.push(x);` → removed: a call the crate declares with no return, or one of std's (`push`, `sort`, `clear`, `extend`, `drop`, …) | defaults | `mutastic/remove-void-call` | implemented |
| `mutastic/remove-discarded-call` | `map.insert(k, v);`, `w.flush()?;`, `let _ = f();` → removed | defaults | `mutastic/remove-discarded-call` | implemented |
| `mutastic/remove-assignment` | `total = total + p;` → `let _ = total + p;` (never a `let x;`'s first value) | defaults | `mutastic/remove-assignment` | implemented |
| `library/swap-string` | `starts_with` ↔ `ends_with`, `to_uppercase` ↔ `to_lowercase` (and ASCII), `trim_start` ↔ `trim_end` (and `_matches`), `strip_prefix` ↔ `strip_suffix` | defaults | `library/swap-string` | implemented |
| `library/swap-math` | `a.min(b)` ↔ `a.max(b)`, `cmp::min` ↔ `cmp::max`, `min_by(_key)` ↔ `max_by(_key)`, `floor` ↔ `ceil` | defaults | `library/swap-math` | implemented |
| `stream/swap-match` | `it.any(p)` ↔ `it.all(p)` | defaults | `stream/swap-match` | implemented |
| `stream/remove-stage` | `.filter(p)`, `.skip(n)`, `.take(n)`, `.skip_while(p)`, `.take_while(p)`, `.step_by(n)`, `.rev()` left out of a chain that visibly starts as an iterator, up to its consumer or the `for` it feeds | defaults | `stream/remove-stage` | implemented |
| `mutastic/comparison` | `<` → `!=`, `<=` → `==`, `>` → `!=`, `>=` → `==`; `==` → `<=`, `>=` and `!=` → `<`, `>` beside a number, a character, a length or a cast | stronger | `mutastic/comparison` | implemented |
| `mutastic/other-comparison` | a comparison → each of the others boundary and comparison don't make (`<` → `>`, `>=`, `==`); `if c` → `if !c`; `matches!(..)` → `!matches!(..)` | stronger | `mutastic/other-comparison` | implemented |
| `mutastic/arithmetic-replacement` | `a + b` → `a * b`, `+=` → `*=` (not `+= 1`) | stronger | `mutastic/arithmetic-replacement` | implemented |
| `mutastic/other-arithmetic` | each of `+ - * / %` → the others math and arithmetic-replacement don't make: `+` → `/`, `%`; `-` → `*`, `/`, `%`; `*` and `/` → `+`, `-`, `%`; `%` → `+`, `-`, `*`, `/`; and compound forms | stronger | `mutastic/other-arithmetic` | implemented |
| `mutastic/keep-operand` | `price + fee` → `price`, and → `fee` (the other still worked out when it could do something; never the constant beside a value) | stronger | `mutastic/keep-operand` | implemented |
| `mutastic/omit-call` | `s.trim()` → `s`, `x.abs()` → `x` (and `to_lowercase`, `round`, …), a builder's `(self) -> Self` step left out, `escape(text)` → `text` for a crate function returning its one argument's type | stronger | `mutastic/omit-call` | implemented |
| `mutastic/swap-arguments` | `transfer(a, from, to)` → `transfer(a, to, from)`, for a crate function or method whose last two parameters have one type | stronger | `mutastic/swap-arguments` | implemented |
| `library/swap-index-of` | `find` ↔ `rfind`, `position` ↔ `rposition`, `split_once` ↔ `rsplit_once`, `match_indices` ↔ `rmatch_indices` | stronger | `library/swap-index-of` | implemented |
| `library/reverse-ordering` | `a.cmp(&b)` → `b.cmp(&a)` (and `partial_cmp`); `.reverse()` on an ordering left out | stronger | Rust only | implemented |
| `mutastic/remove-switch-arm` | `P => e` stops matching (a guard the switch decides), so its values go to the next arm that matches; only before a later unguarded catch-all | stronger | `mutastic/remove-switch-arm` (extended there) | implemented |
| `mutastic/unwrap-try` | `f()?` → `f().unwrap()` | stronger | Rust only | implemented |
| `mutastic/remove-field-write` | `self.status = s;`, `*p = v;`, `v[i] = x;` → `let _ = s;` | stronger | `mutastic/remove-field-write` | implemented |
| `mutastic/remove-negation` | `-x` → `x` | stronger | `mutastic/remove-negation` | implemented |
| `mutastic/constant-value` | a whole number → the ones next to it, `0`, `1`, and `-1` where it's visibly signed; never past its type's range, never a size, capacity, wait, or a counter's first `0` or `1` | stronger | `mutastic/constant-value` | implemented |
| `mutastic/string-constant` | `"csv"` → `""`, `""` → `"mutastic"`; not an error's or `expect`'s message | stronger | `mutastic/string-constant` | implemented |
| `mutastic/null-parameter` | `fn f(c: Option<C>)` → `let c: Option<C> = None;` as the function starts | stronger | `mutastic/null-parameter` | implemented |
| `optional/swap-present` | `is_some()` ↔ `is_none()`, `is_ok()` ↔ `is_err()` | stronger | `optional/swap-present` (extended there) | implemented |
| `optional/empty` | `Some(x)` → `None` (where it isn't returned) | stronger | `optional/empty` (extended there) | implemented |
| `optional/always-default` | `o.unwrap_or(d)` → `d`, `unwrap_or_default()` → `Default::default()`, `unwrap_or_else(f)` → `f()`, `map_or(d, f)` → `d` | stronger | `optional/always-default` (extended there) | implemented |
| `optional/no-default` | `o.unwrap_or(d)`, `unwrap_or_else(f)`, `unwrap_or_default()` → `o.unwrap()` | stronger | `optional/no-default` (extended there) | implemented |
| `library/unchecked-arithmetic` | `a.checked_add(b)` → `Some(a.wrapping_add(b))`; `saturating_sub` → `wrapping_sub` | stronger | `library/unchecked-arithmetic` | implemented |
| `mutastic/bitwise` | `&` → `\|`, `^`; `\|` → `&`, `^`; `^` → `&`, `\|`; `<<` ↔ `>>`; compound forms, but `&=` and `\|=` not to `^=` | stronger | Rust only | implemented |
| `mutastic/remove-struct-field` | `S { a, b, ..base }` → `S { a, ..base }` | stronger | Rust only | implemented |
| `mutastic/replace-body` | a function's whole body → `{ <a value return-value would give> }`, or `{}` when it returns nothing; not `new`, `fmt`, `hash` or a getter of one field | extended | `mutastic/replace-body` | implemented |
| `mutastic/return-value` (extended forms) | a return → `Default::default()` for a type that visibly derives or implements `Default`; and → an `Err(..)` the same function returns elsewhere, when it mentions nothing local | extended | `mutastic/return-value` | implemented |
| `mutastic/regex-constant` | `Regex::new(r"^\d+$")` → `r"\d+$"`, `r"^\d+"`, `r"^\D+$"`: an anchor dropped, a class turned round, each still valid | extended | `mutastic/regex-constant` | implemented |
| `mutastic/drop-vararg` | `f(&[a, b, c])` → `f(&[a, b])`, `f(vec![a, b, c])` → `f(vec![a, b])` | extended | `mutastic/drop-vararg` (stronger there) | implemented |
| `stream/remove-other-stage` | `.inspect(f)` and `.map(f)` left out of an iterator's chain, as stream/remove-stage's stages are | extended | `stream/remove-other-stage` | implemented |
| `optional/remove-filter` | `opt.filter(p)` → `opt`, where `opt` isn't visibly an iterator, nor used as one (looped over, a closure over pairs) | extended | `optional/remove-filter` | implemented |
| `mutastic/default-parameter` | `fn f(p: T)` → `let p: T = Default::default();` as the function starts, for a type that visibly has a default (std's numbers, `bool`, `char`, `String`, collections, or one the crate derives or implements it for); an `Option` is null-parameter's | extended | Rust only | implemented |
| `mutastic/swap-break` | `break` ↔ `continue`, with no label and no value | extended | Rust only | implemented |
| `bigdecimal/remove-step` | `x.round_dp(2)`, `round_sf`, `normalize()`, `trunc_with_scale` → `x` | extended | `bigdecimal/remove-step` (a plugin there) | implemented |
| `mutastic/remove-throw` | `return Err(e);`, `panic!(..);`, `bail!(..);` removed: the code carries on; not the last statement of a block whose value is used | extended | `mutastic/remove-throw` | implemented |
| `mutastic/flip-increment` | — | — | `mutastic/flip-increment` | deferred: Rust has no `++`, and `+=` ↔ `-=` is `mutastic/math`'s; the id stays reserved so the change has one owner |
| `library/omit-argument` | — | — | `library/omit-argument` | deferred: needs overloading, which Rust doesn't have |
| `predicate/swap-connector`, `predicate/remove-negate`, `predicate/remove-not` | — | — | the same ids | deferred: std has no `Predicate` combinators; `&&` and `!` inside closures are logical-connector's and other-comparison's |
| `mutastic/drop-catch-type` | — | — | `mutastic/drop-catch-type` | deferred: Rust has no multi-catch |
| `concurrency/weaken-ordering` | `SeqCst` → `Acquire`/`Release` → `Relaxed`, legal per method | experimental | Rust only | deferred: almost never changes what a test sees on x86-64 or Apple Silicon, and `load(Release)` panics, so it needs loom or Miri to mean anything; never scored |
| `clone` removal | `x.clone()` → `x` | experimental | Rust only | deferred: mostly fails the borrow checker, and where it compiles it's usually equivalent; evidence about performance, not wrong output |
| ghosts | the missing peer call inserted after a write that lacks it (`self.dirty = true`, `notify_all()`) | experimental | ghosts (experimental there too) | deferred: not built in the Java edition yet either; to share its design |
| `proposed/rewrite` | a function's body → a proposed body, behind its switch | extended | `proposed/rewrite` | deferred: waits on the proposals format and a measured run on real model output, as in the Java edition |
| `mutastic/narrow-cast` | `x as u32` → `x as u16 as u32` | experimental | Rust only | deferred: no tool or bug count behind it yet |
| unsafe-specific operators | `ptr.add(i)`, `set_len(n)`, `Layout` sizes off by one | experimental | Rust only | deferred: their mutants are undefined behaviour by design, so they mean something only with a Miri pass, which needs nightly |

`mutastic/return-value(extended)` in a set turns on return-value's extended forms; it isn't an
operator of its own, and its mutants are `mutastic/return-value`'s.

## Parity with the Java edition

The Java edition has 43 operator ids. **37 carry over** with the same meaning, all implemented;
**6 don't**, for want of the language feature they change. mutastic for Rust adds **6 ids of its
own**, and implements one of the Java edition's plugin operators.

- **Carried over (37):** `mutastic/boundary`, `comparison`, `other-comparison`,
  `remove-conditional`, `logical-connector`, `math`, `arithmetic-replacement`, `other-arithmetic`,
  `keep-operand`, `constant-value`, `remove-negation`, `return-value`, `remove-void-call`,
  `remove-discarded-call`, `remove-assignment`, `remove-field-write`, `omit-call`,
  `swap-arguments`, `null-parameter`, `drop-vararg`, `remove-switch-arm`, `replace-body`,
  `remove-throw`, `regex-constant`, `string-constant`; `library/unchecked-arithmetic`,
  `swap-string`, `swap-index-of`, `swap-math`; `stream/remove-stage`, `swap-match`,
  `remove-other-stage`; `optional/swap-present`, `empty`, `remove-filter`, `always-default`,
  `no-default`.
- **Not carried over (6):** `mutastic/flip-increment` (no `++`; `+=` and `-=` are math's),
  `library/omit-argument` (no overloading), the three `predicate/*` ids (no `Predicate`
  combinators in std), `mutastic/drop-catch-type` (no multi-catch).
- **Rust only (6):** `mutastic/bitwise`, `mutastic/remove-struct-field`,
  `library/reverse-ordering`, `mutastic/unwrap-try`, `mutastic/default-parameter`,
  `mutastic/swap-break`.
- **From the Java edition's plugins:** `bigdecimal/remove-step`, for `rust_decimal`. The others
  don't carry: `bigdecimal/swap-arithmetic` and `swap-scaling` come free through math and
  swap-math, as `Decimal` uses operators; `kotlin/empty-return`'s empty values are Rust's own, and
  return-value's; `annotations/*` can't sit behind a run-time switch, as attributes act at compile
  time.

Some ids sit in another set than in Java: the four `optional/*` ids and `remove-switch-arm` are in
`stronger`, because `Option`, `Result` and `match` are core Rust, not a modern API; `drop-vararg`
is in `extended`, as an array's length is part of its type, and more of its mutants are withdrawn.
