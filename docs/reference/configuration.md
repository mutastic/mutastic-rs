# Configuration

The Rust edition has no configuration file yet: a run is set by its options (see
[Commands](cli.md)) and by a few environment variables. Nothing goes in `Cargo.toml`.

## What mutastic reads

| Variable | Effect |
|---|---|
| `MUTASTIC_FORMAT` | The format when `--format` isn't given: `full`, `concise`, `agent` or `jsonl` |
| `MUTASTIC_AGENT=1` | The `agent` format, when neither `--format` nor `MUTASTIC_FORMAT` chooses one. Coding agents' plugins set it |
| `NO_COLOR` | No colour, even at a terminal |
| `FORCE_COLOR` | Colour, even when stdout isn't a terminal |
| `TERM=dumb` | No colour |
| `CARGO` | The `cargo` to run, as Cargo sets it for `cargo mutastic`; `cargo` on the `PATH` otherwise |
| `RUSTC` | The `rustc` Cargo builds with, as for any Cargo build, and whose version the result names |

The order for the format is `--format`, then `MUTASTIC_FORMAT`, then `MUTASTIC_AGENT`, then
whether stdout is a terminal.

## What your tests see

mutastic runs your test binaries itself. In each run it sets:

| Variable | Value |
|---|---|
| `MUTASTIC` | `1`, so a test that needs to can tell it's running under mutastic |
| `CARGO_MANIFEST_DIR` | The package's directory in mutastic's copy, as `cargo test` would set it |
| `RUST_BACKTRACE` | `0`, so panics stay short |

Tests run with the package's directory in the copy as their working directory, as `cargo test`
runs them in the package's directory, so a test that reads `tests/data.txt` by a relative path finds
it. `MUTASTIC_ACTIVE` and `MUTASTIC_COVERAGE` are mutastic's own, for the switch: don't set them.

## Where mutastic keeps things

Everything is under your target directory, in `target/mutastic`:

| Path | Holds |
|---|---|
| `target/mutastic/copy` | The copy of your workspace with the mutants written in |
| `target/mutastic/target` | The copy's build, kept warm between runs |
| `target/mutastic/coverage` | What each test reached in the last run |

`cargo clean` removes it with the rest. A `CARGO_TARGET_DIR` or `build.target-dir` you've set moves
it with your target directory.
