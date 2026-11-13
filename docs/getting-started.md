# Getting started

This runs mutastic on `fixtures/pricing`, a small crate in mutastic's repository whose tests pass and
reach every line, and still miss something. Then it closes the gap and proves it.

## 1. Install it

The installer puts the latest release in `~/.local/bin`:

```sh
curl -fsSL https://mutastic.io/install.sh | sh
```

That installs two commands: `mutastic`, and `cargo-mutastic`, which Cargo runs as `cargo mutastic`.
They do the same thing. [Installing mutastic](install.md) has what it does, and how to build from
source where there's no release.

The crate this page runs on is in mutastic's repository:

```sh
git clone https://github.com/mutastic/mutastic-rs.git
cd mutastic-rs
```

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

  ✗ MISSED  no test checks discount() at total == 100
    ┌─ src/lib.rs:26
    │
 25 │     pub fn discount(&self, total: u32) -> u32 {
 26 │         if total >= 100 {
    │                  ^^ >= changed to >
 27 │             return total / 10;
    │
    = mutant  - if total >= 100 {
              + if total > 100 {
    = note: every test calls discount() away from the boundary
            inputs seen: 50, 250 (none is 100)
            covered by tests::small_orders_pay_full_price, pricing::big_orders_get_ten_percent_off (all 2 covering tests still pass)
    = help: test discount() with total = 100 and assert the result

            // in mod tests
            #[test]
            fn discount_at_the_total_edge() {
                assert_eq!(Pricing::new().discount(100), 10); // today 10 · with this mutant 0
            }
            check today's value is the one you intend: the test pins what the code does now

    done when  mutastic verify mb2f8ae57 fixtures/pricing passes
    reproduce  mutastic run --only=mb2f8ae57 fixtures/pricing
    or ignore  mutastic ignore mb2f8ae57 --reason="<why>" fixtures/pricing

 Next: mutastic verify <id> fixtures/pricing · mutastic run --only=<id> fixtures/pricing · mutastic explain <id> fixtures/pricing

     Files  0 clean | 1 with misses (1)
   Mutants  ✓ 10 caught · ✗ 1 missed (11)
  Worst at  Pricing::discount() 1 ✗
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  13 (33 without coverage selection)
  Start at  11:03:14
  Duration  355ms (setup 20ms, build 303ms, coverage 2ms, mutants 8ms, checking 6ms)
     Tests  3 on rustc 1.99.1 · 4 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  90.9%   PASS  1 missed, above the threshold of 80.
```

On your own crate, run `cargo mutastic run` inside it, or `mutastic run path/to/crate`.

## 3. Read the card

Each line with a missed mutant gets a card.

- **The gap,** first: no test checks `discount()` at `total == 100`, so nothing tells `>=` from `>`.
- **The change** that got through, under the code, and again as a diff.
- **The evidence:** the values your tests actually passed to `total`, none of them 100, and the
  tests that reach the line, which all still pass with the change in.
- **What to do:** test `discount()` with `total = 100`, and assert the result. Below it is that
  test, written out: mutastic called `discount(100)` as the code is and with the mutant in, and
  the comment says what each gave.
- **The commands:** `done when` proves the fix, and `reproduce` runs just this mutant again. Each
  names this crate, so it works as it's copied. `mb2f8ae57` is the mutant's id; it stays the same
  while the code around it moves. See [Mutant ids](concepts/ids.md).

The summary says how the rest went. The score is high enough to pass, but the run still shows the
one it missed: a passing score isn't a reason to leave a gap you can close.

## 4. Write the test

The card's test asserts what `discount(100)` gives today, 10. Is that right? The doc comment says
10% off orders of 100 or more, so yes. Paste it into the `tests` module at the end of
`fixtures/pricing/src/lib.rs`:

<!-- added to: fixtures/pricing/src/lib.rs -->
```rust
#[test]
fn discount_at_the_total_edge() {
    assert_eq!(Pricing::new().discount(100), 10); // today 10 · with this mutant 0
}
```

With `>` in place of `>=`, `discount(100)` is 0, and this test fails. When today's value isn't the
one the code should give, keep the test and write the right value: then it's the code that needs
fixing. See [Suggested tests](concepts/suggested-tests.md).

## 5. Prove it

Run the command the card offered as `done when`:

```sh
mutastic verify mb2f8ae57 fixtures/pricing
```

<!-- output: getting-started/verify -->
```text
mutastic: mb2f8ae57 is caught by tests::discount_at_the_total_edge
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
  Start at  11:04:49
  Duration  368ms (setup 21ms, build 322ms, coverage 2ms, mutants 8ms)
     Tests  4 on rustc 1.99.1 · 4 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  100.0%   PASS  Every mutant was caught.  ▲ 9.1 vs last run (code changed)
```

Every mutant is caught.

## Next

- [Results and the score](concepts/results.md): what caught, missed and the rest mean.
- [Operators](operators.md): the kinds of change mutastic makes, with a card for each.
- [Commands](reference/commands.md): every option, for CI and coding agents, and [the other formats](concepts/output.md).
- [How a run works](concepts/how-a-run-works.md): what happens to your crate, and what doesn't.
