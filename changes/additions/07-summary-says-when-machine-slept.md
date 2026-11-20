- **The summary says when the machine slept during the run.** Duration counts the time the
  machine was awake, so a run a Mac slept through for four minutes said 15.4s. When the clock ran
  at least 5 seconds and a tenth of its time more, a line under Duration says `The machine slept
  for 3m49s during the run, which Duration leaves out.`, and `run-finished` has it as `sleptMs`,
  else 0. The Java edition says it too. See [When the machine
  sleeps](docs/concepts/output.md#when-the-machine-sleeps).
