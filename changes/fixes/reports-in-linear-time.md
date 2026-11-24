- **Writing the reports takes as long as the tests are many, not their square, and the run's
  Duration counts it.** Each test that covers a mutant is placed in its file for
  `mutation.json` and the HTML report, and each was found by reading and parsing its file again:
  with typenum's 1,800 tests, the reports took 74 of the run's 80 seconds, while the Duration row
  said 6.1s. Each file is now read once per report, and the Duration row has a `reports` phase, in
  its total.
