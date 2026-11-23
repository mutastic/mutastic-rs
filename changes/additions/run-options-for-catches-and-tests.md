- **Four of the Java edition's options for a run: `--confirm-catches`, `--kill-matrix`,
  `--exclude-tests` and `--each-mutant`.** `--confirm-catches` runs each catching test again on its
  own against the mutant: a catch that doesn't repeat doesn't count, the other covering tests
  decide, and the test is named as likely flaky (`unconfirmedCatchBy` in the events, and
  `catch-not-repeated` in `run-finished`'s `likelyFlaky`). `--kill-matrix` runs every test that
  reaches a caught mutant and names each that catches it, as `caughtByAll` in the events and
  `killedBy` in `mutation.json`. `--exclude-tests=TEST` leaves out a test, by its name as a run
  shows it, or a module of tests by its path: the summary's `Left out` row names it, what only it
  reaches is not covered, and a flaky test's line now offers it. `--each-mutant` gives every missed
  mutant a card of its own, and lists code no test reaches by line. `verify`, `explain`, `next` and
  `baseline` take `--confirm-catches` and `--exclude-tests` too. See
  [How a run works](docs/concepts/how-a-run-works.md) and
  [Troubleshooting](docs/troubleshooting.md#a-test-is-flaky).
