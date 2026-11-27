- **The run records, baseline, flaky watch and `next`'s queue are in the editions' core.** The
  file formats and their logic move to `mutastic-core`: a mutant's fate (`Status`), the run
  record and its comparison with the last run, the records kept for each set of operators, the
  tests watched since a run named them likely flaky, `.mutastic/baseline.yaml`, and what the last
  `mutastic next` left uncaught. What each makes of a Rust run, from its `engine::Report`, its
  package and its code under test, stays in `mutastic`. Nothing mutastic prints or keeps changes.
