- **`mutastic mutants` says what a set of operators would add to a run's time, before it's run.**
  With `--operators`, the line before the count estimates it from the last whole run: `estimate: 24
  more mutants than defaults, up to 64ms more of tests, from the run of 23 Nov 2026 04:58 UTC`, the
  time of the tests that reach each new mutant's code. It says why when there's no estimate: no run
  recorded, the code changed since, a run under other operators than the project's, or new mutants
  in code that run didn't measure. `--estimate` runs every test once, with no mutant in, and
  estimates what `defaults`, `stronger` and `all` would each take, keeping what it measured for the
  next `mutants --operators`. A whole run and `--estimate` keep the times in
  `target/mutastic/test-times.tsv`. See [Coverage and mutants](docs/coverage-and-mutants.md).
