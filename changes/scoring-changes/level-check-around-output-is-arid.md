- **A log level's check around only output is skipped as arid.** `if log_enabled!(Level::Debug)
  { debug!(…); }`, and `tracing`'s `enabled!`, with no `else`: changing the check only decides
  whether the line is written, so its mutants are `arid: only decides whether output is written`.
  A body with anything but output macros in it keeps them counted. Runs on such code get fewer
  mutants, and scores can go up; `arid: [mutastic:output]` brings them back.
