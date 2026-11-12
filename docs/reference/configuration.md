# Configuration

Some mutants are never worth a test: a function a code generator wrote, a `println!` that only
writes output. mutastic leaves some out on its own, and a project says which others to skip, always
with a reason, so the score is about the code you mean to test, and a person reviewing the skip can
see why it's there.

A run is set by its options (see [`mutastic run`](commands.md#run)), by `mutastic.yaml` in the package's directory,
and by a few environment variables. Nothing goes in `Cargo.toml`.

## `mutastic.yaml`

The project's own say over which mutants run, in the package's directory, in the Java edition's
format: the same keys, read as strictly, with Rust paths where the Java edition names classes. A
line mutastic doesn't understand is an error that names it, never a guess.

<!-- from: fixtures/configured/mutastic.yaml -->
```yaml
rules:
  - skip: configured::generated::**
    reason: written by a code generator
plugins:
  - rules/trims.yaml
operators:
  - defaults
  - -mutastic/return-value
```

| Key | What it does |
|---|---|
| `rules:` | Code to skip, each `- skip: <pattern>` with a `reason:` under it. Its mutants are skipped as ``user: <reason> (rule `skip: <pattern>`)``, out of the score |
| `plugins:` | Rules files of call operators, by their path from the package's directory |
| `operators:` | Which operators run, as `--operators` chooses, which replaces this for one run: a set (`defaults`, `stronger`, `extended`, `all`), a namespace (`stream`) or an operator's id, or one of those after `-` to leave it out |
| `filters:` | A compiled plugin's filters: the Rust edition has no compiled plugins yet, so naming one is an error |

A pattern is a path: `**` matches any depth, `*` one name, and a function is its path in the crate,
with the crate's name or `crate::` before it or not: `shop::Money::add`, `crate::Money::add` or
`Money::add`, where the Java edition writes `com.acme.Money#add`. A rule covers a function by
its path, or by the item that holds it (`shop::Money`), as a Java rule covers a method or its
class. A `mutate:` rule takes back an arid skip in the code it covers, as in the Java edition: a
`println!` or `log::warn!` that's a contract runs as a mutant there. It never takes back any other
skip.

## Rules files

A rules file is call operators as data, in the Java edition's format. Each list item is one
operator:

<!-- from: fixtures/configured/rules/trims.yaml -->
```yaml
- id: acme/remove-trim
  explanation: A call that trims a string is removed. It goes unnoticed when no test passes text with spaces around it.
  owners: str, String
  methods: trim, trim_start, trim_end
  rewrite: remove
  changed: `.{name}()` removed
  headline: no test passes {method} text with spaces for `.{name}()` to remove
  help: add a test whose input has spaces around it, and assert they're gone
```

| Field | What it says |
|---|---|
| `id` | `namespace/name`, lower case: how reports, `--operators` and ignores name it |
| `explanation` | What it changes, and when a test misses that: `mutastic explain` shows it |
| `tier` | `defaults` (unless it says), `stronger` or `extended` |
| `owners` | The types it's for, as Rust names them: `str`, `String` |
| `methods` | The method names it matches |
| `rewrite` | `remove`, so `name.trim()` becomes `name`, or `rename a->b, c->d` |
| `changed` | What the line now does, beside the caret. `{name}` is the method called, `{to}` what a rename calls instead |
| `headline` | A missed mutant's headline. `{method}` is the function the call is in |
| `help` | The test to write, with the same placeholders |

On the fixture, the rules file's operator finds the `trim()` no test gives spaces to, in its own
words:

<!-- output: reference/configured-card -->
```text
 [✗ MISSED] no test passes clean() text with spaces for .trim() to remove
   ┌─ src/lib.rs:6
   │
 5 │ pub fn clean(name: &str) -> String {
 6 │     name.trim().to_lowercase()
   │          ^^^^ .trim() removed
 7 │ }
   │
   = mutant  - name.trim().to_lowercase()
             + name.to_lowercase()
   = note: covered by tests::names_are_lower_case (its 1 covering test still passes)
   = help: add a test whose input has spaces around it, and assert they're gone

   done when  mutastic verify m06d09770 fixtures/configured passes
   reproduce  mutastic run --only=m06d09770 fixtures/configured
```

What differs from the Java edition: mutastic reads source without types, so a rule matches a call
by its method's name, and `owners` isn't checked. A rewrite that doesn't fit the receiver's type
doesn't compile, so the build withdraws it as unviable, at the cost of a build round. `descriptors`
and `since` are the JVM's, and an error here; of the rewrites, only `remove` and `rename` are in the
Rust edition yet.

## What mutastic reads

The environment variables mutastic reads, and the option each stands in for, are on
[Environment and exit codes](commands.md#environment). The order for
the format is `--format`, then `MUTASTIC_FORMAT`, then `MUTASTIC_AGENT`, then whether stdout is a
terminal.

In a Maven or Gradle project, `MUTASTIC_VERSION`, `MUTASTIC_HOME`, `MUTASTIC_JAVA_ENGINE` and
`MUTASTIC_JAVA_RELEASES` say which Java edition runs, and where it's kept: see
[One command, two editions](../editions.md#the-java-edition).

## Ignoring one mutant

`mutastic ignore` records a decision not to test one mutant in `.mutastic/ignores.yaml` in the
package's directory, with the line it's on and the reason, to commit and review. The mutant is
skipped (`user: … (ignored)`) while its line stays as it was, and the summary counts it by the sort
`--as` gave it (`1 ignored as unreachable`), so what ignores hide can be reviewed by sort. Once the
line changes, the decision no longer holds, the mutant runs again, and `run-finished` lists it in
`staleIgnores`. Every card offers it as `or ignore`; a coding agent proposes one instead, in
`.mutastic/proposals.yaml`, for a person to accept.

```sh
mutastic ignore mb2f8ae57 --reason="totals are rounded to tens" --as=unreachable fixtures/pricing
```

The files are the Java edition's, in the same shape, with the function's path in `at`
(`pricing::Pricing::discount, line 26`). See
[`mutastic ignore`](commands.md#ignore) for the command and its options.

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
| `target/mutastic/last-run.tsv` | The last full run, for the next to compare with |
| `target/mutastic/last-runs/` | The last full run under each other set of operators |

`cargo clean` removes it with the rest. A `CARGO_TARGET_DIR` or `build.target-dir` you've set moves
it with your target directory.
