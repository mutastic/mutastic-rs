- **Twins run once, and their place counts once.** Two mutants at one place that always do the
  same now run as one: the other is skipped as `twin: same as <id>`, naming the one that runs, and
  is out of the score. Two operators making the same code are twins, as a function that ends in
  `n > 0` made `true` by `mutastic/return-value` and by `mutastic/remove-conditional` is; so are
  the changes to a comparison of an unsigned value with 0 that agree for 0 and above, `n > 0` made
  `n >= 0` and made `true`. No id changes, and each still works: `verify`, `run --only` and
  `explain` run the one it's the same as, `mutastic ignore` records the decision for that one, and
  an ignore of either holds for both. Runs have fewer mutants to build and run, and a score moves
  a little where a place was counted twice. Events give a twin's `sameAs` and the skip kind
  `twin`. The Java edition has twins too. See [Twins](docs/concepts/results.md#twins).
