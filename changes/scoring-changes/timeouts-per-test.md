- **A mutant is timed out by its tests' own time, side by side and under a ceiling, so a loop that
  never ends is always stopped.** Its tests could run for five times their coverage-pass times
  added up, and the coverage pass records every call's whole numbers: a test of 10.8s took minutes
  there, and a base64 mutant reached by 86 tests ran for over 40 minutes before its run was killed.
  Each test now has five times its own time and a second, timed again without the recording when
  that made 100,000 calls; a binary's tests share their limits out as they run side by side, never
  under ten seconds; and no mutant's tests run past ten minutes, or twenty times the suite's own
  time, which `timeout-ceiling:` sets. Twenty one-second tests and a fast one that a mutant makes
  loop are stopped after 21s on 8 cores, not 1m42s. A slow mutant whose tests would have finished
  can now be timed out, which counts as caught: no fixture's mutant moved, but check a threshold on
  a suite with tests that make millions of calls. The coverage pass skips calls a thread has
  recorded, 7 to 10 times faster on such a test, and the time left counts what a slow mutant has
  left of its limit
  ([docs/troubleshooting.md](docs/troubleshooting.md#how-long-a-mutants-tests-may-run)).
