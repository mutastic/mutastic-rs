- **A time that isn't a sleep's is counted.** A number in `Duration::from_secs(30)` had no mutant
  wherever it was; now only one that goes into a sleep is skipped, so a timeout, a deadline or a
  backoff is mutated, as the Java edition counts them. A runtime's keep-alive that the change
  makes zero is counted too, as the keep-alive rule says. Scores on code with timeouts can go
  down.
