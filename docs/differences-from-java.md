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
- **Only in Rust:** `mutastic/bitwise`, `mutastic/remove-struct-field`, `library/reverse-ordering`,
  `mutastic/unwrap-try`, `mutastic/default-parameter` and `mutastic/swap-break`. Of these,
  `mutastic/unwrap-try` is in the defaults, so a Rust run's defaults are the Java edition's and one
  more.

[Operators](operators.md) has the whole table.

## Commands

The commands and their output are the Java edition's: the summary, the `Not covered` list, the
counter while mutants run, the report files, `explain`, `ignore` and `--since`. Where Rust differs:

- **`explain`** shows the change as source, `total >= 100 → (total) > (100)`, where the Java edition
  shows bytecode, and its `last run` comes from the last full run's `mutation.json`, as there's no
  run record yet.
- **Report files** are `mutation.json` and `mutation.html` in `target/mutastic`; there's no
  `mutations.xml`, whose consumers key on JVM mutator names.
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

## Not in the Rust edition yet

These Java edition features have no Rust counterpart yet. Where you'd use one, here's what to do
instead:

| In the Java edition | In the Rust edition, for now |
|---|---|
| Compiled plugins, and `filters:` in `mutastic.yaml` | A rules file, for call operators: [Configuration](reference/configuration.md#rules-files) |
| `@DoNotMutate` | `mutastic ignore`, one mutant at a time: [Commands](reference/cli.md#mutastic-ignore) |
| Suggested tests on cards | The card's `help` line |
| `--vocabulary=classic` | The standard words only |
