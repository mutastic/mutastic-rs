- **`MUTASTIC_ARID=audit`: see whether the arid rules hide anything.** Set it, and a run runs the
  mutants mutastic's arid rules skip as well, out of the score and the counts, and the summary has
  an `Arid audit` row: how many ran, how many the tests caught, how many of those by a test's own
  assertion, and where. A catch by an assertion means a test checks what the arid code does, so the
  row gives the `mutastic.yaml` rule that brings that rule's mutants back there. `run-finished`
  has it as `aridAudit`. The Java edition has it too.
