- **The ceiling test times the mutant, not the run.** `a_mutant_stopped_at_the_ceiling_says_so`
  held the whole run, the crate's build included, to ten seconds, and failed twice on a slow
  Windows runner at 10.78s and 11.6s with the ceiling working. It now holds the mutant's own
  `durationMs` between the 2s ceiling and the 10s floor.
