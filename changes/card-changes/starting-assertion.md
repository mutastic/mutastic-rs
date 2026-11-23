- **Code no test reaches gets an assertion to start with.** Under `Not covered`, `try:` gives an
  assertion for a function's first test, `assert_eq!(Pricing::new().loyalty_credit(0), 0);`, and
  `--format=agent` lists up to ten as `start with:`, as the Java edition does. Each is found by
  calling the function as it is and with one of its mutants in, so its value is what the code gave
  and the test it starts catches that mutant. A function only the crate calls, or one a test
  can't call with whole numbers alone, gets none. `mutant-finished` carries the call in
  `suggestion`. See [Code no test reaches](docs/concepts/suggested-tests.md#code-no-test-reaches).
