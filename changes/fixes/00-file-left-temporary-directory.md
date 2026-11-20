- **A file one mutant's tests leave in the temporary directory can't change the next mutant's
  result.** Tests shared the system's temporary directory, so a mutant that removes a
  `remove_file` left its file there, and a test of another mutant that removes the write could
  read that file and pass: missed in one run, caught in the next. Each test process now gets an
  empty temporary directory of its own, as `TMPDIR` (and `TMP` and `TEMP` on Windows), deleted
  when it ends. Nothing is left in yours either. The Java edition empties its worker's temporary
  directory between mutants the same way. See [How a run
  works](docs/concepts/how-a-run-works.md#each-mutant-against-the-tests-that-reach-it).
