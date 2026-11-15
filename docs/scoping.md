# Changed lines, members and shares

A whole run makes a mutant of every change mutastic can make to the package, and runs the tests
against each one. Most of the time you need less than that: the lines a pull request changed, one
member of a workspace, the mutants a card named, or a run split across CI jobs. This page shows how
to ask for each.

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
