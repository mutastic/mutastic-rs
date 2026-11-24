- **`gate check` and `next` confirm a watched test's catches, as `run` does.** A test an earlier run
  named likely flaky has each catch it makes run 3 more times, and a catch that doesn't repeat
  doesn't count, so the gate no longer passes a task on a lucky catch, and `next` queues a mutant
  only such a catch hid. `explain` and `baseline` confirm the same way. None of them writes the
  watch.
