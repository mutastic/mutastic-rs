- **A call whose result is only checked is removed: `handle.join().unwrap();` is a mutant.** A
  statement ending in `.unwrap()` or `.expect(..)` had no mutant and wasn't counted, so a test that
  didn't wait for a thread, or never saw what `tx.send(v).unwrap();` sent, was never asked to. Now
  the statement is removed, by `mutastic/remove-discarded-call`, as the Java edition removes
  `thread.join();`. One that checks a value it's given, `x.unwrap();`, is counted and skipped as
  arid, `arid: only checks the value is there`, as the Java edition skips a removed
  `Objects.requireNonNull(x);`. New ids appear; none moves. See [Remove discarded
  call](docs/operators/calls.md#remove-discarded-call).
