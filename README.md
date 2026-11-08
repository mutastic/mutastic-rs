# mutastic for Rust

*Find out whether your tests, or your agent's, actually check anything.*

> **Early days.** This is the Rust edition of [mutastic](https://mutastic.io), and it is not ready
> to use yet: it runs on single Cargo packages, and its output and ids may still change. Its operators
> match the Java edition's. Nothing is published to crates.io. The Java edition is at
> [mutastic/mutastic](https://github.com/mutastic/mutastic).

mutastic makes small changes to your code, such as `>=` to `>`, and runs your tests against each
one. A change no test notices is a **missed mutant**: a place your tests run but don't check. For
each one, mutastic shows the inputs your tests used and the command that proves the fix.

## How it works

Every mutant is written into one copy of your crate, each behind a switch, so a single build holds
them all and an environment variable picks the live one. Mutants that don't compile are rolled back
and the copy builds again. The copy and its build live in `target/mutastic`, on your own stable
toolchain; your source, `Cargo.toml` and `RUSTFLAGS` are never touched.

## Try it

```sh
cargo install --path crates/mutastic
mutastic run fixtures/pricing          # or: cargo mutastic run, inside a package
mutastic verify mb2f8ae57 fixtures/pricing  # passes once a test catches that mutant
```

[Getting started](docs/getting-started.md) walks through it: the card, the test, and the proof.

## Docs

[docs/](docs/README.md) has the rest, written to move into the Rust section of
[docs.mutastic.io](https://docs.mutastic.io):

- [How a run works](docs/concepts/how-a-run-works.md), [results](docs/concepts/results.md),
  [unsafe code](docs/concepts/unsafe.md), [profiles](docs/concepts/profiles.md) and
  [mutant ids](docs/concepts/ids.md);
- [operators](docs/operators.md), with a page and a real card for each family;
- [commands](docs/reference/cli.md), [events](docs/reference/events.md) and
  [configuration](docs/reference/configuration.md);
- [troubleshooting](docs/troubleshooting.md) and the
  [differences from the Java edition](docs/differences-from-java.md).

Every output those pages show comes from a run on the repository's fixtures, and `cargo test` fails
when it no longer matches what mutastic prints.

## Licence

[Apache 2.0](LICENSE).
