# mutastic for Rust: the docs

mutastic changes your code in small ways, such as `>=` to `>`, and runs your tests against each
change. A change no test notices is a gap: code your tests run but don't check. For each one,
mutastic shows the inputs your tests used, the test that would close the gap, and the command that
proves it closed.

These pages are the Rust edition's. They're written to move into the Rust section of
[docs.mutastic.io](https://docs.mutastic.io) once the site publishes more than one edition, so each
page stands alone and links to the others by relative path.

## Getting started

- [Getting started](getting-started.md): build mutastic, run it on a crate, read the first card,
  write the test, and prove it.
- [Installing mutastic](install.md): the installer, building from source, and how a release is
  laid out.
- [One command, two editions](editions.md): which edition runs a project, a Rust crate inside a
  Gradle build, and the Java edition, fetched the first time it's needed.

## Concepts

- [How a run works](concepts/how-a-run-works.md): one build for every mutant, switches, what
  happens to a mutant that doesn't compile, one run split across CI jobs, and CI that fails only
  on new misses.
- [Results and the score](concepts/results.md): caught, missed, timed out, not covered, skipped,
  and the crashes that prove nothing.
- [Unsafe code](concepts/unsafe.md): what's left alone, what's marked, and when to reach for Miri.
- [Profiles](concepts/profiles.md): why overflow checks and debug assertions are always on.
- [Mutant ids](concepts/ids.md): what an id is made of, and what keeps it stable.
- [Suggested tests](concepts/suggested-tests.md): the test a card writes, and how its values are
  found by running your code.
- [Output](concepts/output.md): each format, for people, coding agents and tools, and `explain`.
- [Working through misses](concepts/next.md): `mutastic next`, the missed mutants as a queue, one
  at a time, the one most worth a test first, and a baseline for older misses.

## Operators

- [Operators](operators.md): every operator, its set, its family, and its Java equivalent, in one
  table.
- A page for each family, in the Java edition's order, each operator with the bug it stands for, an
  example from a real run, the card it gives and the test that catches it:
  [conditions](operators/conditions.md), [arithmetic](operators/arithmetic.md),
  [returns and errors](operators/returns.md), [calls and writes](operators/calls.md),
  [arguments](operators/arguments.md), [library methods](operators/library.md),
  [iterators](operators/iterators.md), [`Option` and `Result`](operators/options.md) and
  [control flow](operators/control.md).

## Reference

- [Commands](reference/commands.md): each with every option it takes, and
  [environment and exit codes](reference/commands.md#environment).
- [Report files](reference/reports.md): `mutation.json` and `mutation.html`.
- [Events](reference/events.md): the `--format=jsonl` stream, and what the Rust edition adds to it.
- [Errors](reference/errors.md): each error mutastic stops with, its code, and what to run next.
- [Configuration](reference/configuration.md): `mutastic.yaml`, rules files, and ignoring one mutant.
- [Compatibility](reference/compatibility.md): what stays the same from one release to the next:
  ids, scores, commands and configuration, the events format, and the two editions' versions.

## Guides

- [Working test-first](test-first.md): one red, green, refactor cycle on a fixture, with mutastic
  on the cycle's lines, the card's tests proved with `verify`, and a commit.
- [Working test-first: the refactor](test-first-refactor.md): two more cycles on the same rule, then the
  refactor they call for, with mutastic on the lines before and after it.
- [Working test-first: with an agent](test-first-agent.md): the same cycle as a coding agent
  works it, with what mutastic and the gate tell it at each step, and where you come in.

## Help

- [Troubleshooting](troubleshooting.md): unviable mutants, slow runs, workspaces, test code, and
  tables of cases and property tests.
- [Differences from the Java edition](differences-from-java.md): only where you'd do something
  different.

## How these pages stay true

Every block of output on these pages is what mutastic printed on one of the repository's
fixtures, kept in [`output/`](output/). `cargo test` runs each command again and fails while
mutastic prints anything else; `MUTASTIC_DOCS_WRITE=1 cargo test --test docs` writes the copies
again after a change to what it prints. Every Rust example is a fixture's own code, quoted word for
word from a file the tests build, or code a page adds: the getting-started page's test, which the
same test adds and runs; the test-first pages' cycles, which it writes step by step in a repository
of its own, capturing each run; and each operator's test that catches it, which are added to the
operators fixture together and must catch every mutant on the cards they follow. Every link between pages,
every command shown and every mutant id named is checked too.
