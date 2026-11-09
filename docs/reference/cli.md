# Commands

mutastic installs two commands that do the same thing: `mutastic`, and `cargo mutastic` for use
inside a package, as Cargo's other subcommands are. `cargo mutastic run` is `mutastic run`.

<!-- output: reference/help -->
```text
mutastic: find the tests that run your code but don't check it

Usage:
  mutastic run [dir] [options]       Mutate the package in dir (default .) and run its tests
  mutastic verify <id>… [dir]        Pass when a test catches each mutant named, the ids split
                                     by commas or spaces
  mutastic explain <id> [dir]        Everything known of one mutant, with its tests run
  mutastic ignore <id> [dir] --reason=<why>
                                     Record a decision not to test one mutant, in
                                     .mutastic/ignores.yaml; --as=<kind> says what sort; --propose
                                     proposes it for a person to accept (--accept <id>);
                                     --remove <id> takes one back; --prune those whose line changed
  mutastic gate open|check|close [dir]
                                     Gate a coding agent's task on its changed lines' mutants:
                                     open keeps where it starts, check says whether it's done
                                     (--hook=claude answers as Claude Code's hooks, --tries=<n>),
                                     close forgets it; gate <action> --help says more
  mutastic mutants [dir]             List the mutants, without running anything

Options:
  --only=<id>[,<id>…]   Run only these mutants
  --format=<format>     full (default at a terminal), concise, agent or jsonl
  --threshold=<score>   The score a run must reach to pass (default 80)
  --operators=<set>     defaults, stronger or extended, or operator ids separated by commas
  --workers=<n>         Tests run side by side (default half the cores, at most 4)
  --unsafe=<mode>       skip (default): leave unsafe code and functions holding an unsafe block
                        alone; enclosing: mutate those functions too; all: unsafe code as well
  --since[=<ref>]       Run only the mutants on lines changed since the merge-base of HEAD and
                        <ref> (default: the default branch), uncommitted and new files included
  --report=<kind>[:<path>]  Write a report file: json (mutation.json) or html (mutation.html), in
                        target/mutastic unless a path is given; repeat it for more, or none.
                        A full run writes both by default
  --no-suggestions      Don't look for a test that catches each missed mutant, which runs the
                        missed mutant's function outside the tests
  --progress=<mode>     auto (default): a line redrawn in place at a terminal, none for agent
                        and jsonl; bar, counter (a line every so often) or none
  --edition=<edition>   rust or java: which edition runs. Unless given, the build file in dir, or
                        the nearest above it, decides: Cargo.toml for rust; pom.xml, build.gradle
                        or settings.gradle for java, which is fetched the first time it's needed

Exit status: 0 when the run passes, or verify's mutants are caught; 1 when it doesn't; 2 when
mutastic couldn't run.
```

Each command takes the package's directory, `.` when it's left out. In a workspace, give the
member's directory: see [Troubleshooting](../troubleshooting.md#a-workspace). In a Maven or Gradle
project, the same command runs the Java edition, with everything given to it as it was typed: see
[One command, two editions](../editions.md).

## `mutastic run`

Mutates the package, runs its tests against each mutant, and prints the result. With nothing
missed, or a score at or above the threshold, it passes.

```sh
mutastic run fixtures/pricing
mutastic run fixtures/pricing --operators=stronger
mutastic run --only=mb2f8ae57 fixtures/pricing
```

What it prints at a terminal is the [Getting started](../getting-started.md) page's: a card for each
line with a missed mutant, the code no test calls, and the summary. Its progress goes to stderr, so
stdout holds only the result.

## `mutastic verify`

Runs only the mutants named, and passes when each is caught. It's the "done when" command a card
offers: run it after writing the test.

```sh
mutastic verify mb2f8ae57 fixtures/pricing
```

Name several, separated by commas as a card's `done when` writes them (`verify mb2f8ae57,m0b3873d0`)
or by spaces (`verify mb2f8ae57 m0b3873d0 fixtures/pricing`), as the Java edition takes them, and
it passes only when every one is caught. The directory comes after the ids. One line each says how
it went:

| It prints | Means | Passes |
|---|---|---|
| `is caught by tests::discount_with_total_100` | a test failed | yes |
| `is caught by run::totals, through a panic: attempt to subtract with overflow` | a test failed through a panic the language raised | yes |
| `is caught only because the test process crashed (SIGABRT): no test's check caught it` | no test failed: the process died, in safe code | yes |
| `is caught by a timeout` | the tests ran out of time | yes |
| `is still missed` | every test that reaches it passed | no |
| `isn't reached by any test` | no test reaches it | no |
| `is skipped (unviable: …), so it isn't run and needs no test` | it doesn't compile | yes |
| `crashes, and no test can be said to catch it (…)` | it crashed where a crash proves nothing | yes |

An id names its mutant whatever options showed it, so `verify` needs no `--operators` or `--unsafe`
to find one an `--operators=extended` or `--unsafe=all` run showed.

## `mutastic explain`

Runs one mutant, and shows everything known of it: its card when it's missed, or what became of it
otherwise, then what the change is, its operator, how its id is made, where it is, and how the last
full run went for it, from that run's `mutation.json`. It explains, it doesn't judge: it exits 0
whatever the mutant's result.

```sh
mutastic explain mb2f8ae57 fixtures/pricing
```

<!-- output: reference/explain-missed -->
```text

 [✗ MISSED] no test checks discount() at total == 100
    ┌─ src/lib.rs:26
    │
 25 │     pub fn discount(&self, total: u32) -> u32 {
 26 │         if total >= 100 {
    │                  ^^ changed to >
 27 │             return total / 10;
    │
    = mutant  - if total >= 100 {
              + if total > 100 {
    = note: every test calls discount() away from the boundary
            inputs seen: 50, 250 (none is 100)
            covered by pricing::big_orders_get_ten_percent_off, tests::small_orders_pay_full_price (all 2 covering tests still pass)
    = help: test discount() with total = 100 and assert the result

            // in mod tests
            #[test]
            fn discount_with_total_100() {
                assert_eq!(Pricing::new().discount(100), 10); // today 10 · with this mutant 0
            }
            check today's value is the one you intend: the test pins what the code does now

    done when  mutastic verify mb2f8ae57 fixtures/pricing passes
    reproduce  mutastic run --only=mb2f8ae57 fixtures/pricing
    or ignore  mutastic ignore mb2f8ae57 --reason="<why>" fixtures/pricing

 About this mutant ───────────────────────────────────────────────
    = change    total >= 100 → (total) > (100)
                the code the switch puts in place of the original when this mutant is live
    = operator  mutastic/boundary: `>=` becomes `>`
    = id        mb2f8ae57: `m` and the first 8 hex digits of its fingerprint, b2f8ae57ca6c48cb, a SHA-256 of the crate, function, operator, site and change
                no line number goes in, so the id stays the same when code above it moves
    = site      `total >= 100` in Pricing::discount, line 26, column 18
    = last run  missed: every covering test still passed
```

A mutant that's caught, not covered or skipped gets a card that says so, without help or a test to
add:

<!-- output: reference/explain-caught -->
```text

 [✓ CAUGHT] caught by pricing::big_orders_get_ten_percent_off
    ┌─ src/lib.rs:27
    │
 26 │         if total >= 100 {
 27 │             return total / 10;
    │                          ^ changed to *
 28 │         }
    │
    = mutant  - return total / 10;
              + return total * 10;
    = note: inputs seen: total = 250
            covered by pricing::big_orders_get_ten_percent_off

    reproduce  mutastic run --only=m95f96847 fixtures/pricing

 About this mutant ───────────────────────────────────────────────
    = change    total / 10 → (total) * (10)
                the code the switch puts in place of the original when this mutant is live
    = operator  mutastic/math: `/` becomes `*`
    = id        m95f96847: `m` and the first 8 hex digits of its fingerprint, 95f96847248b0509, a SHA-256 of the crate, function, operator, site and change
                no line number goes in, so the id stays the same when code above it moves
    = site      `total / 10` in Pricing::discount, line 27, column 26
    = last run  caught by pricing::big_orders_get_ten_percent_off
```

## `mutastic ignore`

Records a decision not to test one mutant, in `.mutastic/ignores.yaml` in the package's directory,
with the line it's on and the reason, to commit and review. The mutant is skipped (`user: …
(ignored)`) while its line stays as it was; once the line changes, the decision no longer holds, the
mutant runs again, and `run-finished` lists it in `staleIgnores`. Every card offers it as
`or ignore`. It runs no tests, and each step says how to undo it.

```sh
mutastic ignore mb2f8ae57 --reason="totals are rounded to tens" --as=unreachable fixtures/pricing
```

| With | It |
|---|---|
| `--reason=<why>` | is needed: whoever reviews the decision reads it |
| `--as=<kind>` | says what sort of decision it is: `equivalent`, `unreachable`, `telemetry`, `deprecated` or `accepted-risk` |
| `--propose` | records it in `.mutastic/proposals.yaml` instead, for a person to accept: how a coding agent says a mutant isn't worth a test, since until then it still counts as missed. The agent format offers it as `or propose` |
| `--accept <id>` | moves a proposal into the ignores |
| `--remove <id>` | takes an ignore, or a proposal, back |
| `--prune` | takes away every ignore whose line has changed |

The files are the Java edition's, in the same shape, with the function's path in `at`
(`pricing::Pricing::discount, line 26`).

## `mutastic gate`

Holds a coding agent's task to its changed lines' mutants, as the Java edition's gate does, so the
agents' plugins work with either edition. `gate open` keeps where the task starts: the commit, the
ignores, and how many tests there are and how many are `#[ignore]`d, in `target/mutastic`.
`gate check` runs the mutants on the lines changed since, and passes when each is caught or
proposed for a person to ignore; until then it prints what's left as the agent format's cards. It
doesn't pass a task that ignored a mutant itself, that has fewer tests running than when it
started, or that took away a test which failed at an earlier check. `gate close` forgets the task.

```sh
mutastic gate open fixtures/pricing
mutastic gate check fixtures/pricing
```

With `--hook=claude`, each reads a Claude Code hook's input on stdin and answers as a hook does:
`open` once per session, `check` with a block and its reason or a message for the person. A hooked
`check` lets the agent stop, and tells the person what's left, when the agent changed nothing since
it was sent back, or after `--tries` times. `mutastic gate <action> --help` says how each exits.

## `mutastic mutants`

Lists the mutants, without building or running anything: id, place, operator, the change, and the
function it's in. A near-unsafe mutant says why it's marked, and the last lines count what was left
out.

<!-- output: reference/mutants -->
```text
mc9e13290 src/lib.rs:29:9 mutastic/return-value returns 1 instead in Pricing::discount
mb2f8ae57 src/lib.rs:26:18 mutastic/boundary >= becomes > in Pricing::discount
md56e8c39 src/lib.rs:26:12 mutastic/remove-conditional total >= 100 is always true in Pricing::discount
m1636f6d5 src/lib.rs:26:12 mutastic/remove-conditional total >= 100 is always false in Pricing::discount
m6d05659f src/lib.rs:27:20 mutastic/return-value returns 0 instead in Pricing::discount
m95f96847 src/lib.rs:27:26 mutastic/math / becomes * in Pricing::discount
m383e0002 src/lib.rs:37:9 mutastic/return-value returns 1 instead in Pricing::shipping
m7d92063c src/lib.rs:34:18 mutastic/boundary < becomes <= in Pricing::shipping
m3be6b420 src/lib.rs:34:12 mutastic/remove-conditional total < self.free_shipping_from is always true in Pricing::shipping
m89be540d src/lib.rs:34:12 mutastic/remove-conditional total < self.free_shipping_from is always false in Pricing::shipping
mbddcd9c4 src/lib.rs:35:20 mutastic/return-value returns 0 instead in Pricing::shipping
11 mutants
```

It takes `--operators` and `--unsafe`, to see what those would add before a run.

## Options

| Option | For | What it does |
|---|---|---|
| `--format=<format>` | `run` | `full`, `concise`, `agent` or `jsonl`: see [Formats](#formats) |
| `--operators=<set>` | `run`, `mutants` | `defaults`, `stronger` (the defaults and more), `extended` (all of them), or operator ids separated by commas: see [Operators](../operators.md) |
| `--only=<id>[,<id>…]` | `run` | Runs only these mutants, whatever `--operators` says |
| `--threshold=<score>` | `run` | The score a run must reach to pass; its default is in the usage above |
| `--workers=<n>` | `run`, `verify` | How many mutants run side by side; its default, which follows the cores, is in the usage above |
| `--unsafe=<mode>` | `run`, `mutants` | `skip`, `enclosing` or `all`: see [Unsafe code](../concepts/unsafe.md) |
| `--since[=<ref>]` | `run` | Runs only the mutants on the lines changed since the merge-base of `HEAD` and `<ref>`, or the default branch without one: committed, uncommitted and new files alike. The summary says how much changed (`Changed  3 lines in 1 file since main`), as `run-finished`'s `scope` does |
| `--report=<kind>[:<path>]` | `run` | A report file to write: `json` (`mutation.json`) or `html` (`mutation.html`), in `target/mutastic` unless a path is given, against the package's directory. Give it more than once for more; `none` writes none. With no `--report`, a full run writes both: see [Report files](#report-files) |
| `--no-suggestions` | `run`, `explain` | Don't look for a test that catches each missed mutant: the search calls the mutant's function outside your tests. See [Suggested tests](../concepts/suggested-tests.md) |
| `--progress=<mode>` | `run`, `verify` | How the run shows how far it is while mutants run: `auto`, a line redrawn in place at a terminal and none for `agent` or `jsonl`; `bar`; `counter`, that line every few seconds; or `none` |

## Report files

A full run, one without `--only`, writes two files beside its build in `target/mutastic`, as the
Java edition does, and its summary says where (`Reports`), as `run-finished`'s `reports` does:

- **`mutation.json`,** in the [mutation-testing report schema](https://github.com/stryker-mutator/mutation-testing-elements/tree/master/packages/report-schema),
  version 2: each source file with its text and its mutants, their status, the tests that cover
  them, and why each has its status. Other tools that read that schema read it.
- **`mutation.html`,** the same report in that schema's viewer, embedded whole, so it opens from
  the disk with no network.

`language` is `rust` in each file's entry, and the tests are grouped by the module that holds them
(`tests`, `pricing`), where the Java edition groups them by class.

An option mutastic doesn't know stops it, with the usage, rather than being ignored.

## Formats

`--format` chooses how a run prints its result. At a terminal it's `full`; when stdout isn't a
terminal, such as in CI or a pipe, it's `concise`; with `MUTASTIC_AGENT=1` it's `agent`.
`MUTASTIC_FORMAT` sets it too: see [Configuration](configuration.md).

### `full`

A card for each line with a missed mutant, for a person reading at a terminal. See
[Getting started](../getting-started.md#3-read-the-card) for how to read one.

### `concise`

One line for each line with a missed mutant, with its ids, then the summary. For logs.

<!-- output: reference/format-concise -->
```text

 Missed ────────────────────────────────────────────────────────────
 ✗ src/lib.rs:26 no test checks discount() at total == 100 mb2f8ae57

     Files  0 clean | 1 with misses (1)
   Mutants  ✓ 10 caught · ✗ 1 missed (11)
  Worst at  Pricing::discount() 1 ✗
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  13 (33 without coverage selection)
  Start at  18:25:20
  Duration  1.1s (setup 1.1s, coverage 4ms, mutants 16ms)
     Tests  3 on rustc 1.94.0 · 4 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  90.9%  threshold 80

 [PASS] 1 missed, above the threshold.
 Next: mutastic verify <id> fixtures/pricing · mutastic run --only=<id> fixtures/pricing · mutastic explain <id> fixtures/pricing
```

### `agent`

For a coding agent: the verdict and the rules first, then each missed mutant with its evidence,
the fix, and the commands that prove it. It shows the most telling first, and says how to get the
rest.

<!-- output: reference/format-agent -->
```text
mutastic: PASS · score 90.9% (threshold 80) · 1 missed, 0 not covered, 10 caught of 11 mutants
rules: add or change tests only, never production code; a fix counts when `mutastic verify <id> fixtures/pricing` passes
decline: if a mutant's case can't happen in practice, or the change is one no caller can see, don't contrive a test: say why

1. missed mb2f8ae57: no test checks `discount()` at `total == 100`
   at src/lib.rs:26
   - if total >= 100 {
   + if total > 100 {
   covered by: pricing::big_orders_get_ten_percent_off, tests::small_orders_pay_full_price; all still pass with the change
   evidence: every test calls `discount()` away from the boundary
   evidence: inputs seen: 50, 250 (none is 100)
   fix: test `discount()` with `total = 100` and assert the result
   test to add in mod tests:
     #[test]
     fn discount_with_total_100() {
         assert_eq!(Pricing::new().discount(100), 10); // today 10 · with this mutant 0
     }
     // check today's value is the one you intend: the test pins what the code does now
   done when: mutastic verify mb2f8ae57 fixtures/pricing passes
   reproduce: mutastic run --only=mb2f8ae57 fixtures/pricing
   or propose: mutastic ignore mb2f8ae57 --propose --reason="<why>" fixtures/pricing
```

### `jsonl`

One JSON object per line, the `mutastic.events/v1` stream, for tools. Progress is off, so stdout is
only the events. See [Events](events.md).

<!-- output: reference/format-jsonl -->
```text
{"schema":"mutastic.events/v1","edition":"rust","reason":"run-started","project":"pricing","rules":"add or change tests only, never production code","rule":"add or change tests only, never production code","decline":"if a mutant's case can't happen in practice, or the change is one no caller can see, don't contrive a test: say why","profile":{"name":"mutastic","optLevel":"0","overflowChecks":true,"debugAssertions":true},"unsafe":"skip"}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"mc9e13290","fingerprint":"c9e13290e7d1ce44","status":"Killed","operator":"mutastic/return-value","location":{"file":"src/lib.rs","line":29,"column":9,"class":"Pricing","method":"discount"},"mutation":{"replacement":"1","description":"returns `1` instead","bytecode":"0 -> 1"},"coveredBy":["tests::small_orders_pay_full_price"],"killedBy":"tests::small_orders_pay_full_price","caughtBy":"tests::small_orders_pay_full_price","killedWith":"panic","caughtWith":"panic","inputs":{"total":[50]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"mb2f8ae57","fingerprint":"b2f8ae57ca6c48cb","status":"Survived","operator":"mutastic/boundary","location":{"file":"src/lib.rs","line":26,"column":18,"class":"Pricing","method":"discount"},"mutation":{"original":">=","replacement":">","description":"`>=` becomes `>`","bytecode":"total >= 100 -> (total) > (100)"},"coveredBy":["pricing::big_orders_get_ten_percent_off","tests::small_orders_pay_full_price"],"inputs":{"total":[50,250]},"testsRun":2,"durationMs":4,"doneWhen":"mutastic verify mb2f8ae57 fixtures/pricing passes","reproduce":"mutastic run --only=mb2f8ae57 fixtures/pricing","ignore":"mutastic ignore mb2f8ae57 --reason=\"<why>\" fixtures/pricing","propose":"mutastic ignore mb2f8ae57 --propose --reason=\"<why>\" fixtures/pricing","suggestedTest":{"testClass":"tests","name":"discount_with_total_100","code":"#[test]\nfn discount_with_total_100() {\n    assert_eq!(Pricing::new().discount(100), 10); // today 10 · with this mutant 0\n}","call":"Pricing::new().discount(100)","today":"10","withMutant":"0"},"rendered":"missed mb2f8ae57: no test checks `discount()` at `total == 100`\n   at src/lib.rs:26\n   - if total >= 100 {\n   + if total > 100 {\n   covered by: pricing::big_orders_get_ten_percent_off, tests::small_orders_pay_full_price; all still pass with the change\n   evidence: every test calls `discount()` away from the boundary\n   evidence: inputs seen: 50, 250 (none is 100)\n   fix: test `discount()` with `total = 100` and assert the result\n   test to add in mod tests:\n     #[test]\n     fn discount_with_total_100() {\n         assert_eq!(Pricing::new().discount(100), 10); // today 10 · with this mutant 0\n     }\n     // check today's value is the one you intend: the test pins what the code does now\n   done when: mutastic verify mb2f8ae57 fixtures/pricing passes\n   reproduce: mutastic run --only=mb2f8ae57 fixtures/pricing\n   or propose: mutastic ignore mb2f8ae57 --propose --reason=\"<why>\" fixtures/pricing\n"}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"md56e8c39","fingerprint":"d56e8c3934d92f41","status":"Killed","operator":"mutastic/remove-conditional","location":{"file":"src/lib.rs","line":26,"column":12,"class":"Pricing","method":"discount"},"mutation":{"replacement":"true","description":"`total >= 100` is always true","bytecode":"total >= 100 -> true"},"coveredBy":["pricing::big_orders_get_ten_percent_off","tests::small_orders_pay_full_price"],"killedBy":"tests::small_orders_pay_full_price","caughtBy":"tests::small_orders_pay_full_price","killedWith":"panic","caughtWith":"panic","inputs":{"total":[50,250]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m1636f6d5","fingerprint":"1636f6d59be655d5","status":"Killed","operator":"mutastic/remove-conditional","location":{"file":"src/lib.rs","line":26,"column":12,"class":"Pricing","method":"discount"},"mutation":{"replacement":"false","description":"`total >= 100` is always false","bytecode":"total >= 100 -> false"},"coveredBy":["pricing::big_orders_get_ten_percent_off","tests::small_orders_pay_full_price"],"killedBy":"pricing::big_orders_get_ten_percent_off","caughtBy":"pricing::big_orders_get_ten_percent_off","killedWith":"panic","caughtWith":"panic","inputs":{"total":[50,250]},"testsRun":2,"durationMs":9}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m6d05659f","fingerprint":"6d05659f42679c6c","status":"Killed","operator":"mutastic/return-value","location":{"file":"src/lib.rs","line":27,"column":20,"class":"Pricing","method":"discount"},"mutation":{"replacement":"0","description":"returns `0` instead","bytecode":"total / 10 -> 0"},"coveredBy":["pricing::big_orders_get_ten_percent_off"],"killedBy":"pricing::big_orders_get_ten_percent_off","caughtBy":"pricing::big_orders_get_ten_percent_off","killedWith":"panic","caughtWith":"panic","inputs":{"total":[250]},"testsRun":1,"durationMs":2}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m95f96847","fingerprint":"95f96847248b0509","status":"Killed","operator":"mutastic/math","location":{"file":"src/lib.rs","line":27,"column":26,"class":"Pricing","method":"discount"},"mutation":{"original":"/","replacement":"*","description":"`/` becomes `*`","bytecode":"total / 10 -> (total) * (10)"},"coveredBy":["pricing::big_orders_get_ten_percent_off"],"killedBy":"pricing::big_orders_get_ten_percent_off","caughtBy":"pricing::big_orders_get_ten_percent_off","killedWith":"panic","caughtWith":"panic","inputs":{"total":[250]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m383e0002","fingerprint":"383e0002195614c2","status":"Killed","operator":"mutastic/return-value","location":{"file":"src/lib.rs","line":37,"column":9,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"1","description":"returns `1` instead","bytecode":"0 -> 1"},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m7d92063c","fingerprint":"7d92063c80e7deae","status":"Killed","operator":"mutastic/boundary","location":{"file":"src/lib.rs","line":34,"column":18,"class":"Pricing","method":"shipping"},"mutation":{"original":"<","replacement":"<=","description":"`<` becomes `<=`","bytecode":"total < self.free_shipping_from -> (total) <= (self.free_shipping_from)"},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":2}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m3be6b420","fingerprint":"3be6b420676ea8f8","status":"Killed","operator":"mutastic/remove-conditional","location":{"file":"src/lib.rs","line":34,"column":12,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"true","description":"`total < self.free_shipping_from` is always true","bytecode":"total < self.free_shipping_from -> true"},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":2}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m89be540d","fingerprint":"89be540de2153650","status":"Killed","operator":"mutastic/remove-conditional","location":{"file":"src/lib.rs","line":34,"column":12,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"false","description":"`total < self.free_shipping_from` is always false","bytecode":"total < self.free_shipping_from -> false"},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":2}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"mbddcd9c4","fingerprint":"bddcd9c42b757c41","status":"Killed","operator":"mutastic/return-value","location":{"file":"src/lib.rs","line":35,"column":20,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"0","description":"returns `0` instead","bytecode":"5 -> 0"},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":2}
{"schema":"mutastic.events/v1","edition":"rust","reason":"run-finished","toolchain":{"name":"rustc","version":"1.94.0"},"mutants":11,"Killed":10,"Survived":1,"Timeout":0,"NoCoverage":0,"Ignored":0,"RuntimeError":0,"killed":10,"survived":1,"timeout":0,"noCoverage":0,"ignored":0,"runtimeError":0,"notMutated":{"unsafe":0,"cfg":0},"score":90.9,"threshold":80,"passed":true,"coveredScore":90.9,"tests":3,"flaky":[],"flakyEvidence":[],"likelyFlaky":[],"unrelatedCatches":[],"reports":["target/mutastic/mutation.json","target/mutastic/mutation.html"],"staleIgnores":[],"build":{"rounds":2,"unviable":0},"durationMs":93}
```

## Exit status

| Status | When |
|---|---|
| 0 | `run` passed; every mutant `verify` named is settled (caught, skipped, or crashed where a crash proves nothing); `mutants` listed them |
| 1 | `run`'s score is under the threshold; a mutant `verify` named is missed or not covered |
| 2 | mutastic couldn't run: an unknown option, a word the command doesn't take, not a Cargo package, a workspace's root, a test that fails with no mutant in, an id that isn't a mutant of the package |
