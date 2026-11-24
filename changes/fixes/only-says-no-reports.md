- **A run with `--only` says it writes no report when one is asked for.** `--report` with `--only`
  was passed over in silence, so a person or an agent asking for `mutation.json` got none and no
  reason. A note on stderr now says why: a run of some mutants says nothing about the rest. The
  exit code is as it was.
