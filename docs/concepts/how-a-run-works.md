# How a run works

A mutation testing tool for Rust has a problem the Java edition doesn't: a change to the source
needs the compiler, and building a crate once per mutant takes too long. mutastic builds your crate
once, with every mutant in it, and picks which one is live when the tests run. What it says as it goes, on `fixtures/pricing`:

<!-- output: concepts/run-progress -->
```text
mutastic 0.0.1 · reading the Cargo package…
building  11 mutants in one build…
coverage  3 tests on rustc 1.94.0 in 4ms · 11 mutants
 ❯ [pricing] src/lib.rs (11 mutants | 1 missed) 60ms
   × discount() :26 >= becomes >
```

## One copy, one build

mutastic never touches your source, your `Cargo.toml` or your `target/`. It copies the workspace to
`target/mutastic/copy`, beside a target directory of its own, and writes each mutant into the copy
behind a switch. A site with mutants becomes a `match` on the switch: the mutants are its arms, and
your code is the last one.

<!-- from: fixtures/pricing/src/lib.rs -->
```rust
        if total >= 100 {
```

becomes, in the copy:

<!-- output: concepts/schema-line -->
```text
if (match crate::__mutastic::at(2) { 2 => { (total) > (100) } 3 => { true } 4 => { false } _ => { total >= 100 } }) {
```

The switch reads `MUTASTIC_ACTIVE` once, as the test binary starts. With no mutant named, every
`match` takes your code's arm, so the copy behaves as your crate does. A small module at the end of
your crate root holds the switch; nothing is added to `Cargo.toml`, so a `--locked`, `--offline` or
vendored build works as it does for you.

The copy keeps your line numbers: the rewritten code never adds a line, so a panic's line and a
card's line are your file's. The copy is written only where its bytes change, so Cargo's own
freshness checks keep the next run's build short.

mutastic builds the copy with your own `cargo` and `rustc`, stable or not, from Rust 1.79 on: the
code it adds to your crate uses inline `const` blocks, which came then. Flags reach
only your crate, through `RUSTC_WORKSPACE_WRAPPER`, never `RUSTFLAGS`, so your own rustflags still
apply and no dependency is built again. The one flag added caps lints, so a `#![deny(warnings)]`
in your crate doesn't fail a build on code it was never meant to judge. The build uses a profile
of mutastic's own: see [Profiles](profiles.md).

## A mutant that doesn't compile is rolled back

Each mutant and your code are arms of one `match`, so they must have the same type. Most operators
only make changes that are sure to compile. A few rely on the compiler to say: an argument list
swapped where the types might differ, a call left out whose result might be another type, a `?`
turned into `.unwrap()`.

When the build fails, mutastic reads the compiler's errors, works out which mutants they belong
to, withdraws those, and builds again. An error in a mutant's own arm names that mutant. An error
around a guard, such as a borrow that a mutant's arm upsets, names the mutants in it. Only when an
error can't be placed closer does it name every mutant in its function. The rounds are `cargo
check`, and once that passes, a `cargo build`; the summary counts them, and a build that needed
nothing withdrawn takes two (`2 rounds (check and build)`).

A withdrawn mutant is skipped as unviable, with the compiler's error as its reason, and stays out
of the score: a change that doesn't compile is one no test needs to catch. See
[Troubleshooting](../troubleshooting.md#a-mutant-is-skipped-as-unviable) for what one looks like.

If the build fails for a reason no mutant explains, mutastic stops and prints the compiler's
error: your crate needs to build as it is first.

## What each test reaches

With the build done, mutastic lists every test in your package's test binaries: unit tests in
`#[cfg(test)]` modules, integration tests in `tests/`, and the tests of each binary. Tests marked
`#[ignore]` are left out. Then it runs each test once, on its own, with the switches recording
which sites it reaches and the whole numbers each function was called with. Those numbers are the
"inputs seen" on a card.

Every test must pass here. A test that fails with no mutant in would look as if it caught every
mutant it met, so a test that fails runs again, on its own, a few times. If it passes, it's flaky:
the run goes on without it, and says so (`! flaky:` before the summary, and a `Flaky` row), with
what it reaches that answers differently every run, such as `SystemTime::now` or a `RandomState`.
Its catches don't count, so code only it reaches is not covered. A test that fails every time
stops the run, with its name and where its output is.

## Each mutant against the tests that reach it

Each mutant then runs against only the tests that reach its site, in the test binary mutastic
built, with `MUTASTIC_ACTIVE` naming it. The first test to fail stops it: the mutant is caught. If
they all pass, it's missed. If none reach it, it isn't run, and it's not covered. Mutants run side
by side, one process each, on `--workers` threads.

Each run has a time limit, set from how long its tests took in the coverage pass, so a mutant
that makes a loop run forever is stopped and counted as timed out, which counts as caught.

A test process that dies without a test failing, by a signal or an abort, is run again one test at a
time to see how it died. What that means depends on where the mutant is: see
[Results](results.md#caught-only-because-the-test-process-crashed).

## Running it again

The copy and its build stay in `target/mutastic`, so the next run, or a `verify`, rebuilds only what
changed. `cargo clean` removes them with the rest of `target/`.
