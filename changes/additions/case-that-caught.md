- **A case that catches a mutant is named by its row.** `verify` and `explain` say `caught by
  rstest_tests::discounts::case_2_at_the_edge (case #[case::at_the_edge(100, 10)])`, the rstest
  `#[case]` or test-case `#[test_case]` row as written, and the `mutant-finished` event has
  `caughtCase`, with the `fn` the row is written in, its place in this run and the row. A case that
  fails with no mutant in is named with its row too. The number in a case's name moves when a row
  is added above it; the row says which one it is (`docs/concepts/results.md`).
