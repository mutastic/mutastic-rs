- **Your own output calls, as a `mutastic.yaml` rule.** `- output: crate::audit::record` with a
  `reason:` names a function or a macro of your own that only writes output, an audit log or a
  metric (`metrics::counter`), and a statement that calls it is skipped as `mutastic:output`:
  `arid: only calls audit::record (<reason>)`. The path in the code may be shorter after a `use`.
  A `mutate:` rule brings them back, and none are output until a rule says so. As in the Java
  edition.
