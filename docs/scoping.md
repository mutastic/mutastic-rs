# Changed lines, members and shares

A whole run makes a mutant of every change mutastic can make to the package, and runs the tests
against each one. Most of the time you need less than that: the lines a pull request changed, one
member of a workspace, the mutants on a line, the mutants a card named, or a run split across CI
jobs. This page shows how to ask for each.

## Only what changed

`--since=main` mutates only the lines changed since the branch left `main` (its merge-base),
uncommitted edits and new files included. That's a pull request's change, or an agent's, which is
where a missed mutant is worth acting on. A bare `--since` means the repository's default branch.

```sh
mutastic run --since=main
```

The summary says what the run covered, as `Changed  14 lines in 1 file since HEAD`. A run of a
change has a different score from a whole run by design, so it isn't compared with the last run,
and doesn't replace it: see [Compared with the last run](concepts/how-a-run-works.md#compared-with-the-last-run).

A change with no mutant on its lines has nothing to measure. mutastic builds nothing, runs no test,
and passes in a second or so:

```sh
mutastic run fixtures/pricing --since=HEAD
```

<!-- output: concepts/nothing-to-run -->
```text

    Files  0 clean (0)
  Mutants  ✓ 0 caught (0)
  Changed  0 lines in 0 files since HEAD
    Build  nothing built: no mutant to run
 Start at  19:31:48
 Duration  74ms (setup 74ms)
    Tests  none ran: no mutant to run
    Score  none  [PASS] Nothing to measure: no mutants to score.
```

`--force-coverage` builds the crate and runs the tests anyway, to check that they pass or to time
them. See [When there's nothing to run](concepts/how-a-run-works.md#when-theres-nothing-to-run).

## A coding agent's task

The gate scopes itself. `gate check` runs mutastic on the lines changed since the task started, as
`--since=<that commit>` would, so there's nothing to pass it. See
[What the gate holds it to](test-first-agent.md#what-the-gate-holds-it-to).

## One member of a workspace

mutastic runs one package at a time. Pointed at a workspace's root, it stops and gives the command
for each member:

<!-- output: troubleshooting/workspace -->
```text
mutastic: fixtures/workspace is a workspace's root, and mutastic tests one package at a time.
It has 2 packages. To test one:
  mutastic run fixtures/workspace/billing
  mutastic run fixtures/workspace/shop
```

Run the member by its directory, or `cargo mutastic run` inside it. Anywhere inside a member, such
as its `src`, mutastic runs that member, as Cargo would. Each member reads its own `mutastic.yaml`,
ignores and baseline, from its own directory.

Only the member's own tests count. A test in another member that calls its code doesn't reach its
mutants, so that code may show as not covered. See [A workspace](troubleshooting.md#a-workspace).

## One line

When you're fixing a line, or an agent is working on one function, you have a file and a line in
hand, not mutant ids. `--at` takes them: `--at=<file>:<line>`, or `<file>:<from>-<to>` for the
lines between, counted from 1 as an editor counts them. `mutants --at` lists what's there, and `run
--at` runs it:

```sh
mutastic mutants fixtures/pricing --at=src/lib.rs:26-29
mutastic run fixtures/pricing --at=src/lib.rs:26-29
```

<!-- output: scoping/at-mutants -->
```text
mc9e13290 src/lib.rs:29:9 mutastic/return-value returns 1 instead in Pricing::discount
mb2f8ae57 src/lib.rs:26:18 mutastic/boundary >= becomes > in Pricing::discount
md56e8c39 src/lib.rs:26:12 mutastic/remove-conditional total >= 100 is always true in Pricing::discount
m1636f6d5 src/lib.rs:26:12 mutastic/remove-conditional total >= 100 is always false in Pricing::discount
m6d05659f src/lib.rs:27:20 mutastic/return-value returns 0 instead in Pricing::discount
m95f96847 src/lib.rs:27:26 mutastic/math / becomes * in Pricing::discount
6 mutants
```

A mutant is on a line when its card points at the line: from the line its change starts on to the
one it ends on, so each line of an expression written over several has its mutants. The operators
are the run's own, as a full run would have them on those lines; with `--since`, only those on the
change. Several files or spans go in one option, split by commas
(`--at=src/lib.rs:26,src/tier.rs:3-5`), or in several.

The run is about those mutants, as [`--only`'s](#some-mutants) is: it says each one's fate, after a
line that counts them, and passes when every one is caught or skipped. It has no score, writes no
report and isn't kept as the last run.

<!-- output: scoping/at-run -->
```text

 Missed ────────────────────────────────────────────────────────────

 ❯ src/lib.rs  1 missed in 1 function

 [✗ MISSED] no test checks discount() at total == 100
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

     Files  0 clean | 1 with misses (1)
   Mutants  ✓ 5 caught · ✗ 1 missed (6)
  Worst at  Pricing::discount() 1 ✗
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  8 (18 without coverage selection)
  Start at  02:35:01
  Duration  3.1s (setup 1.1s, build 1.9s, coverage 12ms, mutants 48ms, checking 10ms)
     Tests  3 on rustc 1.100.0 · 4 workers

 lines 26–29 of lib.rs: 6 mutants, 5 caught, 1 missed
 [CAUGHT] mc9e13290 is caught by tests::small_orders_pay_full_price
 [MISSED] mb2f8ae57 is still missed · done when mutastic verify mb2f8ae57 fixtures/pricing passes
 [CAUGHT] md56e8c39 is caught by tests::small_orders_pay_full_price
 [CAUGHT] m1636f6d5 is caught by pricing::big_orders_get_ten_percent_off
 [CAUGHT] m6d05659f is caught by pricing::big_orders_get_ten_percent_off
 [CAUGHT] m95f96847 is caught by pricing::big_orders_get_ten_percent_off
```

The path is absolute, or from where you run mutastic, or from the workspace, as a card prints it.
The file must be one of the package's sources; at a workspace's root it picks its member, so
`mutastic run --at=shop/src/lib.rs:4` there runs `shop`. Lines with no mutant on them are an
error, never a run of none: it names the nearest lines that have one, and the `mutants --at`
command that lists them (see [`no-mutant-at`](reference/errors.md#no-mutant-at)). The lines are
read from the file on disk, so an editor's unsaved changes aren't in them.

## Some mutants

`--only` runs the mutants you name, by id, split by commas: the ones a card listed, say. Such a run
is judged on each mutant, as `verify` judges it, not on a score: it passes when each one is caught
or needs no test, whatever the threshold, and ends on each one's fate:

```sh
mutastic run fixtures/pricing --only=mb2f8ae57
```

To check that a test you've written catches them, `mutastic verify` does the same with less to
read: a line for each mutant. See [What `verify` says](concepts/results.md#what-verify-says).

## One run across CI jobs

A crate's run can take longer than one CI job should. `--shard=N/M` runs only the Nth of M shares of
its mutants, and `--from-shards` puts the shares back together as the whole run, with its score,
reports and threshold:

```sh
mutastic run --shard=2/4 --threshold=0
mutastic run --from-shards=shares --threshold=70
```

For a member of a workspace, name its directory in both commands. See
[One run across CI jobs](concepts/how-a-run-works.md#one-run-across-ci-jobs), with a workflow for
GitHub Actions and GitLab.
