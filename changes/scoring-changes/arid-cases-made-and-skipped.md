- **More of the arid cases are mutants, made and skipped, as in the Java edition.** A number
  given to `with_capacity`, `with_capacity_and_hasher` or `reserve` had no mutant, and arithmetic
  there, `Vec::with_capacity(n * 2)`, was counted; both are now skipped as `arid: only sizes a
  collection's first allocation`. A sleep removed, `thread::sleep(d);` or `sleep(d).await;`, was
  counted; it's `arid: only waits`, and the number or the sum that only sets how long it waits is
  `arid: only sets how long to wait`. An `Err`'s, a `context`'s or an `expect`'s text, and every
  mutant in a `Debug` or `Hash` impl, are made and skipped too, with the Java edition's ids. Each
  is counted under `skipped`, audited by `MUTASTIC_ARID=audit` and brought back by a `mutate:`
  rule. Scores on code that sizes collections or sleeps can go up.
