- **A count of one reads as one.** The gate's pass said the tests `catch 1 of the 1 mutant`; it
  now says they `catch the only mutant`, `catch all 6 mutants` when they catch every one, and
  `catch 4 of the 6 mutants` otherwise. `1 of the baseline's mutants is caught now`, from a run
  and from `baseline --prune`, and `removed 1 ignore that no longer holds`, from `ignore
  --prune`, take their verb from the count too, as the Java edition's do.
