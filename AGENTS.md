# Working on mutastic for Rust

mutastic for Rust is the Rust edition of mutastic, a mutation-testing tool. Read the
[README](README.md) for what it does and [ARCHITECTURE.md](ARCHITECTURE.md) for how it's built.
This file is the conventions the code is held to, and why. Some are enforced by CI; the rest are
judgement, written down so they're designed around rather than met as a failure in review.

## Build

```sh
cargo fmt --all                            # format the code
cargo clippy --all-targets -- -D warnings  # every lint, as errors
cargo test                                 # unit tests, then mutastic run on the fixtures
MUTASTIC_DOCS_WRITE=1 cargo test --test docs  # after changing what mutastic prints or takes: the docs' copies
MUTASTIC_IDS_WRITE=1 cargo test --test ids    # after adding an operator or a shape: the ids list
cargo run --bin mutastic -- run fixtures/pricing
```

CI runs the first three on Linux and macOS, builds on the oldest toolchain `rust-version` names,
and checks the dependencies with cargo-deny (`deny.toml`).

## Code conventions

Formatting is rustfmt's, with its defaults: that is the spec, not taste.

1. Every warning fails the build. A warning nobody acts on hides the next one.
2. One formatter. Layout is not a conversation.
3. A snippet must stand alone. Code is read without its imports or its file — in a review, a
   chat, an article. Prefer the form that survives being torn out.
4. Limits are set where the code already is, then held. Never lower a floor (a score, a count a
   test expects, a threshold) to make a build pass: fix what moved it.
5. When something grows past what reads well, split it and name the pieces.
6. When you allow a lint, write the reason beside the `#[allow]`, not in a config file.
7. Name and structure carefully; let the formatter handle whitespace. Identifiers and shape
   change what a reader and a model understand. Indentation changes neither.
8. Boundaries are asserted in tests, not described in comments.
9. A test that skips when its inputs are missing has checked nothing. Let the everyday task
   skip; make CI run one that fails.
10. A line should read as a sentence. Name locals for what they hold.
11. Name what you'd otherwise repeat or imply. A call made three times becomes a local; a long
    iterator chain inside a bigger function becomes a function named for what it returns.
12. Say what the collection is, and keep its order steady. `contains` followed by `insert` means a
    set. Output follows the order of a `BTreeMap` or `BTreeSet`, or an explicit sort, never a
    `HashMap`'s.
13. Name every regex group, including the ones nothing reads back. Never read a group by number.
14. One concern per type.
15. No unsafe code in mutastic itself (`unsafe_code = "forbid"`). Take a dependency, or a process,
    before reaching for it.

## mutastic's own rules

- **Output never names other mutation tools.** Cards, help text and events describe operators in
  mutastic's own terms. Comparisons belong in code comments and notes.
- **A command the output offers exists.** The docs test parses every `mutastic` command a page
  shows with `cli::check`, and every id it names must be a mutant of its fixture.
- **A command takes what it declares, and the docs say so.** Every command, option, environment
  variable and exit status is declared once, in `src/commands.rs`. The parser takes an option only
  on the commands it names there, and `docs/commands.json`, which the docs site's page for each
  command is written from, and `docs/reference/commands.md`, the same reference for GitHub, are
  written from the same declarations: the docs test fails while either is stale. Link a command
  there (`commands.md#run`), never to the docs site's Rust section until it's published.
- **Ids never change.** A mutant's id is a hash of what it is, never its line. A new operator,
  filter or output must not move an existing id: `tests/ids.rs` holds every fixture's mutants to
  `tests/ids/ids.tsv`, and the list only grows.
- **A false catch is worse than a miss.** Anything that can make a mutant look caught when no test
  caught it (a crash near unsafe code, std's own check of an unsafe precondition) is not counted as
  caught, and said so.
- **Unsafe code is left alone by default.** A mutant that can cause undefined behaviour proves
  nothing either way. Mutating more of it is the user's choice, with `--unsafe`.
- **A card says what its mutant does.** People and agents both act on a card's words, so a wrong
  one costs an agent's tokens, or a wrong test, as well as trust. `tests/truth.rs` writes each
  card's claim into the source and checks it behaves as the mutant does, for every shape in
  `fixtures/truth`: a new shape an operator finds goes in that fixture. The docs test does the same
  for every missed mutant on `fixtures/operators`.
- **The docs show what mutastic prints.** Every output block in `docs/` and the README is a kept
  copy of a real run, in colour in `docs/output/*.ansi` for the docs site and without it on the
  page for GitHub, and every Rust example is a fixture's own code. Change what's printed, then
  write the copies again and read the diff.
- **The words are the Java edition's.** Where both editions say something, say it the same way, so
  the agent plugins and their users read one product. Where Rust needs different words (crashes,
  unviable mutants, unsafe code), say so in `docs/differences-from-java.md`.
- **Keep tests cheap.** Build each fixture once and run it once per test where you can; put many
  claims in one build, as the truth tests do, rather than a build per claim.
- **Check claims against real crates,** not only the fixtures: the bugs that matter are found
  there.
