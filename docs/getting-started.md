# Getting started

This runs mutastic on `fixtures/pricing`, a small crate in mutastic's repository whose tests pass and
reach every line, and still miss something. Then it closes the gap and proves it.

## 1. Install it

mutastic for Rust isn't on crates.io yet, so build it from its repository. It needs a Rust
toolchain, stable is fine, and nothing else.

```sh
git clone https://github.com/mutastic/mutastic-rs.git
cd mutastic-rs
cargo install --path crates/mutastic
```

That installs two commands: `mutastic`, and `cargo-mutastic`, which Cargo runs as `cargo mutastic`.
They do the same thing.

## 2. Run it

From the repository's root:

```sh
mutastic run fixtures/pricing
```

mutastic copies the crate into its `target/mutastic`, writes every mutant into the copy behind a
switch, and builds it once. It runs each test once to see what it reaches, then runs each mutant
against the tests that reach it. One mutant gets past them:

<!-- output: getting-started/run -->
```text

 Missed ────────────────────────────────────────────────────────────

 ❯ src/lib.rs  1 missed in 1 function

 [✗ MISSED] no test checks discount() at total == 100
    ┌─ src/lib.rs:26
    │
 25 │     pub fn discount(&self, total: u32) -> u32 {
 26 │         if total >= 100 {
    │                  ^^ changed to >
 27 │             return total / 10;
    │
    = mutant  - if total >= 100 {
              + if total > 100 {
    = note: every test calls discount() away from the boundary
            inputs seen: 50, 250 (none is 100)
            covered by pricing::big_orders_get_ten_percent_off, tests::small_orders_pay_full_price (all 2 covering tests still pass)
    = help: test discount() with total = 100 and assert the result

    done when  mutastic verify mb2f8ae57 fixtures/pricing passes
    reproduce  mutastic run --only=mb2f8ae57 fixtures/pricing
    or ignore  mutastic ignore mb2f8ae57 --reason="<why>" fixtures/pricing

     Files  0 clean | 1 with misses (1)
   Mutants  ✓ 10 caught · ✗ 1 missed (11)
  Worst at  Pricing::discount() 1 ✗
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  13 (33 without coverage selection)
  Start at  18:25:16
  Duration  2.1s (setup 1.9s, coverage 37ms, mutants 62ms)
     Tests  3 on rustc 1.94.0 · 4 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  90.9%  threshold 80

 [PASS] 1 missed, above the threshold.
 Next: mutastic verify <id> fixtures/pricing · mutastic run --only=<id> fixtures/pricing · mutastic explain <id> fixtures/pricing
```

On your own crate, run `cargo mutastic run` inside it, or `mutastic run path/to/crate`.

## 3. Read the card

Each line with a missed mutant gets a card.

- **The gap,** first: no test checks `discount()` at `total == 100`, so nothing tells `>=` from `>`.
- **The change** that got through, under the code, and again as a diff.
- **The evidence:** the values your tests actually passed to `total`, none of them 100, and the
  tests that reach the line, which all still pass with the change in.
- **What to do:** test `discount()` with `total = 100`, and assert the result.
- **The commands:** `done when` proves the fix, and `reproduce` runs just this mutant again. Each
  names this crate, so it works as it's copied. `mb2f8ae57` is the mutant's id; it stays the same
  while the code around it moves. See [Mutant ids](concepts/ids.md).

The summary says how the rest went. The score is high enough to pass, but the run still shows the
one it missed: a passing score isn't a reason to leave a gap you can close.

## 4. Write the test

What should `discount(100)` be? The doc comment says 10% off orders of 100 or more, so 10. Add a
test to the `tests` module at the end of `fixtures/pricing/src/lib.rs`:

<!-- added to: fixtures/pricing/src/lib.rs -->
```rust
#[test]
fn discount_starts_at_100() {
    assert_eq!(Pricing::new().discount(100), 10);
}
```

With `>` in place of `>=`, `discount(100)` is 0, and this test fails.

## 5. Prove it

Run the command the card offered as `done when`:

```sh
mutastic verify mb2f8ae57 fixtures/pricing
```

<!-- output: getting-started/verify -->
```text
mutastic: mb2f8ae57 is caught by tests::discount_starts_at_100
```

The mutant is caught: a test now fails when `>=` becomes `>`. `verify` exits with 0 when every
mutant it names is caught, and 1 when one isn't, so a script or a coding agent can use it as the
check that a gap is closed.

## 6. Run it again

```sh
mutastic run fixtures/pricing
```

<!-- output: getting-started/run-again -->
```text

     Files  1 clean (1)
   Mutants  ✓ 11 caught (11)
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  15 (44 without coverage selection)
  Start at  18:25:19
  Duration  1.0s (setup 1.0s, coverage 4ms, mutants 16ms)
     Tests  4 on rustc 1.94.0 · 4 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  100.0%  threshold 80

 [PASS] Every mutant was caught.
```

Every mutant is caught.

## Next

- [Results and the score](concepts/results.md): what caught, missed and the rest mean.
- [Operators](operators.md): the kinds of change mutastic makes, with a card for each.
- [Commands](reference/cli.md): every option, and the other formats, for CI and coding agents.
- [How a run works](concepts/how-a-run-works.md): what happens to your crate, and what doesn't.
