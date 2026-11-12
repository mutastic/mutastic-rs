# Commands

<!-- Written from crates/mutastic/src/commands.rs by the docs test, with MUTASTIC_DOCS_WRITE=1 cargo test --test docs; change the declarations, not this page. -->

Every command `mutastic` takes, with its options, the environment it reads and the codes it exits with. `cargo mutastic` takes the same words.

- [`mutastic run`](#run): Mutate the package and run its tests against each mutant
- [`mutastic verify`](#verify): Run mutants' tests: it passes when they catch every one not skipped
- [`mutastic explain`](#explain): Everything known of one mutant, with its tests run
- [`mutastic ignore`](#ignore): Record a decision not to test one mutant
- [`mutastic next`](#next): Show the missed mutant most worth a test first, and why
- [`mutastic gate open`](#gate-open): Start gating a coding agent's task: keep where it starts
- [`mutastic gate check`](#gate-check): Say whether a gated task is done: its changed lines' mutants caught
- [`mutastic gate close`](#gate-close): Stop gating a task: forget where it started
- [`mutastic mutants`](#mutants): List the mutants, without building or running anything
- [Environment](#environment) and [exit codes](#exit-codes)

## run

`mutastic run [dir] [options]`

Mutate the package and run its tests against each mutant. dir is the package's directory, here unless given; in a workspace, a member's. The run passes when the score reaches the threshold. A full run, one without --only, writes report files, and offers the commands to work on each missed mutant. Its progress goes to stderr, so stdout holds only the result. --shard runs one share of the mutants, for splitting a run across CI jobs, and --from-shards puts the shares back together as the whole run.

```sh
mutastic run
mutastic run fixtures/pricing --threshold=70
mutastic run --since=main
mutastic run fixtures/pricing --operators=stronger
mutastic run --shard=2/4 --threshold=0
mutastic run --from-shards=shares --threshold=70
```

**Options:** [`--threshold=N`](#--threshold), [`--since[=REF]`](#--since), [`--only=ID`](#--only), [`--workers=N`](#--workers), [`--no-suggestions`](#--no-suggestions), [`--operators=OPERATORS`](#--operators), [`--unsafe=MODE`](#--unsafe), [`--report=KIND[:PATH]`](#--report), [`--shard=N/M`](#--shard), [`--from-shards=DIR`](#--from-shards), [`--format=FORMAT`](#--format), [`--vocabulary=WORDS`](#--vocabulary), [`--color=WHEN`](#--color), [`--hyperlinks=WHEN`](#--hyperlinks), [`--progress=STYLE`](#--progress), [`--edition=EDITION`](#--edition).

**Exit status:** 0 when the score reaches the threshold, 1 when it doesn't.

## verify

`mutastic verify <id>[,<id>…] [<id>…] [dir]`

Run mutants' tests: it passes when they catch every one not skipped. How a fix is proved: each missed mutant offers it as "done when", and a card with several mutants names them all, split by commas; spaces split them too, and the directory comes after them. An id names its mutant whatever options showed it, so verify needs no --operators or --unsafe to find one.

```sh
mutastic verify mb2f8ae57
mutastic verify mb2f8ae57 fixtures/pricing
mutastic verify mb2f8ae57 m95f96847 fixtures/pricing
```

**Options:** [`--workers=N`](#--workers), [`--progress=STYLE`](#--progress), [`--edition=EDITION`](#--edition).

**Exit status:** 0 when it catches each mutant named, or the mutant is skipped or crashed where a crash proves nothing; 1 while one is still missed or not covered.

## explain

`mutastic explain <id> [dir]`

Everything known of one mutant, with its tests run. Its card when it's missed, or what became of it otherwise, then what the change is, its operator, how its id is made, where it is, and how the last run of every mutant, by run or next, went for it, from that run's mutation.json.

```sh
mutastic explain mb2f8ae57
mutastic explain mb2f8ae57 fixtures/pricing
```

**Options:** [`--workers=N`](#--workers), [`--no-suggestions`](#--no-suggestions), [`--vocabulary=WORDS`](#--vocabulary), [`--color=WHEN`](#--color), [`--hyperlinks=WHEN`](#--hyperlinks), [`--progress=STYLE`](#--progress), [`--edition=EDITION`](#--edition).

**Exit status:** 0 whatever the mutant's result: it explains, it doesn't judge.

## ignore

`mutastic ignore <id> [dir] --reason=<why>`

Record a decision not to test one mutant. It goes in .mutastic/ignores.yaml in the package's directory with its line and the reason, to commit and review, and lapses when that line changes: the mutant runs again, and run-finished lists it in staleIgnores. --remove takes one back; ignore --prune takes away every one that has lapsed. --propose records it in .mutastic/proposals.yaml instead, for a person to accept with --accept: how a coding agent says a mutant isn't worth a test, since until then it still counts as missed. A proposal that a mutant is equivalent or unreachable says the one case where it differs, and why that can't happen. It runs no tests, and each step says how to undo it.

```sh
mutastic ignore mb2f8ae57 --reason="totals are rounded to tens"
mutastic ignore mb2f8ae57 --propose --as=unreachable --reason="Differs only when total is 100. That can't happen because prices end in 9."
mutastic ignore --accept mb2f8ae57
mutastic ignore --remove mb2f8ae57
mutastic ignore --prune
```

**Options:** [`--reason=WHY`](#--reason), [`--as=KIND`](#--as), [`--remove`](#--remove), [`--propose`](#--propose), [`--accept`](#--accept), [`--prune`](#--prune), [`--edition=EDITION`](#--edition).

**Exit status:** 0 when the decision is recorded, proposed, accepted or removed; 1 when --accept names a mutant that isn't proposed.

## next

`mutastic next [dir] [options]`

Show the missed mutant most worth a test first, and why. The missed mutants as a queue, one line's at a time, in the order a run ranks them: its card, why it's first, and the command that says when it's done. --top shows the first few, spread over the code. While the code under test is the same, it runs only the mutants the last next left uncaught, so any a new test catches drop out; once that code changes, it runs every mutant again. A mutant proposed for ignoring isn't queued: it waits on a person. For an agent, one task, and to run next again after it.

```sh
mutastic next
mutastic next --top=5
mutastic next --format=agent
```

**Options:** [`--since[=REF]`](#--since), [`--workers=N`](#--workers), [`--no-suggestions`](#--no-suggestions), [`--operators=OPERATORS`](#--operators), [`--unsafe=MODE`](#--unsafe), [`--format=FORMAT`](#--format), [`--vocabulary=WORDS`](#--vocabulary), [`--color=WHEN`](#--color), [`--hyperlinks=WHEN`](#--hyperlinks), [`--progress=STYLE`](#--progress), [`--top=N`](#--top), [`--edition=EDITION`](#--edition).

**Exit status:** 0 when nothing missed is left, or only mutants proposed for ignoring; 1 while something is queued.

## gate open

`mutastic gate open [dir]`

Start gating a coding agent's task: keep where it starts. Keeps the commit the task starts from, its ignores, and how many tests run and are ignored, in target/mutastic, so gate check can say whether the task is done. With --hook, once for each of the agent's sessions.

```sh
mutastic gate open
mutastic gate open --hook=claude
```

**Options:** [`--hook=AGENT`](#--hook), [`--edition=EDITION`](#--edition).

**Exit status:** 0 once it has kept where the task starts; 2 outside a git repository with a commit.

## gate check

`mutastic gate check [dir]`

Say whether a gated task is done: its changed lines' mutants caught. Runs the mutants on the lines changed since gate open. The task is done when each is caught or proposed for a person to ignore, no ignore was made by the task, and no test was taken away or ignored; until then it says what's left, as the agent format does. With --hook it answers as the agent's hook, and lets the agent stop, saying what's left, when it changed nothing since it was sent back, or after --tries.

```sh
mutastic gate check
mutastic gate check --hook=claude
```

**Options:** [`--hook=AGENT`](#--hook), [`--tries=N`](#--tries), [`--edition=EDITION`](#--edition).

**Exit status:** 0 when the task is done, 1 when it isn't, 2 when no task is open. With --hook, 0 either way: the answer is what it prints, a block or a message, as the agent's hooks read it, and with nothing open the agent may stop.

## gate close

`mutastic gate close [dir]`

Stop gating a task: forget where it started. The next gate open starts a new task.

```sh
mutastic gate close
```

**Options:** [`--hook=AGENT`](#--hook), [`--edition=EDITION`](#--edition).

**Exit status:** 0 whether a task was open or not: it says which, or with --hook says nothing.

## mutants

`mutastic mutants [dir]`

List the mutants, without building or running anything. One line each: its id, place, operator, the change, and the function it's in. A mutant near unsafe code says why it's marked, and the last lines count what was left out. With --operators and --unsafe, it shows what those would add before a run.

```sh
mutastic mutants
mutastic mutants fixtures/pricing --operators=stronger
```

**Options:** [`--operators=OPERATORS`](#--operators), [`--unsafe=MODE`](#--unsafe), [`--edition=EDITION`](#--edition).

**Exit status:** 0 once they're listed.

## Options

### `--threshold`

`--threshold=N`, for [`run`](#run). The score a run must reach to pass; 80 unless set.

```sh
mutastic run --threshold=70
```

### `--since`

`--since[=REF]`, for [`next`](#next), [`run`](#run). Mutate only the lines changed since the merge-base of HEAD and REF, uncommitted and new files included; the default branch without one.

```sh
mutastic next --since=main
mutastic run --since=main
```

### `--only`

`--only=ID`, for [`run`](#run). Run one mutant, or several split by commas, whatever --operators says.

```sh
mutastic run --only=mb2f8ae57
```

### `--workers`

`--workers=N`, for [`explain`](#explain), [`next`](#next), [`run`](#run), [`verify`](#verify). How many mutants run side by side; half the cores, from 1 to 4, unless set.

```sh
mutastic explain mb2f8ae57 --workers=2
mutastic next --workers=2
mutastic run --workers=2
mutastic verify mb2f8ae57 --workers=2
```

### `--no-suggestions`

`--no-suggestions`, for [`explain`](#explain), [`next`](#next), [`run`](#run). Don't look for a test that catches each missed mutant, which runs the missed mutant's function outside the tests.

```sh
mutastic explain mb2f8ae57 --no-suggestions
mutastic next --no-suggestions
mutastic run --no-suggestions
```

### `--operators`

`--operators=OPERATORS`, for [`mutants`](#mutants), [`next`](#next), [`run`](#run). Which operators run: a set (defaults, stronger or extended), or operator ids split by commas, in place of mutastic.yaml's operators; defaults unless set.

```sh
mutastic mutants --operators=stronger
mutastic next --operators=stronger
mutastic run --operators=stronger
```

### `--unsafe`

`--unsafe=MODE`, for [`mutants`](#mutants), [`next`](#next), [`run`](#run). Which unsafe code is mutated: skip leaves unsafe code and the functions holding an unsafe block alone, enclosing mutates those functions too, all unsafe code as well; skip unless set. One of `skip`, `enclosing`, `all`.

```sh
mutastic mutants --unsafe=enclosing
mutastic next --unsafe=enclosing
mutastic run --unsafe=enclosing
```

### `--report`

`--report=KIND[:PATH]`, for [`run`](#run). A report file a full run writes: json (mutation.json), html (mutation.html) or sarif (mutation.sarif), in target/mutastic unless a path is given, or none; json and html unless set. One of `json`, `html`, `sarif`, `none`.

```sh
mutastic run --report=json
```

### `--shard`

`--shard=N/M`, for [`run`](#run). Run only the Nth of M shares of the mutants, to split one run across machines. It keeps its results in shares/N-of-M in target/mutastic for --from-shards.

```sh
mutastic run --shard=2/4 --threshold=0
```

### `--from-shards`

`--from-shards=DIR`, for [`run`](#run). Run no mutants: put together the results every share of a split run kept, found anywhere in DIR, as the whole run, with its reports and threshold.

```sh
mutastic run --from-shards=shares --threshold=70
```

### `--format`

`--format=FORMAT`, for [`next`](#next), [`run`](#run). How the result is shown; full on a terminal, concise elsewhere, agent for coding agents. One of `full`, `brief`, `concise`, `grouped`, `agent`, `jsonl`. Also set by `MUTASTIC_FORMAT` or `MUTASTIC_AGENT`.

```sh
mutastic next --format=agent
mutastic run --format=agent
```

### `--vocabulary`

`--vocabulary=WORDS`, for [`explain`](#explain), [`next`](#next), [`run`](#run). plain says caught and missed; classic says killed and survived. One of `plain`, `classic`. Also set by `MUTASTIC_VOCABULARY`.

```sh
mutastic explain mb2f8ae57 --vocabulary=classic
mutastic next --vocabulary=classic
mutastic run --vocabulary=classic
```

### `--color`

`--color=WHEN`, for [`explain`](#explain), [`next`](#next), [`run`](#run). Colour: auto uses it on a terminal that wants it. One of `auto`, `always`, `never`. Also set by `NO_COLOR` or `FORCE_COLOR`.

```sh
mutastic explain mb2f8ae57 --color=never
mutastic next --color=never
mutastic run --color=never
```

### `--hyperlinks`

`--hyperlinks=WHEN`, for [`explain`](#explain), [`next`](#next), [`run`](#run). File locations as links a terminal can open: auto uses them on a terminal. One of `auto`, `always`, `never`.

```sh
mutastic explain mb2f8ae57 --hyperlinks=never
mutastic next --hyperlinks=never
mutastic run --hyperlinks=never
```

### `--progress`

`--progress=STYLE`, for [`explain`](#explain), [`next`](#next), [`run`](#run), [`verify`](#verify). Progress on stderr: a bar on a terminal, counter lines in logs, none for agents. One of `auto`, `bar`, `counter`, `none`.

```sh
mutastic explain mb2f8ae57 --progress=counter
mutastic next --progress=counter
mutastic run --progress=counter
mutastic verify mb2f8ae57 --progress=counter
```

### `--top`

`--top=N`, for [`next`](#next). How many of the queue next shows, at most one a function and two a file; 1 unless set, at most 25.

```sh
mutastic next --top=5
```

### `--reason`

`--reason=WHY`, for [`ignore`](#ignore). Why the mutant isn't worth a test: every ignore says.

```sh
mutastic ignore mb2f8ae57 --reason="totals are rounded to tens"
```

### `--as`

`--as=KIND`, for [`ignore`](#ignore). What sort of decision it is. One of `equivalent`, `unreachable`, `telemetry`, `deprecated`, `accepted-risk`.

```sh
mutastic ignore mb2f8ae57 --as=unreachable --reason="totals are rounded to tens"
```

### `--remove`

`--remove`, for [`ignore`](#ignore). Take the mutant's ignore, or its proposal, away, so it runs again.

```sh
mutastic ignore --remove mb2f8ae57
```

### `--propose`

`--propose`, for [`ignore`](#ignore). Propose the ignore for a person to accept, rather than make it: a coding agent's way. An equivalent or unreachable mutant's reason says the one case where it differs and why that can't happen.

```sh
mutastic ignore mb2f8ae57 --propose --as=unreachable --reason="Differs only when total is 100. That can't happen because prices end in 9."
```

### `--accept`

`--accept`, for [`ignore`](#ignore). Accept a proposed ignore: it moves into the ignores, as proposed.

```sh
mutastic ignore --accept mb2f8ae57
```

### `--prune`

`--prune`, for [`ignore`](#ignore). Take away every ignore whose line has changed.

```sh
mutastic ignore --prune
```

### `--hook`

`--hook=AGENT`, for [`gate check`](#gate-check), [`gate close`](#gate-close), [`gate open`](#gate-open). Run as the agent's hook: read its input on stdin, and answer as its hooks do. One of `claude`.

```sh
mutastic gate check --hook=claude
mutastic gate close --hook=claude
mutastic gate open --hook=claude
```

### `--tries`

`--tries=N`, for [`gate check`](#gate-check). With --hook, how many times check sends the agent back before it lets it stop and says what's left, from 1 to 100; 3 unless set.

```sh
mutastic gate check --hook=claude --tries=5
```

### `--edition`

`--edition=EDITION`, for [`explain`](#explain), [`gate check`](#gate-check), [`gate close`](#gate-close), [`gate open`](#gate-open), [`ignore`](#ignore), [`mutants`](#mutants), [`next`](#next), [`run`](#run), [`verify`](#verify). Which edition runs; unless set, the build file in dir, or the nearest above it: Cargo.toml for rust, pom.xml, build.gradle or settings.gradle for java. One of `rust`, `java`.

```sh
mutastic explain mb2f8ae57 --edition=rust
mutastic gate check --edition=rust
mutastic gate close --edition=rust
mutastic gate open --edition=rust
mutastic ignore mb2f8ae57 --edition=rust
mutastic mutants --edition=rust
mutastic next --edition=rust
mutastic run --edition=rust
mutastic verify mb2f8ae57 --edition=rust
```

## Environment

- `MUTASTIC_FORMAT`: The format when --format isn't given. For a machine or a CI job that always wants the same format, `concise` say.
- `MUTASTIC_AGENT=1`: The agent format, for coding agents. Coding agents set it, so mutastic answers in the format they read best. `MUTASTIC_FORMAT` still wins.
- `MUTASTIC_VOCABULARY`: plain or classic, when --vocabulary isn't given. `classic` says killed and survived instead of caught and missed. The JSON report and the event stream always use the standard names.
- `NO_COLOR`: No colour. Set to anything, it turns colour off, as it does for many tools.
- `FORCE_COLOR`: Colour even off a terminal. Set to anything, it turns colour on where mutastic can't tell a person is reading, such as a CI log that shows colour. It wins over `NO_COLOR`.
- `TERM=dumb`: No colour, unless `FORCE_COLOR` is set.
- `CARGO`: The cargo mutastic runs. Cargo sets it for `cargo mutastic`; `cargo` on the `PATH` otherwise.
- `RUSTC`: The rustc Cargo builds with, as for any Cargo build. The result names its version.
- `MUTASTIC_VERSION`: In a Maven or Gradle project, the Java edition's release to use. Such as `0.5.0`, fetched if it isn't kept. Unset, from 1.0.0 it's the newest release of this mutastic's own major and minor version.
- `MUTASTIC_HOME`: Where the Java edition's releases are kept. `~/.mutastic` unless set.
- `MUTASTIC_JAVA_ENGINE`: A Java edition's `bin/mutastic` to run instead, such as one built from its source. Nothing is fetched.
- `MUTASTIC_JAVA_RELEASES`: Where the Java edition's releases are downloaded from, for a mirror. GitHub's releases of `mutastic/mutastic` unless set.
- `JAVA_HOME`: The JDK the Java edition runs on. `java` on the `PATH` unless set.

## Exit codes

- **0**: The score reaches the threshold; for verify, each mutant named is caught, skipped, or crashed where a crash proves nothing; for gate check, the task is done; for next, nothing missed is left but what's proposed for ignoring.
- **1**: The score is below the threshold; for verify, a mutant named is still missed or not covered; for gate check, the task isn't done; for ignore --accept, the mutant isn't proposed; for next, something is queued.
- **2**: mutastic couldn't run: an unknown option, a word the command doesn't take, not a Cargo package, a workspace's root, a test that fails with no mutant in, an id that isn't a mutant of the package, shares --from-shards can't put together as one run, or no task open for gate check.
