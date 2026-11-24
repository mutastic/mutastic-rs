- **A mutant only a likely flaky test reached says so, not that no test reaches it.** Once that
  test is left out, nothing reaches the mutant, and it's not covered; its card, `verify`'s verdict
  and the agent entry now say `only <test> reached it, and it's likely flaky: its catch didn't
  count`.
