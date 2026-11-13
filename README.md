# mutastic for Rust

<img src=".github/pip.webp" alt="Pip, mutastic's watermelon mascot, looking determined" width="150" align="right">

*Find out whether your tests, or your agent's, actually check anything.*

[![CI](https://github.com/mutastic/mutastic-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/mutastic/mutastic-rs/actions/workflows/ci.yml)
[![Licence](https://img.shields.io/badge/licence-Apache%202.0-blue)](LICENSE)

This is the Rust edition of [mutastic](https://mutastic.io), a mutation-testing tool. It makes small
changes to your code, such as `>=` to `>`, and runs your tests against each one. A change no test
notices is a **missed mutant**: a place your tests run but don't check. For each one, mutastic shows
the inputs your tests used, what a test would need to check, and the command that proves the fix.

**[Website](https://mutastic.io/)** ·
[Getting started](docs/getting-started.md) ·
[Documentation](docs/README.md) ·
[Commands](docs/reference/commands.md) ·
[Java edition](https://github.com/mutastic/mutastic)

## Why

Coverage says a line ran, not that a test would notice if it were wrong. A suite can reach every
line and still pass with `>=` turned into `>`, and tests a coding agent writes to get a task done
can do the same. mutastic finds those places, says which input would tell the code apart from the
change, and gives a command that passes once a test does. You, or your agent, can close the gap
while still in the code, rather than finding it in review.

Every mutant goes into one copy of your crate, behind a switch, so a single build holds them all
and each test run picks the live one. Your source, `Cargo.toml` and `target/` are left alone.

## What it looks like

On `fixtures/pricing`, a small crate in this repository whose tests reach every line:

<!-- output: readme/card -->
```text
  ✗ MISSED  no test checks discount() at total == 100
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

     Score  90.9%   PASS  1 missed, above the threshold of 80.
```

Paste the test the card wrote, once you've checked today's value is the one you want, then prove
it:

```sh
mutastic verify mb2f8ae57 fixtures/pricing
```

<!-- output: getting-started/verify -->
```text
mutastic: mb2f8ae57 is caught by tests::discount_at_the_total_edge
```

## Install

On macOS or Linux, the installer puts the latest
[release](https://github.com/mutastic/mutastic-rs/releases) in `~/.local/bin`:

```sh
curl -fsSL https://mutastic.io/install.sh | sh
```

On Windows, in PowerShell, it puts the latest release in `%USERPROFILE%\.mutastic\bin` and adds
that to your user `Path`:

```powershell
irm https://mutastic.io/install.ps1 | iex
```

That installs `mutastic`, and `cargo-mutastic`, which Cargo runs as `cargo mutastic`. They do the
same thing. Scoop and cargo-binstall install it too.

It isn't on crates.io yet. Elsewhere, or to work on mutastic, build it yourself: you need a stable
Rust toolchain and nothing else, and `rust-version` in [Cargo.toml](Cargo.toml) is the oldest that
builds it.

```sh
git clone https://github.com/mutastic/mutastic-rs.git
cd mutastic-rs
cargo install --path crates/mutastic
```

[Installing mutastic](docs/install.md) has the details.

`mutastic` is the command for Maven and Gradle projects too: in one, it fetches the Java edition
the first time, and hands it the command. See [One command, two editions](docs/editions.md).

## Quick start

From the repository's root, on the crate above:

```sh
mutastic run fixtures/pricing
```

On your own crate, run `cargo mutastic run` inside it, or `mutastic run path/to/crate`. Each card
ends with the commands for its mutant: `done when` passes once a test catches it, and `reproduce`
runs it alone. `mutastic next` hands the missed mutants over one at a time, the one most worth a
test first. `mutastic mutants` lists the mutants without building or running anything.

[Getting started](docs/getting-started.md) walks through the card, the test, and the proof.

## Documentation

[docs/](docs/README.md) has the rest, written to move into the Rust section of
[docs.mutastic.io](https://docs.mutastic.io):

- [How a run works](docs/concepts/how-a-run-works.md), [results and the score](docs/concepts/results.md),
  [unsafe code](docs/concepts/unsafe.md), [profiles](docs/concepts/profiles.md),
  [mutant ids](docs/concepts/ids.md) and [working through misses](docs/concepts/next.md);
- [operators](docs/operators.md), with a page and a real card for each family;
- [commands](docs/reference/commands.md), [report files](docs/reference/reports.md), [events](docs/reference/events.md) and
  [configuration](docs/reference/configuration.md);
- [troubleshooting](docs/troubleshooting.md) and the
  [differences from the Java edition](docs/differences-from-java.md).

Every output those pages and this one show comes from a run on the repository's fixtures, and
`cargo test` fails when it no longer matches what mutastic prints.

## Status

The operators and their names, the cards' words and the
`mutastic.events/v1` stream are the Java edition's. It runs one Cargo package at a time, with that
package's own tests, and leaves unsafe code alone unless asked. Some of the Java edition's commands
and options aren't here yet: [Differences from the Java edition](docs/differences-from-java.md#not-in-the-rust-edition-yet)
says which. What stays the same from one release to the next is on the
[Compatibility](docs/reference/compatibility.md) page. Releases are on GitHub; nothing is published
to crates.io yet, where the names are reserved.

## Contributing

Issues and pull requests are welcome: see [CONTRIBUTING.md](CONTRIBUTING.md). In short,
`cargo test` runs the tests CI runs; [AGENTS.md](AGENTS.md) has the conventions the code is held
to, and [ARCHITECTURE.md](ARCHITECTURE.md) how it's built. To report a vulnerability, see
[SECURITY.md](SECURITY.md). Everyone taking part agrees to the
[Code of Conduct](CODE_OF_CONDUCT.md).

## Licence and citation

[Apache License 2.0](LICENSE). To cite mutastic for Rust, use [CITATION.cff](CITATION.cff), or
GitHub's *Cite this repository*.
