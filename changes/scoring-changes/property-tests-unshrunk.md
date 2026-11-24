- **A property test runs unshrunk on a mutant, and the run says which frameworks it can't seed.**
  A mutant's run now sets `PROPTEST_MAX_SHRINK_ITERS=0`: proptest's first counterexample catches the
  mutant, and shrinking it, often the costly part of a failing property, no longer runs. A mutant
  caught the same way has the same status, but one whose property ran out of time or crashed
  while shrinking is now caught by the property's failure, so a timeout or a crash can become a
  plain catch, and a crash where a crash proves nothing, out of the score, can come back into it.
  proptest's seed was already pinned, but only proptest 1.7.0 and later read `PROPTEST_RNG_SEED`:
  an older one, as winnow locks, drew afresh in every run. quickcheck, arbtest and bolero seed
  themselves from the system with no variable to pin it. A run of a package that uses any of these
  now ends with a note on stderr, `note: property tests in proptest 1.6.0 run with their own seeds,
  which mutastic can't set: …`, as a mutant only some of their inputs catch can flip from run to
  run, and a property that fails before any mutant no longer offers a `PROPTEST_RNG_SEED=0`
  command that can't repeat its draw. See [Tables of cases and property
  tests](docs/troubleshooting.md#tables-of-cases-and-property-tests).
