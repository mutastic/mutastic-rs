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

## Unsafe code

A mutant inside `unsafe` code can cause undefined behaviour, which no test can be said to catch or
miss, so mutastic leaves alone code inside `unsafe` blocks, `unsafe fn`s and `unsafe impl`s, and
safe functions that hold an `unsafe` block. The summary counts them as `not mutated: unsafe (N)`;
`--unsafe=enclosing` mutates those safe functions too, and `--unsafe=all` everything. Safe code
near unsafe code (called from inside an `unsafe` block, or in a module that has one) is mutated
but marked: its mutants run one test at a time, and a crash among them is reported as
`RuntimeError`, out of the score, not as caught. A safe mutant that only crashes the process is
caught, and says so: no test's check caught it.

Overflow checks and debug assertions are always on in mutastic's build, whatever the project's
profiles say, because they decide what an arithmetic mutant does; the build's settings are in the
`run-started` event.

## Operators

The Java edition's ids: `mutastic/boundary`, `mutastic/remove-conditional`,
`mutastic/logical-connector`, `mutastic/math` and `mutastic/return-value` by default, and
`mutastic/other-comparison` (a comparison reversed) and `mutastic/other-arithmetic` (`%` to `*`)
with `--operators=stronger`.

## Licence

[Apache 2.0](LICENSE).
