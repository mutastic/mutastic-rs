# Operators

An operator is a kind of change; each mutant is one operator applied at one place. mutastic for
Rust uses the Java edition's ids wherever the meaning is the same, so a rule, a script or an agent
that names `mutastic/boundary` means one thing in both editions, and adds a few ids of Rust's own.

A run uses `defaults` unless `--operators` says otherwise: `--operators=stronger` adds the
`stronger` set to the defaults, `--operators=extended` adds both, and a list of ids separated by
commas picks exactly those. Code inside `unsafe`, and safe functions holding an `unsafe` block, get
no mutants unless `--unsafe=enclosing` or `--unsafe=all` asks (see the README).

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
