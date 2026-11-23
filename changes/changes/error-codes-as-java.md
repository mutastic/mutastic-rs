- **Five failures have the Java edition's codes, and two of them now stop the run.** A package
  whose tests are all `#[ignore]`d, and one whose tests pass and reach none of the code with
  mutants, used to finish with every mutant not covered and exit 1: a 0% that measures nothing.
  Each now stops the run with exit 2 and says why, as `no-tests` and `nothing-covered`; a package
  with no tests at all still finishes, and says `Found no tests`. A test that ends its process
  before any mutant, by `process::exit` or a crash, is `tests-stopped` and named with how it ended,
  `(it exited with status 3)`; one that runs out of time is `tests-timed-out`; both were
  `red-baseline`. A file mutastic can't read or write is `disk`, not `other`, and its message ends
  in what to do about a full disk, a directory that isn't writable or a file where a directory
  goes. See [Errors](docs/reference/errors.md).
