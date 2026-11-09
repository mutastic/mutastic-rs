# Contributing to mutastic for Rust

Thanks for helping. Bug reports, ideas and pull requests are all welcome.

## Reporting a bug or asking for something

Open an [issue](https://github.com/mutastic/mutastic-rs/issues/new/choose). For a bug, the form asks
for what helps most: `mutastic --version`, `rustc --version`, how you ran it (`mutastic` or `cargo
mutastic`), the command and what it printed. A small crate that shows the problem is best of all.

For a security problem, don't open an issue: see [SECURITY.md](SECURITY.md).

Everyone taking part in issues, pull requests and discussions agrees to the
[Code of Conduct](CODE_OF_CONDUCT.md). To report a breach of it, write to conduct@mutastic.io.

## Setting up

You need a stable Rust toolchain with `rustfmt` and `clippy`. `rust-version` in
[Cargo.toml](Cargo.toml) is the oldest that builds mutastic, and CI builds on it as well as on the
latest stable.

```sh
git clone https://github.com/mutastic/mutastic-rs.git
cd mutastic-rs
cargo test
```

CI runs these, on Linux and macOS, and each must pass:

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

`cargo test` builds and runs mutastic on the crates in `fixtures/`, so the first run takes a few
minutes; later ones reuse the fixtures' builds. CI also checks the dependencies' licences and
advisories with [cargo-deny](https://github.com/EmbarkStudios/cargo-deny), against
[deny.toml](deny.toml).

Useful along the way:

```sh
cargo test --test docs                       # one integration test file
cargo test --lib sites                       # the unit tests in one module
cargo run --bin mutastic -- run fixtures/pricing
```

[ARCHITECTURE.md](ARCHITECTURE.md) explains how the pieces fit, and `fixtures/` holds the small
crates the tests run mutastic on.

## Making a change

- **Follow [AGENTS.md](AGENTS.md).** It's the conventions the code is held to and why. Most are
  enforced by CI; the rest are judgement, written down.
- **mutastic's own rules:**
  - Its output never names other mutation-testing tools.
  - Every command it offers parses: the docs test checks each one a page shows.
  - A mutant's id never changes while its function's code doesn't. `tests/ids.rs` holds every
    fixture's mutants to the list in `crates/mutastic/tests/ids/ids.tsv`.
  - A catch it can't trust (a crash near unsafe code, a crash in std's own checks) doesn't count as
    caught.
  - A card says what its mutant does: `tests/truth.rs` and the docs test check it by running the
    mutants.
- **Tests:** a change in behaviour comes with a test that fails without it.
- **Docs:** user-facing changes update the pages in `docs/` (see [docs/README.md](docs/README.md)).
  After changing what mutastic prints, run `MUTASTIC_DOCS_WRITE=1 cargo test --test docs`, which
  writes the docs' copies of its output again, then read the diff.
- **Ids:** after adding an operator, or a shape an operator finds, run
  `MUTASTIC_IDS_WRITE=1 cargo test --test ids`. Lines may be added to the list, never changed or
  removed: a changed line is an id that broke.
- **Dependencies:** commit `Cargo.lock` with the change. A new dependency's licence must be one
  [deny.toml](deny.toml) allows.

## Pull requests

- One change per pull request, with a description of what it changes and why. The template asks.
- Commit messages explain why, as the log does: what was wrong or missing, and what the change does
  about it.
- CI must pass.

## Releases

There are none yet. The crate names `mutastic`, `cargo-mutastic` and `mutastic-rs` are reserved on
crates.io, and every crate here is `publish = false` until the first release. Until then, install
from source.

## Licence

mutastic is under the [Apache License 2.0](LICENSE). Contributions are accepted under the same
licence: by opening a pull request, you agree yours is offered under it.
