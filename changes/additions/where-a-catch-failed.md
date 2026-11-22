- **Events say where a catch failed, and which tests catch everything alike.** `mutant-finished`
  and mutation.json have `failureOrigin`, as in the Java edition: `assertion` when an `assert!`
  failed, `test` when the test's own code panicked, `code` when the code under test did. A test
  that caught mutants in three or more functions, each time with a panic in its own code, gets a
  note under the summary and in the agent format, and `run-finished`'s `unrelatedCatches` names
  it, where the list was always empty: it may be flaky, or call a lot of code.
