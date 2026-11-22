- **A value a `let` works out only for a log line is skipped as arid.** In `let took =
  start.elapsed().as_millis() - base; info!("took {took} ms");` the subtraction's mutants only
  change what the log says, so they're now `arid: only feeds a log line`, part of
  `mutastic:output`. Every read of the name has to be in an output macro (`println!`, `info!`,
  `tracing::warn!` and the rest): an argument, `{took}` in the format string, or a tracing field.
  A value read anywhere else stays counted: returned, compared, pushed, passed to a function,
  `format!`, `write!` or `assert!`, read in a closure, or shadowed by another binding of the same
  name. Inline arguments, `info!("{}", a - b)`, never had mutants. Code that logs timings gets
  fewer mutants, and its score can go up; `arid: [mutastic:output]` on a `mutate:` rule brings
  them back. The Java edition skips the same values, inline or through a local.
