# Commands

mutastic installs two commands that do the same thing: `mutastic`, and `cargo mutastic` for use
inside a package, as Cargo's other subcommands are. `cargo mutastic run` is `mutastic run`.

<!-- output: reference/help -->
```text
mutastic: find the tests that run your code but don't check it

Usage:
  mutastic run [dir] [options]       Mutate the package in dir (default .) and run its tests
  mutastic verify <id>[,<id>…] [dir] Pass when a test catches each mutant named
  mutastic mutants [dir]             List the mutants, without running anything

Options:
  --only=<id>[,<id>…]   Run only these mutants
  --format=<format>     full (default at a terminal), concise, agent or jsonl
  --threshold=<score>   The score a run must reach to pass (default 80)
  --operators=<set>     defaults, stronger or extended, or operator ids separated by commas
  --workers=<n>         Tests run side by side (default half the cores, at most 4)
  --unsafe=<mode>       skip (default): leave unsafe code and functions holding an unsafe block
                        alone; enclosing: mutate those functions too; all: unsafe code as well

Exit status: 0 when the run passes, or verify's mutants are caught; 1 when it doesn't; 2 when
mutastic couldn't run.
```

Each command takes the package's directory, `.` when it's left out. In a workspace, give the
member's directory: see [Troubleshooting](../troubleshooting.md#a-workspace).

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

Name several, separated by commas, and it passes only when every one is caught. One line each says
how it went:

| It prints | Means | Passes |
|---|---|---|
| `is caught by tests::discount_starts_at_100` | a test failed | yes |
| `is caught by run::totals, through a panic: attempt to subtract with overflow` | a test failed through a panic the language raised | yes |
| `is caught only because the test process crashed (SIGABRT): no test's check caught it` | no test failed: the process died, in safe code | yes |
| `is caught by a timeout` | the tests ran out of time | yes |
| `is still missed` | every test that reaches it passed | no |
| `isn't reached by any test` | no test reaches it | no |
| `is skipped (unviable: …), so it isn't run and needs no test` | it doesn't compile | yes |
| `crashes, and no test can be said to catch it (…)` | it crashed where a crash proves nothing | yes |

An id names its mutant whatever options showed it, so `verify` needs no `--operators` or `--unsafe`
to find one an `--operators=extended` or `--unsafe=all` run showed.

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
  Start at  16:24:19
  Duration  928ms (setup 882ms, coverage 5ms, mutants 20ms)
     Tests  3 on rustc 1.94.0 · 4 workers
     Score  90.9%  threshold 80

 [PASS] 1 missed, above the threshold.
 Next: mutastic verify <id> fixtures/pricing · mutastic run --only=<id> fixtures/pricing
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
   done when: mutastic verify mb2f8ae57 fixtures/pricing passes
   reproduce: mutastic run --only=mb2f8ae57 fixtures/pricing
```

### `jsonl`

One JSON object per line, the `mutastic.events/v1` stream, for tools. Progress is off, so stdout is
only the events. See [Events](events.md).

<!-- output: reference/format-jsonl -->
```text
{"schema":"mutastic.events/v1","edition":"rust","reason":"run-started","project":"pricing","rules":"add or change tests only, never production code","rule":"add or change tests only, never production code","decline":"if a mutant's case can't happen in practice, or the change is one no caller can see, don't contrive a test: say why","profile":{"name":"mutastic","optLevel":"0","overflowChecks":true,"debugAssertions":true},"unsafe":"skip"}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"mc9e13290","fingerprint":"c9e13290e7d1ce44","status":"Killed","operator":"mutastic/return-value","location":{"file":"src/lib.rs","line":29,"column":9,"class":"Pricing","method":"discount"},"mutation":{"replacement":"1","description":"returns `1` instead","bytecode":"0 -> 1"},"coveredBy":["tests::small_orders_pay_full_price"],"killedBy":"tests::small_orders_pay_full_price","caughtBy":"tests::small_orders_pay_full_price","killedWith":"panic","caughtWith":"panic","inputs":{"total":[50]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"mb2f8ae57","fingerprint":"b2f8ae57ca6c48cb","status":"Survived","operator":"mutastic/boundary","location":{"file":"src/lib.rs","line":26,"column":18,"class":"Pricing","method":"discount"},"mutation":{"original":">=","replacement":">","description":"`>=` becomes `>`","bytecode":"total >= 100 -> (total) > (100)"},"coveredBy":["pricing::big_orders_get_ten_percent_off","tests::small_orders_pay_full_price"],"inputs":{"total":[50,250]},"testsRun":2,"durationMs":8,"doneWhen":"mutastic verify mb2f8ae57 fixtures/pricing passes","reproduce":"mutastic run --only=mb2f8ae57 fixtures/pricing","rendered":"missed mb2f8ae57: no test checks `discount()` at `total == 100`\n   at src/lib.rs:26\n   - if total >= 100 {\n   + if total > 100 {\n   covered by: pricing::big_orders_get_ten_percent_off, tests::small_orders_pay_full_price; all still pass with the change\n   evidence: every test calls `discount()` away from the boundary\n   evidence: inputs seen: 50, 250 (none is 100)\n   fix: test `discount()` with `total = 100` and assert the result\n   done when: mutastic verify mb2f8ae57 fixtures/pricing passes\n   reproduce: mutastic run --only=mb2f8ae57 fixtures/pricing\n"}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"md56e8c39","fingerprint":"d56e8c3934d92f41","status":"Killed","operator":"mutastic/remove-conditional","location":{"file":"src/lib.rs","line":26,"column":12,"class":"Pricing","method":"discount"},"mutation":{"replacement":"true","description":"`total >= 100` is always true","bytecode":"total >= 100 -> true"},"coveredBy":["pricing::big_orders_get_ten_percent_off","tests::small_orders_pay_full_price"],"killedBy":"tests::small_orders_pay_full_price","caughtBy":"tests::small_orders_pay_full_price","killedWith":"panic","caughtWith":"panic","inputs":{"total":[50,250]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m1636f6d5","fingerprint":"1636f6d59be655d5","status":"Killed","operator":"mutastic/remove-conditional","location":{"file":"src/lib.rs","line":26,"column":12,"class":"Pricing","method":"discount"},"mutation":{"replacement":"false","description":"`total >= 100` is always false","bytecode":"total >= 100 -> false"},"coveredBy":["pricing::big_orders_get_ten_percent_off","tests::small_orders_pay_full_price"],"killedBy":"pricing::big_orders_get_ten_percent_off","caughtBy":"pricing::big_orders_get_ten_percent_off","killedWith":"panic","caughtWith":"panic","inputs":{"total":[50,250]},"testsRun":2,"durationMs":8}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m6d05659f","fingerprint":"6d05659f42679c6c","status":"Killed","operator":"mutastic/return-value","location":{"file":"src/lib.rs","line":27,"column":20,"class":"Pricing","method":"discount"},"mutation":{"replacement":"0","description":"returns `0` instead","bytecode":"total / 10 -> 0"},"coveredBy":["pricing::big_orders_get_ten_percent_off"],"killedBy":"pricing::big_orders_get_ten_percent_off","caughtBy":"pricing::big_orders_get_ten_percent_off","killedWith":"panic","caughtWith":"panic","inputs":{"total":[250]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m95f96847","fingerprint":"95f96847248b0509","status":"Killed","operator":"mutastic/math","location":{"file":"src/lib.rs","line":27,"column":26,"class":"Pricing","method":"discount"},"mutation":{"original":"/","replacement":"*","description":"`/` becomes `*`","bytecode":"total / 10 -> (total) * (10)"},"coveredBy":["pricing::big_orders_get_ten_percent_off"],"killedBy":"pricing::big_orders_get_ten_percent_off","caughtBy":"pricing::big_orders_get_ten_percent_off","killedWith":"panic","caughtWith":"panic","inputs":{"total":[250]},"testsRun":1,"durationMs":3}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m383e0002","fingerprint":"383e0002195614c2","status":"Killed","operator":"mutastic/return-value","location":{"file":"src/lib.rs","line":37,"column":9,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"1","description":"returns `1` instead","bytecode":"0 -> 1"},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m7d92063c","fingerprint":"7d92063c80e7deae","status":"Killed","operator":"mutastic/boundary","location":{"file":"src/lib.rs","line":34,"column":18,"class":"Pricing","method":"shipping"},"mutation":{"original":"<","replacement":"<=","description":"`<` becomes `<=`","bytecode":"total < self.free_shipping_from -> (total) <= (self.free_shipping_from)"},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m3be6b420","fingerprint":"3be6b420676ea8f8","status":"Killed","operator":"mutastic/remove-conditional","location":{"file":"src/lib.rs","line":34,"column":12,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"true","description":"`total < self.free_shipping_from` is always true","bytecode":"total < self.free_shipping_from -> true"},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m89be540d","fingerprint":"89be540de2153650","status":"Killed","operator":"mutastic/remove-conditional","location":{"file":"src/lib.rs","line":34,"column":12,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"false","description":"`total < self.free_shipping_from` is always false","bytecode":"total < self.free_shipping_from -> false"},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":8}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"mbddcd9c4","fingerprint":"bddcd9c42b757c41","status":"Killed","operator":"mutastic/return-value","location":{"file":"src/lib.rs","line":35,"column":20,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"0","description":"returns `0` instead","bytecode":"5 -> 0"},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"run-finished","toolchain":{"name":"rustc","version":"1.94.0"},"mutants":11,"Killed":10,"Survived":1,"Timeout":0,"NoCoverage":0,"Ignored":0,"RuntimeError":0,"killed":10,"survived":1,"timeout":0,"noCoverage":0,"ignored":0,"runtimeError":0,"notMutated":{"unsafe":0,"cfg":0},"score":90.9,"threshold":80,"passed":true,"coveredScore":90.9,"tests":3,"flaky":[],"flakyEvidence":[],"likelyFlaky":[],"unrelatedCatches":[],"reports":[],"staleIgnores":[],"build":{"rounds":2,"unviable":0},"durationMs":101}
```

## Exit status

| Status | When |
|---|---|
| 0 | `run` passed; every mutant `verify` named is settled (caught, skipped, or crashed where a crash proves nothing); `mutants` listed them |
| 1 | `run`'s score is under the threshold; a mutant `verify` named is missed or not covered |
| 2 | mutastic couldn't run: an unknown option, not a Cargo package, a workspace's root, a test that fails with no mutant in, an id that isn't a mutant of the package |
