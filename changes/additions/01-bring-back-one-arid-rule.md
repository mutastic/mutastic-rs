- **Bring back one arid rule: `arid:` on a `mutate:` rule.** `arid: [mutastic:output]` under a
  `mutate:` rule takes back only that rule's skips in the code it covers; without it, every arid
  skip, as before. An id mutastic doesn't have is refused, with the one meant when it's close.
  The thread settings' skips have ids too: `mutastic:thread-names`, `mutastic:stack-sizes` and
  `mutastic:keep-alive`.
