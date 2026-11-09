# Operators

An operator is a kind of change; each mutant is one operator applied at one place. mutastic for
Rust uses the Java edition's ids wherever the meaning is the same, so a rule, a script or an agent
that names `mutastic/boundary` means one thing in both editions, and adds a few ids of Rust's own.

A run uses `defaults` unless `--operators` says otherwise: `--operators=stronger` adds the
`stronger` set to the defaults, `--operators=extended` adds both, and a list of ids separated by
commas picks exactly those. Code inside `unsafe`, and safe functions holding an `unsafe` block, get
no mutants unless `--unsafe=enclosing` or `--unsafe=all` asks (see [Unsafe code](concepts/unsafe.md)).

Each operator has its place on a family's page: what it changes, the bug it stands for, an example
from `fixtures/operators` with the card mutastic prints for it, and the test that catches it. The
families are the Java edition's, with the Java edition's names wherever the language allows:

- [Conditions](operators/conditions.md): comparisons, conditions, and how two combine.
- [Arithmetic](operators/arithmetic.md): operators, numbers and bits.
- [Returns and errors](operators/returns.md): what a function returns, and the error it gives.
- [Calls and writes](operators/calls.md): calls, assignments and writes left out.
- [Arguments](operators/arguments.md): what a call is given, and what a function gets.
- [Library methods](operators/library.md): std's methods swapped for their counterparts, and the
  text in your code.
- [Iterators](operators/iterators.md): iterator stages and consumers, where the Java edition has
  streams.
- [`Option` and `Result`](operators/options.md): how they're asked about and unwrapped, where the
  Java edition has `Optional`.
- [Control flow](operators/control.md): `match` arms and loops.

## Every operator

| Operator | What it changes | Set | Family | In Java |
|---|---|---|---|---|
| [`mutastic/boundary`](operators/conditions.md#boundary) | `<` and `<=`, `>` and `>=`; `lo..hi` and `lo..=hi` | defaults | [Conditions](operators/conditions.md) | the same |
| [`mutastic/remove-conditional`](operators/conditions.md#remove-conditional) | a condition to always `true`, and to always `false` | defaults | [Conditions](operators/conditions.md) | the same |
| [`mutastic/logical-connector`](operators/conditions.md#logical-connector) | `&&` and `\|\|` | defaults | [Conditions](operators/conditions.md) | the same |
| [`mutastic/comparison`](operators/conditions.md#comparison) | a comparison to the one that differs on one side: `<` to `!=` | stronger | [Conditions](operators/conditions.md) | the same |
| [`mutastic/other-comparison`](operators/conditions.md#other-comparison) | a comparison to the others; a condition to its opposite | stronger | [Conditions](operators/conditions.md) | the same |
| [`mutastic/math`](operators/arithmetic.md#math) | `+` and `-`, `*` and `/`, and their `checked_`, `wrapping_`, `saturating_` and `overflowing_` methods | defaults | [Arithmetic](operators/arithmetic.md) | the same |
| [`mutastic/arithmetic-replacement`](operators/arithmetic.md#arithmetic-replacement) | `+` to `*` | stronger | [Arithmetic](operators/arithmetic.md) | the same |
| [`mutastic/other-arithmetic`](operators/arithmetic.md#other-arithmetic) | an arithmetic operator to each of the others | stronger | [Arithmetic](operators/arithmetic.md) | the same |
| [`mutastic/keep-operand`](operators/arithmetic.md#keep-operand) | `a + b` to `a`, and to `b` | stronger | [Arithmetic](operators/arithmetic.md) | the same |
| [`mutastic/remove-negation`](operators/arithmetic.md#remove-negation) | `-x` to `x` | stronger | [Arithmetic](operators/arithmetic.md) | the same |
| [`mutastic/constant-value`](operators/arithmetic.md#constant-value) | a number to the ones next to it, `0`, `1` and `-1` | stronger | [Arithmetic](operators/arithmetic.md) | the same |
| [`mutastic/bitwise`](operators/arithmetic.md#bitwise) | `&`, `\|` and `^`; `<<` and `>>` | stronger | [Arithmetic](operators/arithmetic.md) | the same |
| [`library/unchecked-arithmetic`](operators/arithmetic.md#unchecked-arithmetic) | `checked_add` to a `wrapping_add` that can't fail | stronger | [Arithmetic](operators/arithmetic.md) | the same |
| [`mutastic/return-value`](operators/returns.md#return-value) | a returned value to another of its type: `0`, the other `bool`, `None`, an empty one | defaults | [Returns and errors](operators/returns.md) | the same |
| [`mutastic/unwrap-try`](operators/returns.md#unwrap-try) | `f()?` to `f().unwrap()` | defaults | [Returns and errors](operators/returns.md) | — |
| [`mutastic/replace-body`](operators/returns.md#replace-body) | a function's whole body to one value, or to nothing | extended | [Returns and errors](operators/returns.md) | the same |
| [`mutastic/remove-throw`](operators/returns.md#remove-throw) | `return Err(e);` or `panic!(..);` removed, so the code carries on | extended | [Returns and errors](operators/returns.md) | the same |
| [`mutastic/remove-void-call`](operators/calls.md#remove-void-call) | a call that returns nothing, removed | defaults | [Calls and writes](operators/calls.md) | the same |
| [`mutastic/remove-discarded-call`](operators/calls.md#remove-discarded-call) | a call whose result is thrown away, removed | defaults | [Calls and writes](operators/calls.md) | the same |
| [`mutastic/remove-assignment`](operators/calls.md#remove-assignment) | an assignment to a variable that already has a value, removed | defaults | [Calls and writes](operators/calls.md) | the same |
| [`mutastic/remove-field-write`](operators/calls.md#remove-field-write) | a write to a field, through a reference, or to an element, removed | stronger | [Calls and writes](operators/calls.md) | the same |
| [`mutastic/remove-struct-field`](operators/calls.md#remove-struct-field) | a field set in a struct literal with `..base`, left to `base` | stronger | [Calls and writes](operators/calls.md) | — |
| [`mutastic/omit-call`](operators/arguments.md#omit-call) | a call that returns what it's given, left out: `s.trim()` to `s` | stronger | [Arguments](operators/arguments.md) | the same |
| [`mutastic/swap-arguments`](operators/arguments.md#swap-arguments) | a call's last two arguments of one type, swapped | stronger | [Arguments](operators/arguments.md) | the same |
| [`mutastic/null-parameter`](operators/arguments.md#null-parameter) | an `Option` parameter, `None` from the start of the function | stronger | [Arguments](operators/arguments.md) | the same |
| [`mutastic/drop-vararg`](operators/arguments.md#drop-vararg) | the last value of an array or `vec!` passed to a call, left out | extended | [Arguments](operators/arguments.md) | the same, in `stronger` |
| [`mutastic/default-parameter`](operators/arguments.md#default-parameter) | a parameter, its type's default from the start of the function | extended | [Arguments](operators/arguments.md) | — |
| [`library/swap-string`](operators/library.md#swap-string) | `starts_with` and `ends_with`, `to_uppercase` and `to_lowercase`, `trim_start` and `trim_end`, `strip_prefix` and `strip_suffix` | defaults | [Library methods](operators/library.md) | the same |
| [`library/swap-math`](operators/library.md#swap-math) | `min` and `max`, `floor` and `ceil` | defaults | [Library methods](operators/library.md) | the same |
| [`library/swap-index-of`](operators/library.md#swap-index-of) | `find` and `rfind`, `position` and `rposition`, `split_once` and `rsplit_once` | stronger | [Library methods](operators/library.md) | the same |
| [`library/reverse-ordering`](operators/library.md#reverse-ordering) | `a.cmp(&b)` to `b.cmp(&a)`; `.reverse()` left out | stronger | [Library methods](operators/library.md) | — |
| [`mutastic/string-constant`](operators/library.md#string-constant) | a string to `""`, and `""` to `"mutastic"` | stronger | [Library methods](operators/library.md) | the same |
| [`mutastic/regex-constant`](operators/library.md#regex-constant) | a regular expression to one that differs and still compiles | extended | [Library methods](operators/library.md) | the same |
| [`bigdecimal/remove-step`](operators/library.md#remove-step) | `round_dp(2)` and the like, left out | extended | [Library methods](operators/library.md) | the same, in a plugin |
| [`stream/swap-match`](operators/iterators.md#swap-match) | `any` and `all` | defaults | [Iterators](operators/iterators.md) | the same |
| [`stream/remove-stage`](operators/iterators.md#remove-stage) | `.filter(p)`, `.skip(n)`, `.take(n)`, `.skip_while(p)`, `.take_while(p)`, `.step_by(n)` or `.rev()`, left out | defaults | [Iterators](operators/iterators.md) | the same |
| [`stream/remove-other-stage`](operators/iterators.md#remove-other-stage) | `.inspect(f)` or `.map(f)`, left out | extended | [Iterators](operators/iterators.md) | the same |
| [`optional/swap-present`](operators/options.md#swap-present) | `is_some` and `is_none`, `is_ok` and `is_err` | stronger | [`Option` and `Result`](operators/options.md) | the same, in `extended` |
| [`optional/empty`](operators/options.md#empty) | `Some(x)` to `None` | stronger | [`Option` and `Result`](operators/options.md) | the same, in `extended` |
| [`optional/always-default`](operators/options.md#always-default) | `o.unwrap_or(d)` to `d` | stronger | [`Option` and `Result`](operators/options.md) | the same, in `extended` |
| [`optional/no-default`](operators/options.md#no-default) | `o.unwrap_or(d)` to `o.unwrap()` | stronger | [`Option` and `Result`](operators/options.md) | the same, in `extended` |
| [`optional/remove-filter`](operators/options.md#remove-filter) | `o.filter(p)` to `o` | extended | [`Option` and `Result`](operators/options.md) | the same |
| [`mutastic/remove-switch-arm`](operators/control.md#remove-switch-arm) | a `match` arm stops matching, so its values go to a later arm | stronger | [Control flow](operators/control.md) | the same, in `extended` |
| [`mutastic/swap-break`](operators/control.md#swap-break) | `break` and `continue` | extended | [Control flow](operators/control.md) | — |

`mutastic/return-value(extended)` in a set turns on return value's extended forms, a return of
`Default::default()` for a type that visibly has one, and an `Err(..)` the function returns
elsewhere; it isn't an operator of its own, and its mutants are `mutastic/return-value`'s.

Every mutant here compiles whenever the code it changes did, or is withdrawn by the build: a site's
mutants and its original code are arms of one `match`, so a change that gives the expression another
type is rejected by the compiler and reported as unviable, never run.

## Parity with the Java edition

The Java edition has 44 operator ids. **38 carry over** with the same meaning, all implemented;
**6 don't**, for want of the language feature they change. mutastic for Rust adds **5 ids of its
own**, each in the family it fits, and implements one of the Java edition's plugin operators.

- **Not carried over (6):** `mutastic/flip-increment` (no `++`; `+=` and `-=` are math's),
  `library/omit-argument` (no overloading), the three `predicate/*` ids (no `Predicate`
  combinators in std; `&&` and `!` inside a closure are logical connector's and other
  comparison's), `mutastic/drop-catch-type` (no multi-catch).
- **Rust only (5):** `mutastic/unwrap-try`, `mutastic/remove-struct-field`,
  `mutastic/default-parameter`, `library/reverse-ordering`, `mutastic/swap-break`.
- **From the Java edition's plugins:** `bigdecimal/remove-step`, for `rust_decimal`. The others
  don't carry: `bigdecimal/swap-arithmetic` and `swap-scaling` come free through math and
  swap-math, as `Decimal` uses operators; `kotlin/empty-return`'s empty values are Rust's own, and
  return-value's; `annotations/*` can't sit behind a run-time switch, as attributes act at compile
  time.

Some ids sit in another set than in Java: the four `optional/*` ids and `remove-switch-arm` are in
`stronger`, because `Option`, `Result` and `match` are core Rust, not a modern API; `drop-vararg`
is in `extended`, as an array's length is part of its type, and more of its mutants are withdrawn.
`mutastic/unwrap-try`, Rust's own, is in the defaults: a `?` whose error no test makes happen is an
error path never run, and on real crates most of its missed mutants are just that.

Not built yet, each for its reason:

- `concurrency/weaken-ordering` (`SeqCst` to `Acquire`/`Release` to `Relaxed`): it almost never
  changes what a test sees on x86-64 or Apple Silicon, and `load(Release)` panics, so it needs loom
  or Miri to mean anything; it would never be scored.
- Removing `x.clone()`: it mostly fails the borrow checker, and where it compiles it's usually
  equivalent; evidence about performance, not wrong output.
- Ghosts, the missing peer call inserted after a write that lacks it: not built in the Java edition
  either, so the two can share its design.
- `proposed/rewrite`, a function's body to a proposed one: waits on the proposals format and a
  measured run on real model output, as in the Java edition.
- `mutastic/narrow-cast` (`x as u32` to `x as u16 as u32`): no tool or bug count behind it yet.
- Unsafe-specific operators (`ptr.add(i)`, `set_len(n)`, `Layout` sizes off by one): their mutants
  are undefined behaviour by design, so they mean something only with a Miri pass, which needs
  nightly.
