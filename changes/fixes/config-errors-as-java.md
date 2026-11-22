- **A mistake in `mutastic.yaml` or `.mutastic/ignores.yaml` is told as the Java edition tells
  it.** A section mutastic doesn't have says `colour: isn't a section`, with the one meant when
  it's close, where it listed every section first; `operators: stronger` on one line says the list
  goes a line under it, and shows it; `- skp:` says there's a rule called `skip:`. In the ignores,
  a line with no colon gives an example, `reason: only logs`, an entry with a key close to the
  missing `reason:` says so, and `ignore --accept` or `--remove` of an id one slip from a recorded
  one names it, with the command. `MUTASTIC_INCREMENTAL` is in the help, and the configuration
  page says how an editor checks `mutastic.yaml` and rules files against their JSON Schemas.
