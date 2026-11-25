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
- **A case is a test.** Each rstest `#[case]`, test-case row and `#[values]` cell is a test of its
  own, as libtest lists it, `rstest_tests::discounts::case_2_at_the_edge`, where the Java edition
  counts a parameterized method as one test. Where a case catches a mutant, both name it after the
  test, `caughtCase` in the events: here it's the row as written, `#[case(30, 0, 30)]`, where the
  Java edition gives JUnit's display name, `[2] 2, 3, 5`, after the method. See [Tables of cases
  and property tests](troubleshooting.md#tables-of-cases-and-property-tests).
- **Property tests** run seeded where the framework takes a seed from outside, proptest's
  `PROPTEST_RNG_SEED`, with shrinking off on a mutant's run, and a catch names the input it failed
  on, `caughtInput` in the events and `mutation.json`, and `run-started`'s `propertyTests`. The
  Java edition doesn't have them yet. quickcheck, arbtest and bolero can't be seeded from outside,
  and a run says so. See [Tables of cases and property
  tests](troubleshooting.md#tables-of-cases-and-property-tests).
- **A test that fails with no mutant in** runs again on its own, in both: one that then passes is
  flaky, left out of the run and reported, and one that fails every time stops the run. What a
  flaky test reaches that varies is read from source: calls such as `SystemTime::now`,
  `Instant::now` or `rand::thread_rng`, and fields of a type like `RandomState` that a method reads
  through `self`. A Rust test fails with a panic, so there's no failure chain, and none of the Java
  edition's advice for a test that failed on the network or a file.
- **A test that caught mutants and fails on its own** is found as in the Java edition, once the
  mutants have run, and the run doesn't pass. The coverage pass already runs each test in a process
  of its own, so a Rust test that needs another test to set it up stops the run there, before any
  mutant; what's left to find afterwards is what the mutants' runs left behind, such as a file. The
  run exits with status 3, as for a test that fails with no mutant in, as in the Java edition.
- **`--exclude-tests=TEST`** takes a test by its name as a run shows it, or a module of tests by
  its path, which leaves out every test in it, where the Java edition takes a test class by its
  name or full name, one method of it, or one overload by its parameter types. `mutastic.yaml`'s
  `exclude-tests:` takes the same names. The stop for a test that fails with no mutant in offers
  it with the test's name. A name that matches no test, run or ignored, leaves nothing out, and
  both editions say so the same way: `! left out nothing: tests::big_order names no test of
  pricing. There's one called tests::big_orders.`
- **`--confirm-catches` and `--kill-matrix`** work on test binaries. A binary's covering tests run
  together against a mutant, so every one of them that fails is known at once: confirming runs
  each of those again on its own until one repeats, and the kill matrix lists them in the order
  they're listed, then runs the binaries after. The Java edition runs tests one after another and
  stops at the first failure, so it runs the rest again each time.

## Cards

The Rust edition's cards have the gap, the change, the inputs seen, the covering tests, the
suggested test and the commands, as the Java edition's do. The suggested test is a `#[test] fn` for
the module's `mod tests`, asserted with `assert_eq!` or `assert!`, and it's written for functions
and methods a test can call with whole numbers alone: see
[Suggested tests](concepts/suggested-tests.md). Code no test reaches gets a starting point as the
Java edition's does, `try:` under `Not covered` and `start with:` in the agent format: an
`assert_eq!` or `assert!` line for that test module, for a function code outside the crate can
call. The crate's own gets none, as its inputs are its callers' to give.

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
- **Time limits.** A mutant's tests may take five times each test's own time and a second, side by
  side, under a ceiling `timeout-ceiling:` in `mutastic.yaml` sets; the Java edition doesn't read
  `timeout-ceiling:` yet. See
  [How long a mutant's tests may run](troubleshooting.md#how-long-a-mutants-tests-may-run).

## Operators

The ids mean the same change in both editions, but a few sit in other sets, and some don't exist in
one of them:

- **In `stronger` here, `extended` in Java:** the four `optional/*` ids and
  `mutastic/remove-switch-arm`, because `Option`, `Result` and `match` are core Rust.
- **In `extended` here, `stronger` in Java:** `mutastic/drop-vararg`, as an array's length is part of
  its type.
- **Not in Rust:** `mutastic/flip-increment` (no `++`), `library/omit-argument` (no overloading),
  the three `predicate/*` ids, and `mutastic/drop-catch-type` (no multi-catch).
- **Only in Rust:** `mutastic/remove-struct-field`, `mutastic/unwrap-try`,
  `mutastic/default-parameter` and `mutastic/swap-break`. Of these,
  `mutastic/unwrap-try` is in the defaults, so a Rust run's defaults are the Java edition's and one
  more.

[Operators](operators.md) has the whole table.

## Commands

The commands and their output are the Java edition's: the summary, the `Not covered` list, the
counter while mutants run, the report files, `explain`, `ignore` and `--since`. Where Rust differs:

- **`explain`** shows the change as source, `total >= 100 → (total) > (100)`, where the Java edition
  shows bytecode, and its `last run` comes from the `mutation.json` of the last run of every
  mutant, by `run` or `next`.
- **`next`** queues the misses as the Java edition does, in the same words and events, new misses
  before the baseline's. `--top` takes at most one item a function, where Java
  says a method. Whether the code under test is the same, so that only what the last `next` left
  uncaught runs, is a hash of the files mutastic mutates with their test code (`#[test]` functions
  and `#[cfg(test)]` modules) taken out, as Java hashes only the classes under test. See
  [Working through misses](concepts/next.md).
- **A function only the crate calls** (not `pub`) gets no suggested test on an input none of its
  callers passed: its card says no caller passed the value, and to test a caller that can, or
  propose the mutant unreachable. A test of it alone on that value could test what can't happen.
- **A run compared with the last** works as in the Java edition. A mutant that crashed where a
  crash proves nothing is out of the score, as a skipped one is, so a change to or from one isn't
  counted against the test that caught it. A run with tests left out (`exclude-tests:` or
  `--exclude-tests`) counts as other code than one that ran them.
- **`--each-mutant`** gives every missed mutant its own card, as in the Java edition. The Rust
  edition's `full` format has no limit on the cards it shows for a file, so there's no `… and 3
  more missed mutants in this file` line for it to lift.
- **The agent format at 100%** still has `rules:` and `decline:` after the verdict; the Java
  edition's stops at `vs last run:` and `reports:`, as nothing is left for them to govern.
- **The summary's `Left out` row** names each test left out, with its reason, as the Java
  edition's does, then the code the Rust edition doesn't mutate, on lines of its own under it.
- **Report files** are `mutation.json` and `mutation.html` in `target/mutastic`; there's no
  `mutations.xml`, whose consumers key on JVM mutator names.
- **One run across CI jobs,** `--shard` and `--from-shards`, works as in the Java edition: the same
  deal by id, the same folder for each share, the same refusals, and the whole run put back
  together. The one difference follows from `mutation.json` being the last run's record here: a
  share writes its report files in its own folder, `target/mutastic/shares/N-of-M/`, beside its
  events, so `explain` never reads a share as the whole run. Each share makes the one build. See
  [One run across CI jobs](concepts/how-a-run-works.md#one-run-across-ci-jobs).
- **`--at`** runs or lists the mutants on some lines, as in the Java edition, in the same words and
  events. A mutant is on the lines its claim covers, from where its change starts to where it
  ends, as `mutation.json`'s `location` has them, where Java reads javac's line for its statement.
  The path is from the workspace, as a card prints it, and the sources it must be in are the
  files the package's crate roots reach; at a workspace's root, the file picks its member. The
  count line names a crash that proves nothing, `1 crashed`, a fate the Java edition doesn't
  have. `mutants --at` leaves out the package's `not mutated` lines, which count the package, not
  the lines. See [One line](scoping.md#one-line).
- **`mutastic.yaml` and rules files** are the Java edition's, with Rust paths for classes
  (`shop::generated::**`) and Rust types as owners. A rules file's operator matches a call by its
  method's name, since mutastic reads source without types; a rewrite that doesn't fit doesn't
  compile, and is withdrawn. The rewrites are the Java edition's, but for `omit last argument`,
  which calls a shorter overload: a Rust method has one count of arguments. See
  [Configuration](reference/configuration.md#rewrites).
- **The gate** is the Java edition's, so the coding agents' plugins run it unchanged. A test it
  counts is one the test binaries list, each `#[case]` and test-case row a test of its own, and one
  turned off is `#[ignore]`, where Java counts `@Test` and `@Disabled` in the source. `gate open`
  lists them in the background, on the tree as it was then, as the Java edition counts a
  parameterized test's cases, and a check waits for the list. Where the list failed, as for a
  package that doesn't build when the task starts, or isn't done in time, it counts
  `#[test]`-like functions in `src` and `tests` instead, and a pass says so. A check waits for the
  list before its own run, where Java waits for its count after: so a hooked check sets aside a
  run as long as the last check's when it works out how long the budget leaves for the wait.
- **Skipped as arid:** a statement that only writes output, `println!`, `eprintln!`, `dbg!` or a
  logger's macro, where the Java edition skips a call to a logger or `System.out`. A value a `let`
  works out only for output macros to read, where the Java edition follows a value in the bytecode
  to a logger, inline or through a local, by its uses on every path. And a statement
  that only checks a value it's given, `x.unwrap();` or `x.expect("..");`, where the Java edition
  skips a removed `Objects.requireNonNull(x);`.
  Both editions skip a sleep removed (`thread::sleep`, an awaited tokio `sleep`), the number that
  only sets how long it waits, the number that only sizes a collection (`with_capacity`,
  `reserve`), and an error's or a failed check's text (`Err`, `context`, `expect`). Both count a
  counter's first `0` or `1`. Every mutant in a `Debug` or `Hash` impl is arid, as Java's `toString()` and
  `hashCode()` are; a `Display` is counted, as what it writes is often what a person reads.
  Both editions skip a thread's name and stack size; Rust knows the call by the builder its chain
  starts at, `thread::Builder::new()` or a tokio runtime's builder, where Java knows it by the
  `Thread` or builder method. Java also skips a priority, which `std` has no call for, and a
  pool's largest size beside a queue with no limit, as equivalent. Other code the Java edition
  skips as arid, this edition doesn't make a mutant for, or counts:
  [Arid rules](https://docs.mutastic.io/reference/arid/) says which, rule by rule.
- **Skipped as equivalent:** a `?` that only passes on a formatter's error, inside the `fmt` of
  `Display` and the other formatting traits, or on a write to a `Formatter`: no test can make that
  error happen.
  Also a change to a zero check on a `NonZero*` value that differs only at 0: `parts.get() == 0`
  made never true, where `parts` is a `NonZeroU32`, which can't hold 0. And a change to a zero
  check on an unsigned value that differs only below 0, `n == 0` made `n <= 0`, where `n`'s type
  is written `usize` or it's cast to one; the Java edition takes lengths and counts from the JDK.
  And `cfg!(debug_assertions)` forced true, which it always is under mutastic's build.
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
