- **Patches may correct a score upward, and never lower one.** A patch release used to promise
  never to move a score. It now promises never to lower a score, never to add a mutant and never
  to turn a passing run into a failing one: the one score change it may make is a correction that
  drops mutants that could never run, such as code the build never compiles counted by mistake, so
  a score can only rise. Its changelog lists each under a new section, *Score corrections*.
  Anything else that moves a score still waits for a minor release's *Scoring changes*. See
  [Compatibility](docs/reference/compatibility.md#scores).
