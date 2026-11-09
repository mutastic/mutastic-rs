# Output

mutastic prints its result on stdout and its progress on stderr. How the result looks is
`--format`'s choice.

## Formats

`--format` chooses how a run prints its result. At a terminal it's `full`; when stdout isn't a
terminal, such as in CI or a pipe, it's `concise`; with `MUTASTIC_AGENT=1` it's `agent`.
`MUTASTIC_FORMAT` sets it too: see [Environment and exit codes](../reference/commands.md#environment).

### `full`

A card for each line with a missed mutant, for a person reading at a terminal. See
[Getting started](../getting-started.md#3-read-the-card) for how to read one.

### `concise`

One line for each line with a missed mutant, with its ids, then the summary. For logs.

<!-- output: concepts/format-concise -->
```text

 Missed ────────────────────────────────────────────────────────────
 ✗ src/lib.rs:26 no test checks discount() at total == 100 mb2f8ae57

     Files  0 clean | 1 with misses (1)
   Mutants  ✓ 10 caught · ✗ 1 missed (11)
  Worst at  Pricing::discount() 1 ✗
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  13 (33 without coverage selection)
  Start at  13:21:36
  Duration  1.3s (setup 1.2s, coverage 4ms, mutants 29ms)
     Tests  3 on rustc 1.98.0 · 4 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  90.9%  threshold 80

 [PASS] 1 missed, above the threshold.
 Next: mutastic verify <id> fixtures/pricing · mutastic run --only=<id> fixtures/pricing · mutastic explain <id> fixtures/pricing
```

### `agent`

For a coding agent: the verdict and the rules first, then each missed mutant with its evidence,
the fix, and the commands that prove it. It shows the most telling first, and says how to get the
rest.

<!-- output: concepts/format-agent -->
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
only the events. See [Events](../reference/events.md).

<!-- output: concepts/format-jsonl -->
```text
{"schema":"mutastic.events/v1","edition":"rust","reason":"run-started","project":"pricing","rules":"add or change tests only, never production code","rule":"add or change tests only, never production code","decline":"if a mutant's case can't happen in practice, or the change is one no caller can see, don't contrive a test: say why","profile":{"name":"mutastic","optLevel":"0","overflowChecks":true,"debugAssertions":true},"unsafe":"skip"}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"mc9e13290","fingerprint":"c9e13290e7d1ce44","status":"Killed","operator":"mutastic/return-value","location":{"file":"src/lib.rs","line":29,"column":9,"class":"Pricing","method":"discount"},"mutation":{"replacement":"1","description":"returns `1` instead","applied":{"level":"source","text":"0 -> 1"}},"coveredBy":["tests::small_orders_pay_full_price"],"killedBy":"tests::small_orders_pay_full_price","caughtBy":"tests::small_orders_pay_full_price","killedWith":"panic","caughtWith":"panic","inputs":{"total":[50]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"mb2f8ae57","fingerprint":"b2f8ae57ca6c48cb","status":"Survived","operator":"mutastic/boundary","location":{"file":"src/lib.rs","line":26,"column":18,"class":"Pricing","method":"discount"},"mutation":{"original":">=","replacement":">","description":"`>=` becomes `>`","applied":{"level":"source","text":"total >= 100 -> (total) > (100)"}},"coveredBy":["pricing::big_orders_get_ten_percent_off","tests::small_orders_pay_full_price"],"inputs":{"total":[50,250]},"testsRun":2,"durationMs":8,"doneWhen":"mutastic verify mb2f8ae57 fixtures/pricing passes","reproduce":"mutastic run --only=mb2f8ae57 fixtures/pricing","ignore":"mutastic ignore mb2f8ae57 --reason=\"<why>\" fixtures/pricing","propose":"mutastic ignore mb2f8ae57 --propose --reason=\"<why>\" fixtures/pricing","suggestedTest":{"testClass":"tests","name":"discount_with_total_100","code":"#[test]\nfn discount_with_total_100() {\n    assert_eq!(Pricing::new().discount(100), 10); // today 10 · with this mutant 0\n}","call":"Pricing::new().discount(100)","today":"10","withMutant":"0"},"suggestion":{"call":"Pricing::new().discount(100)","parameters":["total"],"arguments":[100],"receiver":[{"type":"Pricing","arguments":[]}],"today":"10","withMutant":"0"},"rendered":"missed mb2f8ae57: no test checks `discount()` at `total == 100`\n   at src/lib.rs:26\n   - if total >= 100 {\n   + if total > 100 {\n   covered by: pricing::big_orders_get_ten_percent_off, tests::small_orders_pay_full_price; all still pass with the change\n   evidence: every test calls `discount()` away from the boundary\n   evidence: inputs seen: 50, 250 (none is 100)\n   fix: test `discount()` with `total = 100` and assert the result\n   test to add in mod tests:\n     #[test]\n     fn discount_with_total_100() {\n         assert_eq!(Pricing::new().discount(100), 10); // today 10 · with this mutant 0\n     }\n     // check today's value is the one you intend: the test pins what the code does now\n   done when: mutastic verify mb2f8ae57 fixtures/pricing passes\n   reproduce: mutastic run --only=mb2f8ae57 fixtures/pricing\n   or propose: mutastic ignore mb2f8ae57 --propose --reason=\"<why>\" fixtures/pricing\n"}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"md56e8c39","fingerprint":"d56e8c3934d92f41","status":"Killed","operator":"mutastic/remove-conditional","location":{"file":"src/lib.rs","line":26,"column":12,"class":"Pricing","method":"discount"},"mutation":{"replacement":"true","description":"`total >= 100` is always true","applied":{"level":"source","text":"total >= 100 -> true"}},"coveredBy":["pricing::big_orders_get_ten_percent_off","tests::small_orders_pay_full_price"],"killedBy":"tests::small_orders_pay_full_price","caughtBy":"tests::small_orders_pay_full_price","killedWith":"panic","caughtWith":"panic","inputs":{"total":[50,250]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m1636f6d5","fingerprint":"1636f6d59be655d5","status":"Killed","operator":"mutastic/remove-conditional","location":{"file":"src/lib.rs","line":26,"column":12,"class":"Pricing","method":"discount"},"mutation":{"replacement":"false","description":"`total >= 100` is always false","applied":{"level":"source","text":"total >= 100 -> false"}},"coveredBy":["pricing::big_orders_get_ten_percent_off","tests::small_orders_pay_full_price"],"killedBy":"pricing::big_orders_get_ten_percent_off","caughtBy":"pricing::big_orders_get_ten_percent_off","killedWith":"panic","caughtWith":"panic","inputs":{"total":[50,250]},"testsRun":2,"durationMs":8}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m6d05659f","fingerprint":"6d05659f42679c6c","status":"Killed","operator":"mutastic/return-value","location":{"file":"src/lib.rs","line":27,"column":20,"class":"Pricing","method":"discount"},"mutation":{"replacement":"0","description":"returns `0` instead","applied":{"level":"source","text":"total / 10 -> 0"}},"coveredBy":["pricing::big_orders_get_ten_percent_off"],"killedBy":"pricing::big_orders_get_ten_percent_off","caughtBy":"pricing::big_orders_get_ten_percent_off","killedWith":"panic","caughtWith":"panic","inputs":{"total":[250]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m95f96847","fingerprint":"95f96847248b0509","status":"Killed","operator":"mutastic/math","location":{"file":"src/lib.rs","line":27,"column":26,"class":"Pricing","method":"discount"},"mutation":{"original":"/","replacement":"*","description":"`/` becomes `*`","applied":{"level":"source","text":"total / 10 -> (total) * (10)"}},"coveredBy":["pricing::big_orders_get_ten_percent_off"],"killedBy":"pricing::big_orders_get_ten_percent_off","caughtBy":"pricing::big_orders_get_ten_percent_off","killedWith":"panic","caughtWith":"panic","inputs":{"total":[250]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m383e0002","fingerprint":"383e0002195614c2","status":"Killed","operator":"mutastic/return-value","location":{"file":"src/lib.rs","line":37,"column":9,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"1","description":"returns `1` instead","applied":{"level":"source","text":"0 -> 1"}},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m7d92063c","fingerprint":"7d92063c80e7deae","status":"Killed","operator":"mutastic/boundary","location":{"file":"src/lib.rs","line":34,"column":18,"class":"Pricing","method":"shipping"},"mutation":{"original":"<","replacement":"<=","description":"`<` becomes `<=`","applied":{"level":"source","text":"total < self.free_shipping_from -> (total) <= (self.free_shipping_from)"}},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m3be6b420","fingerprint":"3be6b420676ea8f8","status":"Killed","operator":"mutastic/remove-conditional","location":{"file":"src/lib.rs","line":34,"column":12,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"true","description":"`total < self.free_shipping_from` is always true","applied":{"level":"source","text":"total < self.free_shipping_from -> true"}},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m89be540d","fingerprint":"89be540de2153650","status":"Killed","operator":"mutastic/remove-conditional","location":{"file":"src/lib.rs","line":34,"column":12,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"false","description":"`total < self.free_shipping_from` is always false","applied":{"level":"source","text":"total < self.free_shipping_from -> false"}},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"mbddcd9c4","fingerprint":"bddcd9c42b757c41","status":"Killed","operator":"mutastic/return-value","location":{"file":"src/lib.rs","line":35,"column":20,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"0","description":"returns `0` instead","applied":{"level":"source","text":"5 -> 0"}},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"run-finished","toolchain":{"name":"rustc","version":"1.94.0"},"mutants":11,"Killed":10,"Survived":1,"Timeout":0,"NoCoverage":0,"Ignored":0,"RuntimeError":0,"killed":10,"survived":1,"timeout":0,"noCoverage":0,"ignored":0,"runtimeError":0,"notMutated":{"unsafe":0,"cfg":0},"score":90.9,"threshold":80,"passed":true,"coveredScore":90.9,"tests":3,"workers":4,"workersStarted":4,"coverageMs":4,"flaky":[],"flakyEvidence":[],"likelyFlaky":[],"unrelatedCatches":[],"reports":["target/mutastic/mutation.json","target/mutastic/mutation.html"],"staleIgnores":[],"build":{"rounds":2,"unviable":0,"setupMs":62},"durationMs":110}
```

## Streams

The result goes to stdout and progress to stderr, so `mutastic run fixtures/pricing > result.txt`
keeps only the result.

## One mutant, explained

`mutastic explain <id> [dir]` runs that one mutant and shows its card when it's missed, or what
became of it otherwise, then what it is: the change, its operator, how its id is made, where it is,
and how the last full run went for it, from that run's `mutation.json`. On the pricing fixture,
after a run:

```sh
mutastic explain mb2f8ae57 fixtures/pricing
```

<!-- output: concepts/explain-missed -->
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

<!-- output: concepts/explain-caught -->
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
