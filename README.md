# mutastic for Rust

*Find out whether your tests, or your agent's, actually check anything.*

> **Early days.** This is the Rust edition of [mutastic](https://mutastic.io), and it is not ready
> to use yet: it runs on single Cargo packages, with a handful of operators, and its output and ids
> may still change. Nothing is published to crates.io. The Java edition is at
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
mutastic verify <id> fixtures/pricing  # passes once a test catches that mutant
```

`--format` is `full` (cards, the default at a terminal), `concise`, `agent` or `jsonl` (the
`mutastic.events/v1` stream).

## Operators

The Java edition's ids: `mutastic/boundary`, `mutastic/remove-conditional`,
`mutastic/logical-connector`, `mutastic/math` and `mutastic/return-value` by default, and
`mutastic/other-comparison` (a comparison reversed) with `--operators=stronger`.

## Licence

[Apache 2.0](LICENSE).
