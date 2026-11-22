- **A test whose code exits is named, and an exit isn't a crash.** A mutant that made code a test
  ran call `process::exit(2)` read as caught only because the test process crashed (`crash: an
  abnormal exit`), with no test named. It's now caught by the test that was running, by the exit:
  the card says `caught by …, by the process exiting with status 2`, and the event has `caughtWith:
  "exit: 2"`. Status 0, which `cargo test` reads as a pass, is no catch: the tests that never ran
  after it now run without it, and decide. The Java edition does the same for `System.exit`. See
  [When the code a test runs exits](docs/concepts/results.md#when-the-code-a-test-runs-exits).
