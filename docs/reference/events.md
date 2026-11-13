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
| `phase-started` | A phase begins: `reading`, `building`, `coverage`, `mutants` or `checking`. Only these and `phase-finished` come before `run-started` |
| `phase-finished` | A phase ends, with its `durationMs` |
| `run-started` | Once the build is done, as it carries the build's profile |
| `progress` | Every 30 seconds (`MUTASTIC_HEARTBEAT`), once `run-started` is out: the phase's `elapsedMs`, and what it knows: `done` and `total`; for mutants `weighted`, `leftMs`, the fates so far and how many are `running` |
| `mutant-finished` | For each mutant, after the phases: each carries what the checking phase found |
| `run-finished` | Last: the totals, the score and the verdict |
| `next-item` | From `mutastic next`: each item shown, its lead mutant as `mutant-finished` has it, with `ids` and `rank`. See [Working through misses](../concepts/next.md#for-a-tool) |
| `next-finished` | Last, from `mutastic next`: how many lines and mutants are `queued`, `shown`, and `waiting` on a person |

This is the pricing fixture's whole stream:

<!-- output: concepts/format-jsonl -->
```text
{"schema":"mutastic.events/v1","edition":"rust","reason":"phase-started","phase":"reading"}
{"schema":"mutastic.events/v1","edition":"rust","reason":"phase-finished","phase":"reading","durationMs":38}
{"schema":"mutastic.events/v1","edition":"rust","reason":"phase-started","phase":"building"}
{"schema":"mutastic.events/v1","edition":"rust","reason":"phase-finished","phase":"building","durationMs":27}
{"schema":"mutastic.events/v1","edition":"rust","reason":"run-started","project":"pricing","rule":"add or change tests only; never change production code to make a mutant go away: tell the person","decline":"if a mutant's case can't happen in practice, or the change is one no caller can see, don't contrive a test: propose ignoring it, for a person to accept, as the kind it is (equivalent, unreachable, telemetry, deprecated or accepted-risk), with the reason in two sentences: the one case where it differs, and why that can't happen","profile":{"name":"mutastic","optLevel":"0","overflowChecks":true,"debugAssertions":true},"unsafe":"skip"}
{"schema":"mutastic.events/v1","edition":"rust","reason":"phase-started","phase":"coverage"}
{"schema":"mutastic.events/v1","edition":"rust","reason":"phase-finished","phase":"coverage","durationMs":23}
{"schema":"mutastic.events/v1","edition":"rust","reason":"phase-started","phase":"mutants"}
{"schema":"mutastic.events/v1","edition":"rust","reason":"phase-finished","phase":"mutants","durationMs":8}
{"schema":"mutastic.events/v1","edition":"rust","reason":"phase-started","phase":"checking"}
{"schema":"mutastic.events/v1","edition":"rust","reason":"phase-finished","phase":"checking","durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"mc9e13290","fingerprint":"c9e13290e7d1ce44","status":"Killed","operator":"mutastic/return-value","docs":"https://docs.mutastic.io/rust/operators/returns/#return-value","location":{"file":"src/lib.rs","line":29,"column":9,"class":"Pricing","method":"discount"},"mutation":{"replacement":"1","description":"returns `1` instead","applied":{"level":"source","text":"0 -> 1"}},"coveredBy":["tests::small_orders_pay_full_price"],"caughtBy":"tests::small_orders_pay_full_price","caughtWith":"panic","inputs":{"total":[50]},"testsRun":1,"durationMs":2}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"mb2f8ae57","fingerprint":"b2f8ae57ca6c48cb","status":"Survived","operator":"mutastic/boundary","docs":"https://docs.mutastic.io/rust/operators/conditions/#boundary","location":{"file":"src/lib.rs","line":26,"column":18,"class":"Pricing","method":"discount"},"mutation":{"original":">=","replacement":">","description":"`>=` becomes `>`","applied":{"level":"source","text":"total >= 100 -> (total) > (100)"}},"coveredBy":["tests::small_orders_pay_full_price","pricing::big_orders_get_ten_percent_off"],"inputs":{"total":[50,250]},"testsRun":2,"durationMs":4,"doneWhen":"mutastic verify mb2f8ae57 fixtures/pricing passes","reproduce":"mutastic run --only=mb2f8ae57 fixtures/pricing","ignore":"mutastic ignore mb2f8ae57 --reason=\"<why>\" fixtures/pricing","propose":"mutastic ignore mb2f8ae57 --propose --as=<equivalent|unreachable|…> --reason=\"Differs only when <case>. That can't happen because <why>.\" fixtures/pricing","suggestedTest":{"testClass":"tests","name":"discount_at_the_total_edge","code":"#[test]\nfn discount_at_the_total_edge() {\n    assert_eq!(Pricing::new().discount(100), 10); // today 10 · with this mutant 0\n}","call":"Pricing::new().discount(100)","today":"10","withMutant":"0"},"suggestion":{"call":"Pricing::new().discount(100)","parameters":["total"],"arguments":[100],"receiver":[{"type":"Pricing","arguments":[]}],"today":"10","withMutant":"0"},"rendered":"missed mb2f8ae57: no test checks `discount()` at `total == 100`\n   at src/lib.rs:26\n   - if total >= 100 {\n   + if total > 100 {\n   covered by: tests::small_orders_pay_full_price, pricing::big_orders_get_ten_percent_off; all still pass with the change\n   evidence: every test calls `discount()` away from the boundary\n   evidence: inputs seen: 50, 250 (none is 100)\n   fix: test `discount()` with `total = 100` and assert the result\n   test to add in mod tests:\n     #[test]\n     fn discount_at_the_total_edge() {\n         assert_eq!(Pricing::new().discount(100), 10); // today 10 · with this mutant 0\n     }\n     // check today's value is the one you intend: the test pins what the code does now\n   done when: mutastic verify mb2f8ae57 fixtures/pricing passes\n   reproduce: mutastic run --only=mb2f8ae57 fixtures/pricing\n   or propose: mutastic ignore mb2f8ae57 --propose --as=<equivalent|unreachable|…> --reason=\"Differs only when <case>. That can't happen because <why>.\" fixtures/pricing\n"}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"md56e8c39","fingerprint":"d56e8c3934d92f41","status":"Killed","operator":"mutastic/remove-conditional","docs":"https://docs.mutastic.io/rust/operators/conditions/#remove-conditional","location":{"file":"src/lib.rs","line":26,"column":12,"class":"Pricing","method":"discount"},"mutation":{"replacement":"true","description":"`total >= 100` is always true","applied":{"level":"source","text":"total >= 100 -> true"}},"coveredBy":["tests::small_orders_pay_full_price","pricing::big_orders_get_ten_percent_off"],"caughtBy":"tests::small_orders_pay_full_price","caughtWith":"panic","inputs":{"total":[50,250]},"testsRun":1,"durationMs":2}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m1636f6d5","fingerprint":"1636f6d59be655d5","status":"Killed","operator":"mutastic/remove-conditional","docs":"https://docs.mutastic.io/rust/operators/conditions/#remove-conditional","location":{"file":"src/lib.rs","line":26,"column":12,"class":"Pricing","method":"discount"},"mutation":{"replacement":"false","description":"`total >= 100` is always false","applied":{"level":"source","text":"total >= 100 -> false"}},"coveredBy":["tests::small_orders_pay_full_price","pricing::big_orders_get_ten_percent_off"],"caughtBy":"pricing::big_orders_get_ten_percent_off","caughtWith":"panic","inputs":{"total":[50,250]},"testsRun":2,"durationMs":4}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m6d05659f","fingerprint":"6d05659f42679c6c","status":"Killed","operator":"mutastic/return-value","docs":"https://docs.mutastic.io/rust/operators/returns/#return-value","location":{"file":"src/lib.rs","line":27,"column":20,"class":"Pricing","method":"discount"},"mutation":{"replacement":"0","description":"returns `0` instead","applied":{"level":"source","text":"total / 10 -> 0"}},"coveredBy":["pricing::big_orders_get_ten_percent_off"],"caughtBy":"pricing::big_orders_get_ten_percent_off","caughtWith":"panic","inputs":{"total":[250]},"testsRun":1,"durationMs":2}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m95f96847","fingerprint":"95f96847248b0509","status":"Killed","operator":"mutastic/math","docs":"https://docs.mutastic.io/rust/operators/arithmetic/#math","location":{"file":"src/lib.rs","line":27,"column":26,"class":"Pricing","method":"discount"},"mutation":{"original":"/","replacement":"*","description":"`/` becomes `*`","applied":{"level":"source","text":"total / 10 -> (total) * (10)"}},"coveredBy":["pricing::big_orders_get_ten_percent_off"],"caughtBy":"pricing::big_orders_get_ten_percent_off","caughtWith":"panic","inputs":{"total":[250]},"testsRun":1,"durationMs":2}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m383e0002","fingerprint":"383e0002195614c2","status":"Killed","operator":"mutastic/return-value","docs":"https://docs.mutastic.io/rust/operators/returns/#return-value","location":{"file":"src/lib.rs","line":37,"column":9,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"1","description":"returns `1` instead","applied":{"level":"source","text":"0 -> 1"}},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"caughtBy":"tests::shipping_is_free_from_the_threshold","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":2}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m7d92063c","fingerprint":"7d92063c80e7deae","status":"Killed","operator":"mutastic/boundary","docs":"https://docs.mutastic.io/rust/operators/conditions/#boundary","location":{"file":"src/lib.rs","line":34,"column":18,"class":"Pricing","method":"shipping"},"mutation":{"original":"<","replacement":"<=","description":"`<` becomes `<=`","applied":{"level":"source","text":"total < self.free_shipping_from -> (total) <= (self.free_shipping_from)"}},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"caughtBy":"tests::shipping_is_free_from_the_threshold","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":2}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m3be6b420","fingerprint":"3be6b420676ea8f8","status":"Killed","operator":"mutastic/remove-conditional","docs":"https://docs.mutastic.io/rust/operators/conditions/#remove-conditional","location":{"file":"src/lib.rs","line":34,"column":12,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"true","description":"`total < self.free_shipping_from` is always true","applied":{"level":"source","text":"total < self.free_shipping_from -> true"}},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"caughtBy":"tests::shipping_is_free_from_the_threshold","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":2}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"m89be540d","fingerprint":"89be540de2153650","status":"Killed","operator":"mutastic/remove-conditional","docs":"https://docs.mutastic.io/rust/operators/conditions/#remove-conditional","location":{"file":"src/lib.rs","line":34,"column":12,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"false","description":"`total < self.free_shipping_from` is always false","applied":{"level":"source","text":"total < self.free_shipping_from -> false"}},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"caughtBy":"tests::shipping_is_free_from_the_threshold","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":2}
{"schema":"mutastic.events/v1","edition":"rust","reason":"mutant-finished","id":"mbddcd9c4","fingerprint":"bddcd9c42b757c41","status":"Killed","operator":"mutastic/return-value","docs":"https://docs.mutastic.io/rust/operators/returns/#return-value","location":{"file":"src/lib.rs","line":35,"column":20,"class":"Pricing","method":"shipping"},"mutation":{"replacement":"0","description":"returns `0` instead","applied":{"level":"source","text":"5 -> 0"}},"coveredBy":["tests::shipping_is_free_from_the_threshold"],"caughtBy":"tests::shipping_is_free_from_the_threshold","caughtWith":"panic","inputs":{"total":[39,40]},"testsRun":1,"durationMs":2}
{"schema":"mutastic.events/v1","edition":"rust","reason":"run-finished","toolchain":{"name":"rustc","version":"1.99.1"},"mutants":11,"killed":10,"survived":1,"timeout":0,"noCoverage":0,"ignored":0,"runtimeError":0,"notMutated":{"unsafe":0,"cfg":0},"score":90.9,"threshold":80,"passed":true,"coveredScore":90.9,"tests":3,"workers":4,"workersStarted":4,"coverageMs":2,"flaky":[],"flakyEvidence":[],"likelyFlaky":[],"unrelatedCatches":[],"reports":["target/mutastic/mutation.json","target/mutastic/mutation.html"],"staleIgnores":[],"build":{"rounds":2,"unviable":0,"setupMs":58},"durationMs":92}
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
| every event | `edition` | `rust`. The schema asks the Java edition's own field, `run-finished`'s `jdk`, only of events whose `edition` is `java` |
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
| `run-finished` | `build` | `rounds`, the builds it took, `unviable`, the mutants withdrawn, and `setupMs`, how long reading the package and building it took, before the coverage pass |

A run split across CI jobs writes what the Java edition's does, so `--from-shards` can put the
whole run together from the shares' events: `run-started`'s `share` says which share a run is
(`number`, `of`, the mutastic `version` and a hash of the `code`, every Rust file and Cargo manifest
in the workspace); a missed mutant with a suggested test carries the call it was found by, whole,
in `suggestion`; and `run-finished` says how many `workers` ran side by side and how long the
coverage pass took, `coverageMs`. `workersStarted` is the same as `workers`, as a Rust worker is a
thread, never stopped and started again. See
[One run across CI jobs](../concepts/how-a-run-works.md#one-run-across-ci-jobs).

Some fields keep the Java edition's names where Rust has other words. In `location`, `class` is the
item that holds the function (`pricing::Pricing`, or the module for a free function), and `method`
is the function's name. In `mutation`, `applied` is the change as the engine makes it, and at what
level: the Java edition's is `bytecode`, to the instruction (`IF_ICMPLT -> IF_ICMPLE`); the Rust
edition's is `source`, the whole changed code before and after (`total >= 100 -> true`), which no
other field gives for every kind of change. A later engine may add a level, so accept one you don't
know and read its `text` as opaque.

## What stays the same

Before 1.0.0 the schema may change; from 1.0.0 it only gains. Within v1, a field keeps its name and
its type, a field every event has stays in every event, and anything new is optional, so a reader
that takes what it knows keeps working, release after release. A change that would break a reader
comes only with a major release, as `mutastic.events/v2`. The words in some fields (`rendered`,
`rule`, `decline`, a mutation's `description`, and the commands offered with a missed mutant) are
for people and agents to read, and can change from one release to the next; the field stays.
[Schemas and stability](https://docs.mutastic.io/reference/schemas/) says exactly what counts as
breaking.

## Not there yet

`flaky` names each test that failed, then passed when run again, and `flakyEvidence` says what it
reaches that varies, as the Java edition's do. A Rust test fails with a panic, not a chain of
exceptions, so each one's `failure` is empty and `outside` is `false`. The lists the Java edition
fills that the Rust edition doesn't yet (`likelyFlaky`, `unrelatedCatches`) are there and empty.
