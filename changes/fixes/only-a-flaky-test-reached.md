- **A mutant only a likely flaky test reached says so, not that no test reaches it.** Once that
  test is left out, nothing reaches the mutant, and it's not covered: `verify`, the cards and the
  agent entry now say `only <test> reached it, and it's likely flaky: its catch didn't count`, or
  `…: it was left out` when the test hadn't caught it, and the same for several tests. A run put
  together with `--from-shards` says it too.
