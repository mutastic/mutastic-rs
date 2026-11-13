# Errors

When mutastic can't run, it says why on stderr, as `mutastic: ` and one sentence naming what's
wrong in your words, then where it looked, and last the command to run, with your own values. It
exits 2. In `--format=agent` and `--format=jsonl`, the same error is the last line on stdout, as the
[`run-failed` event](events.md#when-mutastic-cant-run), with a `code` for its kind: each kind has an
entry here, under its code. Each output below is a kept copy of a real run, and each command an
error gives as one to run as it is, is run by the docs test, and gets past the error.

A refusal that does nothing, such as `ignore --remove` for a mutant that isn't ignored, or
`baseline` when there's one already, is said on stderr too, and exits 1. Stdout holds only results.

## Exit codes, and the Java edition's

Both editions exit 0 when the run passes and 1 when it doesn't. The Rust edition exits 2 for every
error on this page. The Java edition exits 2 for most of them, 3 when a test fails before any
mutant, and 4 for a bug in mutastic itself. The Rust edition keeps 2 for both until its 2.0, so a
script that wraps `mutastic` should treat any status above 1 as "couldn't run", and read the event's
`code` for which. See [Differences from the Java edition](../differences-from-java.md).

## `no-directory`

The directory named isn't there. mutastic says where it looked, offers a package or directory whose
name is close, and ends in the command to run instead.

```sh
mutastic run fixtures/pricng
```

<!-- output: errors/no-directory -->
```text
mutastic: there's no directory fixtures/pricng in /home/you/project.
There's a directory called fixtures/pricing. To test it:
  mutastic run fixtures/pricing
```

## `no-project`

No `Cargo.toml`, `pom.xml`, `build.gradle(.kts)` or `settings.gradle(.kts)` in the directory or any
above it. Go to your project, or name its directory.

```sh
mutastic run empty
```

<!-- output: errors/no-project -->
```text
mutastic: there's no project in empty for mutastic to test.
It looked in /home/you/project/empty and every directory above it, for Cargo.toml, pom.xml, build.gradle(.kts) or settings.gradle(.kts).
Go to your project, or name its directory:
  mutastic run <project directory>
```

## `workspace-root`

A workspace's root, or a directory in a workspace but in none of its packages: mutastic tests one
package at a time, and gives the command for each. Inside a member, it runs that member.

```sh
mutastic run fixtures/workspace
```

<!-- output: errors/workspace-root -->
```text
mutastic: fixtures/workspace is a workspace's root, and mutastic tests one package at a time.
It has 2 packages. To test one:
  mutastic run fixtures/workspace/billing
  mutastic run fixtures/workspace/shop
```

## `no-cargo`

There's no `cargo` to run: mutastic says where it looked, the `CARGO` variable when it's set, or the
PATH.

```sh
CARGO=/nowhere/cargo mutastic run fixtures/pricing
```

<!-- output: errors/no-cargo -->
```text
mutastic: CARGO is /nowhere/cargo, and there's no cargo there.
mutastic runs the cargo CARGO names. Point it at one, or unset it to use the cargo on the PATH:
  unset CARGO
```

## `unknown-command`

A command mutastic doesn't have. A close one is offered, never run, in your line with only that word
changed; with none close, one line of usage.

```sh
mutastic rnu fixtures/pricing
```

<!-- output: errors/unknown-command -->
```text
mutastic: mutastic has no command rnu. There's one called run.
  mutastic run fixtures/pricing
```

## `unknown-option`

An option mutastic doesn't have, including a short one such as `-v`: its options are all long ones.
A close one is offered in your line; with none close, the command's usage and its `--help`.

```sh
mutastic run fixtures/pricing --formt=agent
```

<!-- output: errors/unknown-option -->
```text
mutastic: run has no option --formt. There's one called --format.
  mutastic run fixtures/pricing --format=agent
```

## `option-value`

A value an option doesn't take, or one given after a space: values go after an `=`. The valid ones
are listed, and a close one offered.

```sh
mutastic run fixtures/pricing --format agent
```

<!-- output: errors/option-value -->
```text
mutastic: --format takes its value after an =, not a space.
  mutastic run fixtures/pricing --format=agent
```

## `option-not-taken`

An option mutastic has, on a command that doesn't take it. The command's synopsis follows.

```sh
mutastic verify mb2f8ae57 fixtures/pricing --threshold=70
```

<!-- output: errors/option-not-taken -->
```text
mutastic: verify doesn't take --threshold: mutastic verify <id>[,<id>…] [<id>…] [dir]
```

## `extra-words`

A word the command doesn't take, after its directory.

```sh
mutastic mutants fixtures/pricing fixtures/workspace
```

<!-- output: errors/extra-words -->
```text
mutastic: mutants doesn't take fixtures/workspace: mutastic mutants [dir]
```

## `needs-id`

A command that needs a mutant's id, `m` and eight hex digits as a card shows it, given something
else; or `ignore` without its reason.

```sh
mutastic verify fixtures/pricing
```

<!-- output: errors/needs-id -->
```text
mutastic: verify needs a mutant's id, m and eight hex digits as a card shows it, and fixtures/pricing is a directory: mutastic verify <id>[,<id>…] [<id>…] [dir], as in mutastic verify m1a2b3c4d fixtures/pricing
```

## `unknown-id`

An id that's a mutant's shape, but no mutant of the package: kept from before the code changed, or
mistyped. An id one digit, or one swap, from a mutant's is offered as that mutant's.

```sh
mutastic verify mb2f8ae58 fixtures/pricing
```

<!-- output: errors/unknown-id -->
```text
mutastic: mb2f8ae58 isn't a mutant of pricing. There's one called mb2f8ae57. To see its mutants:
  mutastic mutants fixtures/pricing
```

## `config`

A `mutastic.yaml`, a rules file, or `.mutastic/ignores.yaml` that mutastic can't read: the file and
line, and what that line could hold.

```sh
mutastic run configured
```

<!-- output: errors/config -->
```text
mutastic: mutastic.yaml line 1: expected extends:, rules:, plugins:, filters: or operators:, not rulez:. There's one called rules:.
```

## `no-baseline`

`--fail-on-new` with no `.mutastic/baseline.yaml` to tell new misses from.

```sh
mutastic run fixtures/pricing --fail-on-new
```

<!-- output: errors/no-baseline -->
```text
mutastic: --fail-on-new needs a baseline to tell new misses from, and /home/you/project/fixtures/pricing has no .mutastic/baseline.yaml: `mutastic baseline` writes one, to commit
```

## `git`

`--since` or the gate outside a git repository, or with a commit git doesn't know.

```sh
mutastic run fixtures/pricing --since=main
```

<!-- output: errors/git -->
```text
mutastic: --since needs a git repository
```

## `shards`

Shares `--from-shards` can't put together as one run: none there, one missing or twice, from
different runs, code or versions.

```sh
mutastic run fixtures/pricing --from-shards=fixtures/workspace
```

<!-- output: errors/shards -->
```text
mutastic: fixtures/workspace holds no share's events.jsonl: each run with --shard keeps its own in shares/N-of-M, in target/mutastic
```

## `both-editions`

A directory with both a `Cargo.toml` and a Maven or Gradle build, where either edition could run.
Say which with `--edition`.

```sh
mutastic run both
```

<!-- output: errors/both-editions -->
```text
mutastic: /home/you/project/both has both Cargo.toml and pom.xml, so either edition could read it: say which with --edition=rust or --edition=java
```

## `build-failed`

The package doesn't build before any mutant is in it. mutastic checks the package as you wrote it
and names each error by your file and line, then the `cargo test --no-run` that shows them in full.
See [The package doesn't build](../troubleshooting.md#the-package-doesnt-build).

## `copy-build-failed`

Your package builds, but mutastic's copy of it, with the mutants in, doesn't: a bug in mutastic. The
message has the copy's first error, to report.

## `red-baseline`

A test fails with no mutant in, every time it runs: how it failed, where its output is, and the
`cargo test` that runs it alone. See [A test fails before any
mutant](../troubleshooting.md#a-test-fails-before-any-mutant).

## `java-edition`

The Java edition can't run: no JDK 25, a `MUTASTIC_JAVA_ENGINE` that isn't a file, no `HOME` to keep
it in, or no release to fetch. See [One command, two editions](../editions.md).

## `proc-macro`

A proc-macro crate, which mutastic can't mutate yet.

## `other`

Anything else, usually what the system said: a disk that's full, a file mutastic can't write. The
message names the file.
