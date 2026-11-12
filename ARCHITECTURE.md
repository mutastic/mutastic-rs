# Architecture

How mutastic for Rust is built, for people working on it. To use it, see the [docs](docs/README.md);
for the conventions the code is held to, [AGENTS.md](AGENTS.md).

## How a run works

1. **The project.** `workspace` reads the package from `cargo metadata`: its targets, its features
   and the workspace around it. A workspace's root that isn't itself a package is refused, with
   the members named. `cfg` asks rustc which `cfg` predicates hold for the target (`rustc --print
   cfg`) and follows the package's default features, so code this build doesn't have is counted
   rather than mutated. A predicate rustc doesn't know for every target is never taken as off.
2. **Mutants.** `sites` parses every file reachable from the package's targets with
   rust-analyzer's parser (`ra_ap_syntax`), from syntax alone, with no type information. It finds
   each site for each operator the run uses: the defaults in `sites.rs`, and `stronger` and
   `extended` in `sites/stronger.rs`, `sites/rollback.rs`, `sites/operators.rs` and
   `sites/extended.rs`. Only function bodies are mutated, and never test code. Unsafe code is
   fenced off by its syntax (`UnsafeMode`): left alone, or mutated and marked near unsafe. A
   mutant's id (`id`) is a hash of what it is, without its line, so it stays the same while code
   around it moves. Some mutants are made but skipped, with a reason by kind (`equivalent:`), so
   they keep their ids but stay out of the score.
3. **One build.** `workspace` copies the workspace to `target/mutastic/copy`, writing a file only
   when its bytes change, so Cargo's freshness checks keep the next build short. `schema` writes
   every mutant into the copy, each site a `match` on a switch whose last arm is the original code,
   without adding a line, and puts the switch's runtime at the end of the crate root. `compile`
   builds the copy: `cargo check --tests`, then `cargo build --tests`, with mutastic's own profile
   (overflow checks and debug assertions on). It blames each compile error on the mutants it lies
   in, withdraws them as unviable, and builds again. Flags reach only the user's crate, through
   `RUSTC_WORKSPACE_WRAPPER`, which is the mutastic binary itself (`wrapper`), never `RUSTFLAGS`.
4. **Coverage.** `testrun` lists the tests in each built test binary and runs each test alone with
   the switches recording which sites it reaches and the whole numbers each function was called
   with: the "inputs seen" on a card. Every test must pass here: one that fails runs again on its
   own, and if it then passes it's flaky, left out of the run, with what it reaches that varies
   (`flaky`); one that fails every time stops the run.
5. **Mutants, side by side.** `engine` runs each mutant against only the tests that reach its site,
   the fastest binary first, with `MUTASTIC_ACTIVE` naming it, each run a process of its own under a
   time limit, on `--workers` threads. The first failing test catches it. A process that dies
   without a test failing is run again, one test at a time, to see how: in safe code that's caught
   by the crash, and near unsafe code, or in std's own check of an unsafe precondition, it's a
   `RuntimeError`, out of the score. Near-unsafe mutants run with one test thread.
6. **Suggested tests.** For each missed mutant whose function a test can call with whole numbers
   alone (`sites/callable.rs`), `suggest` runs the probe `schema` built beside that function: a
   test of mutastic's own, left out of the test list, that calls it on candidate inputs, as the
   code is and with the mutant in. The first input where they differ, and both values, go on the
   card. A probe that doesn't compile is dropped, not blamed on a mutant.
7. **The report.** `report` prints the result in one of four formats: `full` cards for people,
   `concise` lines, `agent` text for coding agents, and `jsonl`, the `mutastic.events/v1` stream.
   `words` writes what a card says about a mutant: the gap, the change and the fix, in the Java
   edition's words.

`cli` is the `mutastic` command, and `cargo mutastic` runs the same `cli::main`. `run`, `verify`,
`explain` and `next` share the first three steps, and `mutants` stops after the second. `next` runs
every mutant once, then only those it left uncaught while the code under test is the same, and
prints the misses as a queue (`report/next.rs`).

## Modules

All the code is in `crates/mutastic/src`:

| Module | What it does |
|---|---|
| `cli` | The command: parses options, picks the format, runs the engine, sets the exit status |
| `commands` | Every command, option, environment variable and exit status, declared once: what the parser takes, `gate --help`, `docs/commands.json` and `docs/reference/commands.md` |
| `workspace` | Reads the package from `cargo metadata`; keeps mutastic's copy in step |
| `cfg` | Which `cfg` predicates hold in mutastic's build: built, not built, or not known |
| `sites` | Finds mutants with rust-analyzer's parser; the operators, their sets, and unsafe fencing; `sites/callable.rs`, whether a test can call a function, and how |
| `id` | Mutant ids and fingerprints |
| `schema` | Writes every mutant into the copy behind a switch, and the switch's runtime |
| `compile` | Builds the copy, blames compile errors on mutants, withdraws the unviable ones |
| `wrapper` | mutastic as Cargo's `RUSTC_WORKSPACE_WRAPPER`, adding flags to the user's crate only |
| `testrun` | Lists and runs libtest binaries' tests, with a time limit, and runs work side by side |
| `flaky` | Flaky tests: how many reruns, what a test reaches that varies, and the words for a test that always fails |
| `engine` | A run: finds, builds, records coverage, runs each mutant, and decides its status |
| `words` | What a card says: the gap, the change, the inputs seen and the help |
| `suggest` | Suggested tests: candidate inputs, running the probes, and the test written out |
| `report` | The formats: `full`, `concise`, `agent` and `jsonl`; `report/next.rs`, `mutastic next`'s queue, its order and `--top`, in each format |
| `nextrun` | What the last `mutastic next` left uncaught, and the hash of the code under test, its test code taken out, that says whether only those run again |
| `shares` | A run split across machines: which share a mutant is in (`--shard`), a share's events, and the whole run put back together from them (`--from-shards`) |

`crates/cargo-mutastic` and `crates/mutastic-rs` hold the other two names reserved on crates.io.
They're empty, and `publish = false`.

## What it shares with the Java edition

The two editions share a contract, not code: the operators' ids (`mutastic/boundary` means the same
change in both), the cards' words, the score, and the `mutastic.events/v1` schema. A copy of the
schema, pinned from the Java repository, is in `crates/mutastic/tests/schema/`, and the tests
check the events mutastic prints against it. Every event says `"edition": "rust"`. Where the editions differ,
[docs/differences-from-java.md](docs/differences-from-java.md) says how.

## Tests

| Where | What it holds |
|---|---|
| `#[cfg(test)]` modules in `src/` | Unit tests, beside the code |
| `tests/samples.rs` | `mutastic run` on the fixtures, through the binary, as a person runs it |
| `tests/truth.rs` | A card says what its mutant does: each claim written into the source behaves as the mutant does |
| `tests/shares.rs` | Two shares of the pricing fixture, put back together, are the whole run, in every format; shares that aren't one run's are refused |
| `tests/ids.rs` | Every mutant of every fixture, under every operator, held to `tests/ids/ids.tsv` |
| `tests/docs.rs` | Every output block in the docs and the README is what mutastic prints; every link, command, id and Rust example in them is checked; every missed card on the operators fixture is true |

`fixtures/` holds the crates they run on, each a package of its own outside mutastic's workspace:
`pricing` (the Java edition's sample, ported), `operators` (an example of each operator), `truth`
(each shape a mutated expression can take), `unsafety`, `loops`, `places`, `platforms`, `ranking`,
`suggestions`, `repeats` (identical lines in one function), `flaky` (a test that fails on its first
try, and passes after) and `workspace`. `samples.rs` and
`truth.rs` run mutastic on them in place, so its copy goes in each fixture's own `target/`;
`docs.rs` copies them to the system's temporary directory first, so the commands it captures read
as a person types them at the repository's root.
