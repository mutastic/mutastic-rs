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

- [How a run works](concepts/how-a-run-works.md): one build for every mutant, switches, and what
  happens to a mutant that doesn't compile.
- [Results and the score](concepts/results.md): caught, missed, timed out, not covered, skipped,
  and the crashes that prove nothing.
- [Unsafe code](concepts/unsafe.md): what's left alone, what's marked, and when to reach for Miri.
- [Profiles](concepts/profiles.md): why overflow checks and debug assertions are always on.
- [Mutant ids](concepts/ids.md): what an id is made of, and what keeps it stable.
- [Suggested tests](concepts/suggested-tests.md): the test a card writes, and how its values are
  found by running your code.

## Operators

- [Operators](operators.md): every operator, its set, and its Java equivalent, in one table.
- A page for each family, each with an example from a real run and the card it gives:
  [conditions](operators/conditions.md), [arithmetic](operators/arithmetic.md),
  [returns and errors](operators/returns.md), [calls and writes](operators/calls.md),
  [arguments](operators/arguments.md), [std's methods](operators/library.md),
  [iterators](operators/iterators.md), [`Option` and `Result`](operators/options.md) and
  [`match` and loops](operators/control.md).

## Reference

- [Commands](reference/cli.md): `run`, `verify` and `mutants`, and every option.
- [Events](reference/events.md): the `--format=jsonl` stream, and what the Rust edition adds to it.
- [Configuration](reference/configuration.md): the environment variables mutastic reads.

## Help

- [Troubleshooting](troubleshooting.md): unviable mutants, slow runs, workspaces, and test code.
- [Differences from the Java edition](differences-from-java.md): only where you'd do something
  different.

## How these pages stay true

Every block of output on these pages is what mutastic printed on one of the repository's
fixtures, kept in [`output/`](output/). `cargo test` runs each command again and fails while
mutastic prints anything else; `MUTASTIC_DOCS_WRITE=1 cargo test --test docs` writes the copies
again after a change to what it prints. Every Rust example is a fixture's own code, quoted word for
word from a file the tests build, or the test the getting-started page adds, which the same test
adds and runs. Every link between pages, every command shown and every mutant id named is checked
too.
