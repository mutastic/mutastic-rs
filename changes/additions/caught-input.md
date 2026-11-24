- **A card names the input a property caught a mutant with.** When a proptest, quickcheck or
  bolero property catches a mutant, `verify`, `explain` and the card say which input it failed on,
  ``caught by props::reverses_twice with `xs = [0]` ``, and `mutant-finished` has it as
  `caughtInput`, as `mutation.json` has `mutastic.caughtInput`. In a run of every mutant it's the
  first case that failed, unshrunk; a run of mutants named by id runs the catching proptest once
  more with shrinking on, for the smallest. `run-started` says which property-testing frameworks
  the package uses in `propertyTests`, with the seed mutastic drew proptest's cases from and the
  frameworks it couldn't seed.
