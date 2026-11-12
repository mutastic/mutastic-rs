# Working through misses

A run on code that has never been mutation tested can miss hundreds of mutants, and a list that
long goes unread. `mutastic next` hands them over one at a time instead: the one most worth a test
first, why it's first, the test to write, and the command that says when it's done.

```sh
mutastic next            # the head of the queue
mutastic next --top=5    # the first few, spread over the code
```

Write the test, run its `done when` command, and run `mutastic next` again. Each time, `next` runs
the mutants that were still uncaught, so any a new test catches drop out of the queue, not only the
one it was written for. While the code under test is the same it runs only those; once the code
changes, it runs every mutant again, since a change can make new ones. A test is never the code
under test: one written in `tests/`, or in a `#[cfg(test)]` module beside the code, leaves it as it
was.

On the ranking fixture, whose three missed lines are found in the opposite order to the one
they're ranked in:

```sh
mutastic next fixtures/ranking
```

<!-- output: concepts/next -->
```text
Next: 1 of 3 lines with missed mutants in ranking

 [✗ MISSED] no test checks is_small() at n == 10 · md0d6db64
    ┌─ src/lib.rs:15
    │
 14 │ pub fn is_small(n: u32) -> bool {
 15 │     n < 10
    │       ^ < changed to <=
 16 │ }
    │
    = mutant  - n < 10
              + n <= 10
    = also    ✗ no test checks is_small() returning false here · m2e329a72
              + true
    = also    ✗ no test needs n < 10 to be false in is_small() · m729a5380
              + true
    = note: every test calls is_small() away from the boundary
            inputs seen: 4, 5, 6 (none is 10)
            covered by tests::five_doubled_is_more_than_one_and_small, tests::four_doubled_is_more_than_one_and_small, tests::six_doubled_is_more_than_one_and_small (all 3 covering tests still pass)
    = help: test is_small() with n = 10 and assert the result

            // in mod tests
            #[test]
            fn is_small_with_n_10() {
                assert!(!is_small(10)); // today false · with md0d6db64 true
            }
            check today's value is the one you intend: the test pins what the code does now

    done when  mutastic verify md0d6db64,m2e329a72,m729a5380 fixtures/ranking passes
    reproduce  mutastic run --only=md0d6db64,m2e329a72,m729a5380 fixtures/ranking
    or ignore  mutastic ignore <id> --reason="<why>" fixtures/ranking for each of md0d6db64, m2e329a72, m729a5380
 Ranked first because
   a condition or a removal: it changes what the code decides or does
   3 tests run it, and none checks it

Then: mutastic next fixtures/ranking, for the next one
```

## The order

The queue is in the order a run ranks its misses, the order the agent format lists them in, and
each item says why it's where it is:

- **A condition or a removal** comes before arithmetic and other changes: one no test checks
  changes what the code decides or does, while arithmetic is more often noise.
- **Among those, the line more tests run** comes first: many tests reach it and none checks it.
  That this helps is a hypothesis, still to be measured, not a finding.
- **Then where it is in the code.**

The mutants on one line are one item, as they share a card. `--top` shows at most one item for a
function and two for a file, so one function with many misses can't fill the list.

The Java edition puts new misses first, where a baseline says which misses a project already had.
The Rust edition has no baseline yet: see
[Differences from the Java edition](../differences-from-java.md#commands).

## For an agent

With `--format=agent`, `next` gives exactly one task: the mutant and its line as it changes, why
it's first, the test to write, the rules (tests only, never production code changed to make a
mutant go away, and never delete, disable or loosen a test), its `done when`, how to decline it
(proposing an ignore, for a person to accept), and to run `next` again after. The order is
mutastic's: an agent works the head of the queue, or declines it.

<!-- output: concepts/next-agent -->
```text
mutastic next: 1 of 3 lines with missed mutants in ranking; close this one, then ask for the next
why first: a condition or a removal: it changes what the code decides or does; 3 tests run it, and none checks it
rules: add or change tests only; never change production code to make a mutant go away: tell the person, and never delete, disable or loosen a test; it's done when `mutastic verify md0d6db64,m2e329a72,m729a5380 fixtures/ranking` passes
decline: if a mutant's case can't happen in practice, or the change is one no caller can see, don't contrive a test: propose ignoring it, for a person to accept, as the kind it is (equivalent, unreachable, telemetry, deprecated or accepted-risk), with the reason in two sentences: the one case where it differs, and why that can't happen (`mutastic ignore <id> --propose --as=<equivalent|unreachable|…> --reason="Differs only when <case>. That can't happen because <why>." fixtures/ranking`, for each of md0d6db64, m2e329a72, m729a5380)

missed md0d6db64: no test checks `is_small()` at `n == 10`
   at src/lib.rs:15
   - n < 10
   + n <= 10
   also missed m2e329a72: no test checks `is_small()` returning `false` here
     + true
   also missed m729a5380: no test needs `n < 10` to be false in `is_small()`
     + true
   covered by: tests::five_doubled_is_more_than_one_and_small, tests::four_doubled_is_more_than_one_and_small, tests::six_doubled_is_more_than_one_and_small; all still pass with the change
   evidence: every test calls `is_small()` away from the boundary
   evidence: inputs seen: 4, 5, 6 (none is 10)
   fix: test `is_small()` with `n = 10` and assert the result
   test to add in mod tests:
     #[test]
     fn is_small_with_n_10() {
         assert!(!is_small(10)); // today false · with md0d6db64 true
     }
     // check today's value is the one you intend: the test pins what the code does now
   done when: mutastic verify md0d6db64,m2e329a72,m729a5380 fixtures/ranking passes
   reproduce: mutastic run --only=md0d6db64,m2e329a72,m729a5380 fixtures/ranking
   or propose: mutastic ignore <id> --propose --as=<equivalent|unreachable|…> --reason="Differs only when <case>. That can't happen because <why>." fixtures/ranking for each of md0d6db64, m2e329a72, m729a5380

then: run `mutastic next fixtures/ranking` again for the next one; when it says nothing is left, you're done
```

`next` exits 1 while something is queued and 0 when nothing missed is left, so a loop can run it
until it's done.

## For a tool

With `--format=jsonl`, `next` writes a `next-item` event for each item shown, its lead mutant as
`mutant-finished` has it, with `ids`, every mutant on its line, and a `rank` object listing what the
order went by; then a `next-finished` event with how many lines and mutants are queued, how many
were shown, and how many wait on a person. With `--top=3` on the ranking fixture, two items, as two
is the most from one file:

<!-- output: concepts/next-jsonl -->
```text
{"schema":"mutastic.events/v1","edition":"rust","reason":"next-item","id":"md0d6db64","fingerprint":"d0d6db64a432770e","status":"Survived","operator":"mutastic/boundary","location":{"file":"src/lib.rs","line":15,"column":7,"class":"ranking","method":"is_small"},"mutation":{"original":"<","replacement":"<=","description":"`<` becomes `<=`","applied":{"level":"source","text":"n < 10 -> (n) <= (10)"}},"coveredBy":["tests::five_doubled_is_more_than_one_and_small","tests::four_doubled_is_more_than_one_and_small","tests::six_doubled_is_more_than_one_and_small"],"inputs":{"n":[4,5,6]},"testsRun":3,"durationMs":1,"doneWhen":"mutastic verify md0d6db64 fixtures/ranking passes","reproduce":"mutastic run --only=md0d6db64 fixtures/ranking","ignore":"mutastic ignore md0d6db64 --reason=\"<why>\" fixtures/ranking","propose":"mutastic ignore md0d6db64 --propose --as=<equivalent|unreachable|…> --reason=\"Differs only when <case>. That can't happen because <why>.\" fixtures/ranking","suggestedTest":{"testClass":"tests","name":"is_small_with_n_10","code":"#[test]\nfn is_small_with_n_10() {\n    assert!(!is_small(10)); // today false · with this mutant true\n}","call":"is_small(10)","today":"false","withMutant":"true"},"suggestion":{"call":"is_small(10)","parameters":["n"],"arguments":[10],"receiver":[],"today":"false","withMutant":"true"},"rendered":"missed md0d6db64: no test checks `is_small()` at `n == 10`\n   at src/lib.rs:15\n   - n < 10\n   + n <= 10\n   covered by: tests::five_doubled_is_more_than_one_and_small, tests::four_doubled_is_more_than_one_and_small, tests::six_doubled_is_more_than_one_and_small; all still pass with the change\n   evidence: every test calls `is_small()` away from the boundary\n   evidence: inputs seen: 4, 5, 6 (none is 10)\n   fix: test `is_small()` with `n = 10` and assert the result\n   test to add in mod tests:\n     #[test]\n     fn is_small_with_n_10() {\n         assert!(!is_small(10)); // today false · with this mutant true\n     }\n     // check today's value is the one you intend: the test pins what the code does now\n   done when: mutastic verify md0d6db64 fixtures/ranking passes\n   reproduce: mutastic run --only=md0d6db64 fixtures/ranking\n   or propose: mutastic ignore md0d6db64 --propose --as=<equivalent|unreachable|…> --reason=\"Differs only when <case>. That can't happen because <why>.\" fixtures/ranking\n","ids":["md0d6db64","m2e329a72","m729a5380"],"rank":{"position":1,"order":"run-1","factors":[{"signal":"operator","value":"condition-or-removal","why":"a condition or a removal: it changes what the code decides or does"},{"signal":"coveringTests","value":3,"why":"3 tests run it, and none checks it"}]}}
{"schema":"mutastic.events/v1","edition":"rust","reason":"next-item","id":"mabf32cb7","fingerprint":"abf32cb76b7ad2de","status":"Survived","operator":"mutastic/boundary","location":{"file":"src/lib.rs","line":11,"column":7,"class":"ranking","method":"is_big"},"mutation":{"original":">=","replacement":">","description":"`>=` becomes `>`","applied":{"level":"source","text":"n >= 100 -> (n) > (100)"}},"coveredBy":["tests::five_isnt_big"],"inputs":{"n":[5]},"testsRun":1,"durationMs":1,"doneWhen":"mutastic verify mabf32cb7 fixtures/ranking passes","reproduce":"mutastic run --only=mabf32cb7 fixtures/ranking","ignore":"mutastic ignore mabf32cb7 --reason=\"<why>\" fixtures/ranking","propose":"mutastic ignore mabf32cb7 --propose --as=<equivalent|unreachable|…> --reason=\"Differs only when <case>. That can't happen because <why>.\" fixtures/ranking","suggestedTest":{"testClass":"tests","name":"is_big_with_n_100","code":"#[test]\nfn is_big_with_n_100() {\n    assert!(is_big(100)); // today true · with this mutant false\n}","call":"is_big(100)","today":"true","withMutant":"false"},"suggestion":{"call":"is_big(100)","parameters":["n"],"arguments":[100],"receiver":[],"today":"true","withMutant":"false"},"rendered":"missed mabf32cb7: no test checks `is_big()` at `n == 100`\n   at src/lib.rs:11\n   - n >= 100\n   + n > 100\n   covered by: tests::five_isnt_big; all still pass with the change\n   evidence: every test calls `is_big()` away from the boundary\n   evidence: inputs seen: 5 (none is 100)\n   fix: test `is_big()` with `n = 100` and assert the result\n   test to add in mod tests:\n     #[test]\n     fn is_big_with_n_100() {\n         assert!(is_big(100)); // today true · with this mutant false\n     }\n     // check today's value is the one you intend: the test pins what the code does now\n   done when: mutastic verify mabf32cb7 fixtures/ranking passes\n   reproduce: mutastic run --only=mabf32cb7 fixtures/ranking\n   or propose: mutastic ignore mabf32cb7 --propose --as=<equivalent|unreachable|…> --reason=\"Differs only when <case>. That can't happen because <why>.\" fixtures/ranking\n","ids":["mabf32cb7","m2ed852d7","m03e1ab09"],"rank":{"position":2,"order":"run-1","factors":[{"signal":"operator","value":"condition-or-removal","why":"a condition or a removal: it changes what the code decides or does"},{"signal":"coveringTests","value":1,"why":"1 test runs it, and none checks it"}]}}
{"schema":"mutastic.events/v1","edition":"rust","reason":"next-finished","queued":3,"mutants":7,"shown":2,"legacy":0,"waiting":0}
```

See [Events](../reference/events.md).

## Proposed for ignoring

A mutant [proposed for ignoring](../reference/configuration.md#ignoring-one-mutant) isn't queued:
it still counts as missed in a run, but the next step is a person's, to accept the proposal or
withdraw it, not another test. `next` says how many are waiting (`2 mutants proposed for ignoring,
waiting on a person to accept them`), `next-finished` counts them as `waiting`, and when they're
all that's left, `next` exits 0.

## A run like any other

The first `next`, and any after the code changes, runs every mutant, and keeps that run as `mutastic
run` does: its record, which the next run compares with, and `mutation.json`, from which `explain`
says how the last run went for a mutant. A `next` that runs only what was left uncaught keeps
neither, as a run of some mutants says nothing about the rest. What it left uncaught is kept in
`target/mutastic/next.tsv`.
