# Compatibility

mutastic's version says what a release may change. From 1.0.0, a release that breaks one of the
promises below is a new major version. Before 1.0.0, anything may change, and a release that
breaks something says so in the [changelog](../../CHANGELOG.md).

## Mutant ids

A script's `mutastic verify mb2f8ae57`, an ignore and a coding agent's notes all name a mutant by
its id, and an id names the same mutant in every later version while the code at its site is
unchanged. [Mutant ids](../concepts/ids.md) says what keeps an id, and what changes one. The tests
hold every mutant of every fixture to a kept list, and fail if an id goes or comes to name another
mutant: a new operator only adds ids.

## Scores

A score is the share of the mutants caught. The same code and tests score the same, unless a
release says otherwise:

- **A patch release** (`1.2.3` to `1.2.4`) never moves a score. It fixes what mutastic gets wrong
  without changing which mutants count.
- **A minor release** (`1.2` to `1.3`) may change a score: an operator added to the default set,
  or a filter that skips more or less. Its changelog says which, and how scores move, under
  *Scoring changes*.
- **A score compares only with one under the same operators.** `--operators`, or `operators:` in
  `mutastic.yaml`, decides which mutants there are, so choosing more operators can lower a score
  without the tests changing.

A threshold may need a look after a minor release's scoring change, never after a patch.

## Commands, `mutastic.yaml` and rules files

A command line, a `mutastic.yaml` and a rules file that work with one release work with every
later release of the same major version: a command, an option, a value an option takes, an
environment variable and an exit status may be added, and none is taken away; a key may be added
to either file, and none is taken away or read another way. The tests hold each build to what the
last 1.0+ release took, kept in `crates/mutastic/tests/released/`: its commands, and the
`mutastic.yaml` and rules files of a fixture, with how each read.

## The events format

`--format=jsonl` writes `mutastic.events/v1` events. Before 1.0.0 the schema may change; from
1.0.0 it only gains: a field keeps its name and its type, and anything new is optional. See
[Events](events.md#what-stays-the-same).

The agent format (`--format=agent`) is for a coding agent to read as text. From 1.0.0 its
structure is fixed and its words are not: see [Output](../concepts/output.md#the-agent-formats-contract).

## The two editions

From 1.0.0, the Rust and Java editions release each minor version together, at the same version,
and keep the same promises; either may ship a patch alone. In a Maven or Gradle project,
`mutastic` runs the newest Java edition of its own major and minor version, so `mutastic 1.2.0`
runs Java 1.2.3 once that's out, unless `MUTASTIC_VERSION` names another: see [One command, two editions](../editions.md#the-java-edition).

## What's experimental

Anything the docs call experimental may change in a minor release, and its mutants' ids may move
as it settles.
