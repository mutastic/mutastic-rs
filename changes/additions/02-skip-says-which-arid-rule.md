- **A skip says which arid rule made it, and how to undo it.** `explain` on a mutant an arid rule
  skipped has an `arid` row: the rule's id, what it skips, a link to its docs and the
  `mutastic.yaml` rule to copy that runs it. The HTML report says the same. Events carry the id as
  `skipRule` (`mutant-finished`) and the row as `arid` (`mutant-explained`), and mutation.json as
  `mutastic.skipRule`, as in the Java edition.
