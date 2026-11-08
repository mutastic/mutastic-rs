# Events

`mutastic run --format=jsonl` writes one JSON object per line: the `mutastic.events/v1` stream. It's
the format the Java edition writes, described by one JSON Schema for both editions, so a tool, a CI
step or a coding agent's plugin reads either edition the same way.

- **The schema:** [`https://docs.mutastic.io/schemas/events/v1.json`](https://docs.mutastic.io/schemas/events/v1.json),
  with what it promises in [Schemas and stability](https://docs.mutastic.io/reference/schemas/).
- **This repository's copy,** pinned and checked: `crates/mutastic/tests/schema/events-v1.json`.
  Every event the tests' runs write is validated against it.

Each event has `schema` (`mutastic.events/v1`), `edition` (`rust`, where the Java edition writes
`java`), and `reason`, which says which event it is:

| `reason` | When |
|---|---|
| `run-started` | First, once the build is done |
| `mutant-finished` | For each mutant |
| `run-finished` | Last: the totals, the score and the verdict |

This is the pricing fixture's whole stream:

<!-- output: reference/format-jsonl -->
```text
{"schema":"mutastic.events/v1","edition":"rust","reason":"run-started","project":"pricing","rules":"add or change tests only, never production code","rule":"add or change tests only, never production code","decline":"if a mutant's case can't happen in practice, or the change is one no caller can see, don't contrive a test: say why","profile":{"name":"mutastic","optLevel":"0","overflowChecks":true,"debugAssertions":true},"unsafe":"skip"}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"mc9e13290","fingerprint":"c9e13290e7d1ce44","status":"Killed","operator":"mutastic/return-value","location":{"file":"src/lib.rs","line":29,"column":9,"class":"Pricing","method":"discount"},"mutation":{"replacement":"1","description":"returns `1` instead","bytecode":"0 -> 1"},"coveredBy":["tests::small_orders_pay_full_price"],"killedBy":"tests::small_orders_pay_full_price","caughtBy":"tests::small_orders_pay_full_price","killedWith":"panic","caughtWith":"panic","inputs":{"total":[50]},"testsRun":1,"durationMs":8}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"mb2f8ae57","fingerprint":"b2f8ae57ca6c48cb","status":"Survived","operator":"mutastic/boundary","location":{"file":"src/lib.rs","line":26,"column":18,"class":"Pricing","method":"discount"},"mutation":{"original":">=","replacement":">","description":"`>=` becomes `>`","bytecode":"total >= 100 -> (total) > (100)"},"coveredBy":["pricing::big_orders_get_ten_percent_off","tests::small_orders_pay_full_price"],"inputs":{"total":[50,250]},"testsRun":2,"durationMs":12,"doneWhen":"mutastic verify mb2f8ae57 fixtures/pricing passes","reproduce":"mutastic run --only=mb2f8ae57 fixtures/pricing","ignore":"mutastic ignore mb2f8ae57 --reason=\"<why>\" fixtures/pricing","propose":"mutastic ignore mb2f8ae57 --propose --reason=\"<why>\" fixtures/pricing","rendered":"missed mb2f8ae57: no test checks `discount()` at `total == 100`\n   at src/lib.rs:26\n   - if total >= 100 {\n   + if total > 100 {\n   covered by: pricing::big_orders_get_ten_percent_off, tests::small_orders_pay_full_price; all still pass with the change\n   evidence: every test calls `discount()` away from the boundary\n   evidence: inputs seen: 50, 250 (none is 100)\n   fix: test `discount()` with `total = 100` and assert the result\n   done when: mutastic verify mb2f8ae57 fixtures/pricing passes\n   reproduce: mutastic run --only=mb2f8ae57 fixtures/pricing\n   or propose: mutastic ignore mb2f8ae57 --propose --reason=\"<why>\" fixtures/pricing\n"}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"md56e8c39","fingerprint":"d56e8c3934d92f41","status":"Killed","operator":"mutastic/remove-conditional","location":{"file":"src/lib.rs","line":26,"column":12,"class":"Pricing","method":"discount"},"mutation":{"replacement":"true","description":"`total >= 100` is always true","bytecode":"total >= 100 -> true"},"coveredBy":["pricing::big_orders_get_ten_percent_off","tests::small_orders_pay_full_price"],"killedBy":"tests::small_orders_pay_full_price","caughtBy":"tests::small_orders_pay_full_price","killedWith":"panic","caughtWith":"panic","inputs":{"total":[50,250]},"testsRun":1,"durationMs":8}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m1636f6d5","fingerprint":"1636f6d59be655d5","status":"Killed","operator":"mutastic/remove-conditional","location":{"file":"src/lib.rs","line":26,"column":12,"class":"Pricing","method":"discount"},"mutation":{"replacement":"false","description":"`total >= 100` is always false","bytecode":"total >= 100 -> false"},"coveredBy":["pricing::big_orders_get_ten_percent_off","tests::small_orders_pay_full_price"],"killedBy":"pricing::big_orders_get_ten_percent_off","caughtBy":"pricing::big_orders_get_ten_percent_off","killedWith":"panic","caughtWith":"panic","inputs":{"total":[50,250]},"testsRun":2,"durationMs":12}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m6d05659f","fingerprint":"6d05659f42679c6c","status":"Killed","operator":"mutastic/return-value","location":{"file":"src/lib.rs","line":27,"column":20,"class":"Pricing","method":"discount"},"mutation":{"replacement":"0","description":"returns `0` instead","bytecode":"total / 10 -> 0"},"coveredBy":["pricing::big_orders_get_ten_percent_off"],"killedBy":"pricing::big_orders_get_ten_percent_off","caughtBy":"pricing::big_orders_get_ten_percent_off","killedWith":"panic","caughtWith":"panic","inputs":{"total":[250]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m95f96847","fingerprint":"95f96847248b0509","status":"Killed","operator":"mutastic/math","location":{"file":"src/lib.rs","line":27,"column":26,"class":"Pricing","method":"discount"},"mutation":{"original":"/","replacement":"*","description":"`/` becomes `*`","bytecode":"total / 10 -> (total) * (10)"},"coveredBy":["pricing::big_orders_get_ten_percent_off"],"killedBy":"pricing::big_orders_get_ten_percent_off","caughtBy":"pricing::big_orders_get_ten_percent_off","killedWith":"panic","caughtWith":"panic","inputs":{"total":[250]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m383e0002","fingerprint":"383e0002195614c2","status":"Killed","operator":"mutastic/return-value","location":{"file":"src/lib.rs","line":37,"column":9,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"1","description":"returns `1` instead","bytecode":"0 -> 1"},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m7d92063c","fingerprint":"7d92063c80e7deae","status":"Killed","operator":"mutastic/boundary","location":{"file":"src/lib.rs","line":34,"column":18,"class":"Pricing","method":"shipping"},"mutation":{"original":"<","replacement":"<=","description":"`<` becomes `<=`","bytecode":"total < self.free_shipping_from -> (total) <= (self.free_shipping_from)"},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m3be6b420","fingerprint":"3be6b420676ea8f8","status":"Killed","operator":"mutastic/remove-conditional","location":{"file":"src/lib.rs","line":34,"column":12,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"true","description":"`total < self.free_shipping_from` is always true","bytecode":"total < self.free_shipping_from -> true"},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m89be540d","fingerprint":"89be540de2153650","status":"Killed","operator":"mutastic/remove-conditional","location":{"file":"src/lib.rs","line":34,"column":12,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"false","description":"`total < self.free_shipping_from` is always false","bytecode":"total < self.free_shipping_from -> false"},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"mbddcd9c4","fingerprint":"bddcd9c42b757c41","status":"Killed","operator":"mutastic/return-value","location":{"file":"src/lib.rs","line":35,"column":20,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"0","description":"returns `0` instead","bytecode":"5 -> 0"},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"killedBy":"tests::shipping_is_free_from_the_threshold","caughtBy":"tests::shipping_is_free_from_the_threshold","killedWith":"panic","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":3}
{"schema":"mutastic.events/v1","edition":"rust","reason":"run-finished","toolchain":{"name":"rustc","version":"1.94.0"},"mutants":11,"Killed":10,"Survived":1,"Timeout":0,"NoCoverage":0,"Ignored":0,"RuntimeError":0,"killed":10,"survived":1,"timeout":0,"noCoverage":0,"ignored":0,"runtimeError":0,"notMutated":{"unsafe":0,"cfg":0},"score":90.9,"threshold":80,"passed":true,"coveredScore":90.9,"tests":3,"flaky":[],"flakyEvidence":[],"likelyFlaky":[],"unrelatedCatches":[],"reports":["target/mutastic/mutation.json","target/mutastic/mutation.html"],"staleIgnores":[],"build":{"rounds":2,"unviable":0},"durationMs":104}
```

Read what you know and let the rest pass: ignore a field you don't know, and an event whose `reason`
you don't know, and treat a status you don't know as neither caught nor missed. Any release may add
a field, an event or a value.

## What the Rust edition adds

Every field the schema requires is there, with the schema's meaning. A few say what only Rust has;
the schema names each as the Rust edition's, and a reader written for the Java edition passes over
them.

| Event | Field | Means |
|---|---|---|
| every event | `edition` | `rust`. The schema asks the Java edition's own fields (`jdk`, a missed mutant's `ignore` and `propose`, a mutation's `bytecode`) only of events whose `edition` is `java` |
| `run-started` | `profile` | How mutastic's build was made: `name`, `optLevel`, `overflowChecks`, `debugAssertions`, as Cargo reports them. See [Profiles](../concepts/profiles.md#what-the-run-records) |
| `run-started` | `unsafe` | `skip`, `enclosing` or `all`: how unsafe code was treated |
| `mutant-finished` | `status: "RuntimeError"` | Crashed where a crash proves nothing; out of the score. See [Unsafe code](../concepts/unsafe.md) |
| `mutant-finished` | `nearUnsafe` | Why the mutant is near unsafe code: `called from unsafe code`, `in a module with unsafe code`, and, with `--unsafe`, `in a function with an unsafe block` or `inside unsafe code` |
| `mutant-finished` | `caughtWith` | How the test failed: `panic`, `panic: attempt to subtract with overflow` for a panic the language raised, or `crash: SIGABRT` when the process died and no test is named |
| `mutant-finished` | `location.column` | The column of the change, from 1 |
| `run-finished` | `toolchain` | `{"name": "rustc", "version": …}`, the compiler that built the tests, where the Java edition also writes `jdk` |
| `run-finished` | `runtimeError` | How many crashed where a crash proves nothing |
| `run-finished` | `notMutated.unsafe` | How many mutants were left out in or around unsafe code |
| `run-finished` | `notMutated.cfg` | How many mutants were left out because their code is under a `cfg` this build doesn't have. See [Troubleshooting](../troubleshooting.md#code-behind-other-cfgs) |
| `run-finished` | `build` | `rounds`, the builds it took, and `unviable`, the mutants withdrawn |

Some fields keep the Java edition's names where Rust has other words. In `location`, `class` is the
item that holds the function (`pricing::Pricing`, or the module for a free function), and `method`
is the function's name. In `mutation`, `bytecode` is optional outside the Java edition; the Rust
edition still writes it, as the whole changed code before and after (`total >= 100 -> true`), which
no other field gives for every kind of change.

## Names that change in 1.0

The schema renamed some fields from 0.6.0 and keeps the old names until 1.0. The Rust edition writes
both, so read the new ones:

| Old, removed in 1.0 | New |
|---|---|
| `rules` | `rule` |
| `killedBy` | `caughtBy` |
| `killedWith` | `caughtWith` |
| `Killed`, `Survived`, `Timeout`, `NoCoverage`, `Ignored`, `RuntimeError` | `killed`, `survived`, `timeout`, `noCoverage`, `ignored`, `runtimeError` |

## Not there yet

The commands a missed mutant's `ignore` and `propose` would offer don't exist in the Rust edition
yet, so it leaves those fields out, as the schema allows for `edition: "rust"`: output never offers
a command that doesn't exist.

The lists in `run-finished` the Java edition fills (`flaky`, `flakyEvidence`, `likelyFlaky`,
`unrelatedCatches`, `reports`, `staleIgnores`) are there and empty: the Rust edition doesn't do what
fills them yet.
