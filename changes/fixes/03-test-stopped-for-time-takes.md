- **A test stopped for time takes the processes it started with it.** A mutant that made a test
  run past its time stopped the test process, and left whatever it had started running: a build,
  a server, a `sleep`. They went on taking memory and cores from the next mutants' tests, and on
  a small machine could take it down. Those processes are now stopped too, as the Java edition
  does.
