- **A likely flaky test's catches don't count, and the next runs check it harder.** A test was named
  likely flaky (`! likely flaky:`) when a mutant it caught changed result with the code unchanged,
  or a catch of it didn't repeat, but its catches still counted in that run, so a run could pass
  its threshold on them. Now, in the run that names it, each mutant it reached runs again without
  it. `last-runs/flaky-watch.tsv` remembers it, and later runs confirm each catch it makes 3 more
  times (`! watched:`), until its catches have held for 3 full runs. A score can fall where a flaky
  test's catches were counted. See [Flaky
  tests](https://docs.mutastic.io/concepts/flaky-tests/#when-a-test-is-named-likely-flaky).
