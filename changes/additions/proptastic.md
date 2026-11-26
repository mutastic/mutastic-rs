- **proptastic's properties are seeded, and a card names the input that caught.** A package whose
  tests use proptastic, found in its `Cargo.lock` whether it depends on proptastic or on a crate
  that wraps it, has every test process given `PROPTASTIC_SEED`, the run's seed as proptastic reads
  one (`0x0`) unless the person set theirs, with proptastic's saved failures, events, corpus and
  colour off, so a run writes none of their files. A mutant's run stops at the first
  counterexample (`PROPTASTIC_SHRINK=false`); `run --only`, `verify` and `explain` shrink it once,
  and the card says ``caught by lists::sorts_as_std_does with `xs = [0]` ``, read from
  proptastic's report, its stacked form too (`docs/concepts/results.md`).
