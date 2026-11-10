# One command, two editions

`mutastic` is one command for Rust, Maven and Gradle projects. The Rust edition is the command
itself. The Java edition runs Maven and Gradle projects: the first time `mutastic` runs in one, it
fetches the Java edition and hands it the command. Nothing about the Java edition is downloaded
until then, so a Rust-only machine never has it.

## Which edition runs

The directory a command points at decides, by its build file: the directory given, `.` when none
is, or the nearest directory above it with a build file.

| Build file | Edition |
|---|---|
| `Cargo.toml` | Rust |
| `pom.xml` | Java |
| `build.gradle.kts` | Java |
| `build.gradle` | Java |
| `settings.gradle.kts` | Java |
| `settings.gradle` | Java |

With no build file in the directory or above it, the Rust edition answers, and says there's no
project there. `--edition=rust` or `--edition=java` decides instead, for any command:

```sh
mutastic run --edition=java
```

A directory with both kinds of build file could be either edition's, so mutastic doesn't guess: it
stops, and asks for `--edition`.

## A Rust crate inside a Gradle build

A tree can hold both: a Gradle build with a Rust crate inside it, say, for a native library at
`native/src/main/rust`. Each is a project of its own, run by its own edition, so point mutastic at
the one you mean:

```sh
mutastic run                       # the Gradle build, with the Java edition
mutastic run native/src/main/rust  # the crate, with the Rust edition
```

At the Gradle build's root, `mutastic run` runs the Java edition, and first says which Rust crates
the build holds, with the command for each. The Java edition doesn't mutate Rust code, and the Rust
edition doesn't mutate Java, Kotlin or Gradle code.

The same goes for a coding agent's gate, which runs at the project's root: in a Gradle build, it
checks the mutants on the Java and Kotlin lines a task changes, not the crate's. To gate the
crate's changes, start the agent in the crate's directory.

## Everything goes through

The Java edition gets the command exactly as it was typed, without `--edition`, and its output and
exit status are the command's. Every command and option the Java edition has works through
`mutastic`, including those the Rust edition doesn't have, so a coding agent's plugin that runs
`mutastic gate check` works in either kind of project.

## The Java edition

The Java edition runs on JDK 25, found through `JAVA_HOME` or the `PATH`. mutastic checks for one
before it downloads anything, and says what it found if it isn't 25 or later.

It keeps each release of the Java edition in `~/.mutastic/mutastic-<version>`, as the Java
edition's own installer does, so a release installed that way is used as it is. A download is
checked against its release's `SHA256SUMS` before it's kept, and one that doesn't match is
refused. Without a version asked for, mutastic uses the newest release it has kept, and fetches
the latest when it has none.

| Variable | Effect |
|---|---|
| `MUTASTIC_VERSION` | The Java edition's release to use, such as `0.5.0`, fetched if it isn't kept |
| `MUTASTIC_HOME` | Where releases are kept: `~/.mutastic` unless set |
| `MUTASTIC_JAVA_ENGINE` | A Java edition's `bin/mutastic` to run instead, such as one built from its source. Nothing is fetched |
| `MUTASTIC_JAVA_RELEASES` | Where releases are downloaded from, for a mirror: GitHub's releases of `mutastic/mutastic` unless set |

To move to a newer Java edition, run once with `MUTASTIC_VERSION` set to it. Removing a release's
directory from `~/.mutastic` removes it.

## Which versions

`mutastic --version` says the command's version, and each edition's. Before the Java edition is
fetched:

<!-- output: reference/version -->
```text
mutastic 0.2.0
Rust edition 0.2.0, built in
Java edition: not fetched yet, which happens the first time mutastic runs in a Maven or Gradle project
```

Once it's fetched, the last line names its version and where it's kept.
