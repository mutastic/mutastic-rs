# Fuzzing

Each target feeds arbitrary input to something that reads what a user writes, and fails on a panic:
a panic there would end a whole run. A workspace of its own, built on nightly, as cargo-fuzz needs.

| Target | Reads |
|---|---|
| `config` | `mutastic.yaml` (`config::parse`) |
| `rules` | a rules file (`callrules::parse`) |
| `ignores` | `.mutastic/ignores.yaml` (`ignores::parse`) |
| `sites` | a crate's source, to find its mutants (`sites::find_with`, every operator, unsafe code included) |

```sh
cargo install cargo-fuzz
cargo +nightly fuzz run config -- -max_total_time=300
```

`rules` needs leak detection off: a rule's id is leaked on purpose, to live as long as the run, as
a process reads its rules once.

```sh
ASAN_OPTIONS=detect_leaks=0 cargo +nightly fuzz run rules -- -max_total_time=300 -detect_leaks=0
```

Seed a target with the files it reads, so it starts from real shapes: `fixtures/*/mutastic.yaml` for
`config`, `fixtures/configured/rules/*.yaml` for `rules`, any `.rs` under `fixtures/` for `sites`:

```sh
cargo +nightly fuzz run sites fuzz/corpus/sites fixtures/pricing/src
```

An input that panics is kept in `fuzz/artifacts/<target>/`. Fix the panic, then copy the input to
`crates/mutastic/tests/fuzzed/<target>/`, where `tests/fuzzed.rs` keeps it read without a panic on
every build.
