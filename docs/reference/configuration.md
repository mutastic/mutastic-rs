# Configuration

Some mutants are never worth a test: a function a code generator wrote, a `println!` that only
writes output. mutastic leaves some out on its own, and a project says which others to skip, always
with a reason, so the score is about the code you mean to test, and a person reviewing the skip can
see why it's there.

A run is set by its options (see [`mutastic run`](commands.md#run)), by the package's nearest
`mutastic.yaml`, and by a few environment variables. Nothing goes in `Cargo.toml`.

## `mutastic.yaml`

The project's own say over which mutants run, in the package's directory or above it, in the Java
edition's format: the same keys, read as strictly, with Rust paths where the Java edition names classes. A
line mutastic doesn't understand is an error that names it, never a guess.

<!-- from: fixtures/configured/mutastic.yaml -->
```yaml
rules:
  - skip: configured::generated::**
    reason: written by a code generator
plugins:
  - rules/trims.yaml
  - rules/rewrites.yaml
operators:
  - defaults
  - -mutastic/return-value
```

| Key | What it does |
|---|---|
| `extends:` | Another `mutastic.yaml` this one adds to, by its path from this file's directory: see [In a workspace](#in-a-workspace) |
| `rules:` | Code to skip, each `- skip: <pattern>` with a `reason:` under it. Its mutants are skipped as ``user: <reason> (rule `skip: <pattern>`)``, out of the score; `- mutate: <pattern>` takes back arid skips, and `- output: <pattern>` names your own output calls, both below |
| `plugins:` | Rules files of call operators, by their path from the directory of the file that names them |
| `operators:` | Which operators run, as `--operators` chooses, which replaces this for one run: a set (`defaults`, `stronger`, `extended`, `all`), a namespace (`stream`) or an operator's id, or one of those after `-` to leave it out |
| `exclude-tests:` | Tests every run leaves out, each `- test: <name>` with a `reason:` under it: see [Leaving a test out](#leaving-a-test-out) |
| `gate:` | What the gate holds an agent's task to: `operators:` under it, a list as `operators:` takes, chooses the gate's operators in its place, and `gate open --operators` replaces it. See [Which operators](../test-first-agent.md#which-operators) |
| `timeout-ceiling:` | The most one mutant's tests may take, a whole number with its unit (`30m`, `90s`, `2h`), in place of ten minutes or twenty times the suite's own time. A mutant stopped there is timed out, with a note that says so. A package's file sets it over the one it extends. See [How long a mutant's tests may run](../troubleshooting.md#how-long-a-mutants-tests-may-run) |
| `filters:` | A compiled plugin's filters: the Rust edition has no compiled plugins yet, so naming one is an error |

A pattern is a path: `**` matches any depth, `*` one name, and a function is its path in the crate,
with the crate's name or `crate::` before it or not: `shop::Money::add`, `crate::Money::add` or
`Money::add`, where the Java edition writes `com.acme.Money#add`. A rule covers a function by
its path, or by the item that holds it (`shop::Money`), as a Java rule covers a method or its
class. A `mutate:` rule takes back an arid skip in the code it covers, as in the Java edition: a
`println!` or `log::warn!` that's a contract runs as a mutant there. It never takes back any other
skip. Each arid rule, what it can hide and the rule that takes it back are in
[Arid rules](https://docs.mutastic.io/reference/arid/), and why mutastic skips them at all in
[Arid mutants](https://docs.mutastic.io/concepts/arid/).

Every arid skip is one rule's, by the id the Java edition uses: `mutastic:output` for a statement
that only writes output or a value worked out only for one, `mutastic:null-checks` for a
`x.unwrap();` that only checks the value is there, `mutastic:waits` for a sleep removed, and so on
through the [Arid rules](https://docs.mutastic.io/reference/arid/). An `arid:` list on a `mutate:`
rule takes back only those rules' skips:

```yaml
rules:
  - mutate: shop::audit::record
    arid: [mutastic:output]
    reason: the audit log is what the auditor reads
```

An `output:` rule names a function or a macro of your own that only writes output: the wrapper
your team logs through, an audit log, a metric no test reads. mutastic knows the common output
macros, not those, and without a rule each of their calls is a mutant only a test of the log would
catch:

```yaml
rules:
  - output: crate::audit::record
    reason: the audit trail, which ops read and tests don't
```

A statement that calls it is skipped under `mutastic:output`, as `arid: only calls audit::record
(the audit trail, which ops read and tests don't)`. The path in the code may be shorter than the
rule's, `audit::record` or `record` after a `use`: its segments are matched against the end of the
rule's. A `mutate:` rule takes the skip back like any other of `mutastic:output`, and nothing is
output until a rule says so, since some teams test their metrics. The Java edition's rule names
methods, `com.acme.AuditLog#*`.

An id mutastic doesn't have is an error that names the one meant, when it's close. `mutastic
explain` on a skipped mutant names its rule, links to its section of
[Arid rules](https://docs.mutastic.io/reference/arid/) and prints the rule to copy.

An editor can check the file as you write it: both editions' `mutastic.yaml` follow one [JSON
Schema](https://docs.mutastic.io/schemas/mutastic-yaml/v1.json). With the YAML language server, a
first line names it:

```yaml
# yaml-language-server: $schema=https://docs.mutastic.io/schemas/mutastic-yaml/v1.json
```

The schema takes `filters:`, which the Rust edition refuses until it has compiled plugins. It
doesn't have `timeout-ceiling:` yet, which only the Rust edition reads: an editor checking the
file against it flags the key.

### Leaving a test out

A test that fails on its own, with no mutant in, stops the run: mutastic can't tell a mutant it
catches from one it would fail on anyway. Fix it if you can. Until then, `exclude-tests:` leaves it
out of every run, with the reason a reviewer reads:

```yaml
exclude-tests:
  - test: tests::shipping_is_free_from_the_threshold
    reason: expects the old threshold until the pricing change lands
```

A test is named as `--exclude-tests` takes it: by its name as a run shows it, or a module of tests
by its path, which leaves out every test in it. A test without a `reason:` is an error. The
summary's `Left out` row names each test with its reason, the Score row ends in `· 1 test left
out`, and `run-finished` lists them as `testsLeftOut`. A left-out test can't make a mutant look
caught: what only it reaches counts as not covered, so the score can only fall. A run with other
tests left out is other code than one that ran them, never compared as the same. `--exclude-tests`
adds to the file's tests for one run. With `extends:`, the file's own come first, then the other
file's, and a test named in both keeps this file's reason.

Leaving a test out is a person's call: an agent's output never offers it, and `mutastic gate`
holds a task that adds one as a task that took a test away.

### In a workspace

Each package uses the nearest `mutastic.yaml` at or above its own directory, however it's run:
`mutastic run` on a workspace's top that runs `shop`, and `mutastic run shop`, both read
`shop/mutastic.yaml` for `shop` when there is one, and the top's when there isn't. The search stops
at the top of the git repository, or outside one at the directory the run is on, so a stray file
above the project is never read.

One file applies, never a cascade: a package's file replaces the top's for that package. To add to
the top's instead, say so with `extends:`, a path from the file's own directory:

```yaml
# shop/mutastic.yaml
extends: ../mutastic.yaml
rules:
  - skip: shop::legacy::**
    reason: on its way out, replaced in 2.0
```

The package's rules come first, then the top's; the plugins of both apply, each rules file found
from the file that names it; and the package's `operators:`, if it has any, replace the top's. A
file names one other, which may extend another in turn.

Ignores are the run's: `.mutastic/` in the directory the run is on.

### Arguments for Cargo

A package that needs an argument to build as you test it, a feature to turn on, say, needs it when
mutastic builds it too. Put it after `--`:

```sh
mutastic run -- --features postgres
```

Everything after the first `--` goes to Cargo, word for word, each time mutastic checks or builds the
package, and never to the tests. `--features`, `--all-features` and `--no-default-features` choose
which code is mutated too. Every command takes it, and each command a run offers to copy ends in it.

To give the same to every run, name it in `mutastic.yaml`, or set `MUTASTIC_BUILD_ARGS`, split at
spaces, with quotes around an argument that has some:

```yaml
build:
  args:
    - --features=postgres
    - --offline
```

The first given is used whole, in place of the others: the command line, the variable, the file. A
bare `--` gives none. Of a file and the one it extends, the nearer that has `args:` is used.

**A secret goes in `MUTASTIC_BUILD_ARGS`, or Cargo's own settings.** What you type after `--` is in
the commands mutastic prints, and so in reports and CI logs that show them. The variable's and the
file's are never repeated.

**For the gate, use the variable or the file.** `gate open -- …` gives its arguments to its count of
the tests, and keeps them nowhere, so a hooked `gate check` doesn't have them.

mutastic refuses an argument that fights how it drives Cargo, and says why: one that chooses the
package, the targets, the tests, the profile or where it's built, that stops the build, or that
changes the output it reads.

A run with other arguments than the last is never compared as the same code. In CI, keep
`target/mutastic/build-args.key` with `last-run.tsv` and `last-runs/` for that: without it, a run with
arguments is compared as `code changed` every time, so its likely flaky tests aren't found between
runs. Whoever has both the key and the record can try guesses at a short secret among the arguments,
so where others can read the cache, leave the key out, or keep secrets out of the arguments. Shares of
a split run don't need it.

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
| `rewrite` | What the call becomes: [see below](#rewrites) |
| `changed` | What the line now does, beside the caret. `{name}` is the method called, `{to}` what's called instead |
| `headline` | A missed mutant's headline. `{method}` is the function the call is in |
| `help` | The test to write, with the same placeholders |

On the fixture, the rules file's operator finds the `trim()` no test gives spaces to, in its own
words:

<!-- output: reference/configured-card -->
```text
  ✗ MISSED  no test passes clean() text with spaces for .trim() to remove
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

### Rewrites

| Rewrite | Does | For example |
|---|---|---|
| `remove` | Drops the call and its arguments, and keeps what it was called on as the result | `name.trim().len()` becomes `name.len()` |
| `rename a->b, c->d` | Calls another method with the same arguments; each `a->b` is one mutant | `rename trim_start->trim_end` |
| `keep argument` | What the call was given goes on in its place. Only for a call of one argument: any other gets no mutant | `set.unwrap_or(8080)` becomes `8080` |
| `unchecked arithmetic` | Makes a `checked_` method the `wrapping_` one, in `Some`, and a `saturating_` method the `wrapping_` one: it wraps around instead of stopping. A method of another name gets no mutant | `a.checked_add(b)` becomes `Some(a.wrapping_add(b))` |
| `call name(TYPES) with A, B` | Drops the arguments and calls another method on the same value, with these. `TYPES` are the method's parameter types, for whoever reads the file: mutastic doesn't check them | `call take(usize) with usize::MAX` |

A `call` rewrite's arguments are numbers (`0`, `-1`, `0.5`), `true`, `false`, `None`, a constant by
its path (`usize::MAX`) or a call of no arguments (`String::new()`); leave out `with` to call the
method with none. In the words, `{to}` is what's called instead: a rename's other method, the
`wrapping_` method, or a `call` rewrite's `take(usize::MAX)`.

This rules file asks for the other three, each on a call the fixture makes:

<!-- from: fixtures/configured/rules/rewrites.yaml -->
```yaml
- id: acme/always-fallback
  explanation: A call that gives a fallback for a value that may be missing gives the fallback every time. It goes unnoticed when no test has the value there.
  owners: Option
  methods: unwrap_or
  rewrite: keep argument
  changed: `.{name}()` always gives its fallback
  headline: no test gives {method} a value for `.{name}()` to prefer to its fallback
  help: add a test where the value is there, and assert it's the one used

- id: acme/no-limit
  explanation: A limit on how many items an iterator gives is lifted. It goes unnoticed when no test has more items than the limit.
  owners: Iterator
  methods: take
  rewrite: call take(usize) with usize::MAX
  changed: `.{name}()` changed to `.{to}`
  headline: no test gives {method} more items than `.{name}()` lets through
  help: add a test with more items than the limit, and assert how many come back

- id: acme/wrap-on-overflow
  explanation: A checked or saturating operation wraps around instead. It goes unnoticed when no test has a result too big for its type.
  owners: u8, u32
  methods: checked_add, saturating_add
  rewrite: unchecked arithmetic
  changed: `.{name}()` changed to `.{to}()`, which wraps around
  headline: no test makes `.{name}()` overflow in {method}
  help: add a test whose result doesn't fit its type, and assert what comes back
```

A test that never sets a port can't tell the fallback from the port set:

<!-- output: reference/configured-kept-card -->
```text
  ✗ MISSED  no test gives port() a value for .unwrap_or() to prefer to its fallback
    ┌─ src/lib.rs:20
    │
 19 │ pub fn port(set: Option<u16>) -> u16 {
 20 │     set.unwrap_or(8080)
    │         ^^^^^^^^^ .unwrap_or() always gives its fallback
 21 │ }
    │
    = mutant  - set.unwrap_or(8080)
              + 8080
    = note: covered by tests::a_port_is_8080_unless_set (its 1 covering test still passes)
    = help: add a test where the value is there, and assert it's the one used

    done when  mutastic verify m294c8110 fixtures/configured passes
    reproduce  mutastic run --only=m294c8110 fixtures/configured
```

What differs from the Java edition: mutastic reads source without types, so a rule matches a call
by its method's name, and `owners` isn't checked. A rewrite that doesn't fit the receiver's type
doesn't compile, so the build withdraws it as unviable, at the cost of a build round. `descriptors`
and `since` are the JVM's, and an error here, and so is `omit last argument`, which calls a
method's shorter overload: a Rust method has one count of arguments. A `call` rewrite's types are
Rust's, and read but not checked, where the Java edition finds the method by the JVM's.

A rules file has its [JSON Schema](https://docs.mutastic.io/schemas/rules/v1.json) too, shared
with the Java edition, for an editor to check it against: `# yaml-language-server:
$schema=https://docs.mutastic.io/schemas/rules/v1.json` on its first line.

## What mutastic reads

The environment variables mutastic reads, and the option each stands in for, are on
[Environment and exit codes](commands.md#environment). The order for
the format is `--format`, then `MUTASTIC_FORMAT`, then `MUTASTIC_AGENT`, then whether stdout is a
terminal.

In a Maven or Gradle project, `MUTASTIC_VERSION`, `MUTASTIC_HOME`, `MUTASTIC_JAVA_ENGINE` and
`MUTASTIC_JAVA_RELEASES` say which Java edition runs, and where it's kept, and `MUTASTIC_JAVA_HOME`
the JDK it runs on: see
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
| `target/mutastic/next.tsv` | What the last `mutastic next` left uncaught, and what it ran under |
| `target/mutastic/build-args.key` | A random key, only yours to read, that stands Cargo's arguments in the run record without saying them |
| `target/mutastic/gate.properties` | An open gate: where the task started; `gate close` deletes it |

The run record (`last-run.tsv` and `last-runs/`), `next.tsv` and `gate.properties` are mutastic's
own. Their names stay, and `gate close` deletes the gate's file, but what's in them is private to
each edition: the Rust and Java editions write them differently, and either may change them in any
release. Read a run's results from `mutation.json` or the [events](events.md) instead. See
[Compatibility](compatibility.md#files-mutastic-keeps-for-itself).

`cargo clean` removes it with the rest. A `CARGO_TARGET_DIR` or `build.target-dir` you've set moves
it with your target directory.
