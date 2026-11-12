# Differences from the Java edition

The two editions share their operators' ids, their cards' words, the event stream and the way they
score. This page is only where you'd do something different in Rust. For the rest, the
[Java edition's docs](https://docs.mutastic.io) hold.

## Installing and running

- **Install from a release or from source:** nothing is on crates.io yet. See
  [Installing mutastic](install.md).
- **No build plugin.** There's no Maven or Gradle plugin to add: run `mutastic run` in a package, or
  `cargo mutastic run`.
- **Your own toolchain.** mutastic builds with the `cargo` and `rustc` you have, stable or not, from
  Rust 1.79 on, as the code it adds to your crate needs, and needs no JDK.

## One package at a time

The Java edition runs a whole multi-module build, and `--tests-from` brings in other modules'
tests. The Rust edition runs one Cargo package, with that package's own tests: point it at a
workspace member's directory, and a test in another member doesn't count for it, even if it calls
the code. See [Troubleshooting](troubleshooting.md#a-workspace).

## Tests

- **What runs:** unit tests in `#[cfg(test)]` modules, integration tests in `tests/`, and a binary's
  tests, through libtest. Doc tests don't run, so code only a doc test calls is not covered. A test
  marked `#[ignore]` is left out, as JUnit's `@Disabled` is.
- **Test names** are libtest's, `tests::small_orders_pay_full_price` for a unit test, and the
  target's name first for an integration test, `pricing::big_orders_get_ten_percent_off` for one in
  `tests/pricing.rs`, where the Java edition writes `PricingTest#smallOrdersPayFullPrice`.
- **A test that fails with no mutant in** runs again on its own, in both: one that then passes is
  flaky, left out of the run and reported, and one that fails every time stops the run. What a
  flaky test reaches that varies is read from source: calls such as `SystemTime::now`,
  `Instant::now` or `rand::thread_rng`, and fields of a type like `RandomState` that a method reads
  through `self`. A Rust test fails with a panic, so there's no failure chain, and none of the Java
  edition's advice for a test that failed on the network or a file.

## Cards

The Rust edition's cards have the gap, the change, the inputs seen, the covering tests, the
suggested test and the commands, as the Java edition's do. The suggested test is a `#[test] fn` for
the module's `mod tests`, asserted with `assert_eq!` or `assert!`, and it's written for functions
and methods a test can call with whole numbers alone: see
[Suggested tests](concepts/suggested-tests.md). Code no test reaches gets no starting point yet.

## What's different in the results

- **Unviable mutants.** Java changes bytecode, so every mutant loads. Rust changes source, and a
  few operators make changes the compiler then rejects. Those are skipped as `unviable:`, with the
  compiler's error, and left out of the score. See
  [How a run works](concepts/how-a-run-works.md#a-mutant-that-doesnt-compile-is-rolled-back).
- **Crashes.** A test process can die in Rust without a test failing. In safe code that counts as
  caught, with no test named; near unsafe code, it's a `RuntimeError`, out of the score. Java has
  neither. See [Results](concepts/results.md#caught-only-because-the-test-process-crashed).
- **Unsafe code** is left alone by default, and code near it is marked and run one test at a
  time. See [Unsafe code](concepts/unsafe.md).
- **Overflow checks and debug assertions** are always on in mutastic's build, whatever your
  profiles say. See [Profiles](concepts/profiles.md).

## Operators

The ids mean the same change in both editions, but a few sit in other sets, and some don't exist in
one of them:

- **In `stronger` here, `extended` in Java:** the four `optional/*` ids and
  `mutastic/remove-switch-arm`, because `Option`, `Result` and `match` are core Rust.
- **In `extended` here, `stronger` in Java:** `mutastic/drop-vararg`, as an array's length is part of
  its type.
- **Not in Rust:** `mutastic/flip-increment` (no `++`), `library/omit-argument` (no overloading),
  the three `predicate/*` ids, and `mutastic/drop-catch-type` (no multi-catch).
- **Only in Rust:** `mutastic/remove-struct-field`, `library/reverse-ordering`,
  `mutastic/unwrap-try`, `mutastic/default-parameter` and `mutastic/swap-break`. Of these,
  `mutastic/unwrap-try` is in the defaults, so a Rust run's defaults are the Java edition's and one
  more.

[Operators](operators.md) has the whole table.

## Commands

The commands and their output are the Java edition's: the summary, the `Not covered` list, the
counter while mutants run, the report files, `explain`, `ignore` and `--since`. Where Rust differs:

- **`explain`** shows the change as source, `total >= 100 → (total) > (100)`, where the Java edition
  shows bytecode, and its `last run` comes from the `mutation.json` of the last run of every
  mutant, by `run` or `next`.
- **`next`** queues the misses as the Java edition does, in the same words and events, but with no
  `mutastic baseline` yet: so no new misses come first, `rank` has no `baseline` factor, and
  `next-finished`'s `legacy` is always 0. `--top` takes at most one item a function, where Java
  says a method. Whether the code under test is the same, so that only what the last `next` left
  uncaught runs, is a hash of the files mutastic mutates with their test code (`#[test]` functions
  and `#[cfg(test)]` modules) taken out, as Java hashes only the classes under test. See
  [Working through misses](concepts/next.md).
- **A function only the crate calls** (not `pub`) gets no suggested test on an input none of its
  callers passed: its card says no caller passed the value, and to test a caller that can, or
  propose the mutant unreachable. A test of it alone on that value could test what can't happen.
- **A run compared with the last** works as in the Java edition. A mutant that crashed where a
  crash proves nothing is out of the score, as a skipped one is, so a change to or from one isn't
  counted against the test that caught it.
- **Report files** are `mutation.json` and `mutation.html` in `target/mutastic`; there's no
  `mutations.xml`, whose consumers key on JVM mutator names.
- **One run across CI jobs,** `--shard` and `--from-shards`, works as in the Java edition: the same
  deal by id, the same folder for each share, the same refusals, and the whole run put back
  together. The one difference follows from `mutation.json` being the last run's record here: a
  share writes its report files in its own folder, `target/mutastic/shares/N-of-M/`, beside its
  events, so `explain` never reads a share as the whole run. Each share makes the one build. See
  [One run across CI jobs](concepts/how-a-run-works.md#one-run-across-ci-jobs).
- **`mutastic.yaml` and rules files** are the Java edition's, with Rust paths for classes
  (`shop::generated::**`) and Rust types as owners. A rules file's operator matches a call by its
  method's name, since mutastic reads source without types; a rewrite that doesn't fit doesn't
  compile, and is withdrawn. See [Configuration](reference/configuration.md).
- **The gate** is the Java edition's, so the coding agents' plugins run it unchanged. A test it
  counts is a `#[test]`-like function in `src` or `tests`, and one turned off is `#[ignore]`, where
  Java counts `@Test` and `@Disabled`.
- **Skipped as arid:** a statement that only writes output, `println!`, `eprintln!`, `dbg!` or a
  logger's macro, where the Java edition skips a call to a logger or `System.out`.
- **Skipped as equivalent:** a `?` that only passes on a formatter's error, inside the `fmt` of
  `Display` and the other formatting traits, or on a write to a `Formatter`: no test can make that
  error happen.
  Also a change to a zero check on a `NonZero*` value that differs only at 0: `parts.get() == 0`
  made never true, where `parts` is a `NonZeroU32`, which can't hold 0. And a change to a zero
  check on an unsigned value that differs only below 0, `n == 0` made `n <= 0`, where `n`'s type
  is written `usize` or it's cast to one; the Java edition takes lengths and counts from the JDK.
  Both editions skip a boundary inside a guard that its sides differ, `a < b` made `<=` inside
  `if a != b`; Rust also takes a field of a binding that can't change and the `x` of a `Some(x)`
  (`ver.major > cmp.major` inside `if ver.major != cmp.major`), where Java takes only `int` and
  `long` locals.

## Not in the Rust edition yet

These Java edition features have no Rust counterpart yet. Where you'd use one, here's what to do
instead:

| In the Java edition | In the Rust edition, for now |
|---|---|
| Compiled plugins, and `filters:` in `mutastic.yaml` | A rules file, for call operators: [Configuration](reference/configuration.md#rules-files) |
| `@DoNotMutate` | `mutastic ignore`, one mutant at a time: [Configuration](reference/configuration.md#ignoring-one-mutant) |
| A suggested starting point for code no test reaches | The `Not covered` list, which names each function and its lines |
