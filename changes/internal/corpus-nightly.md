- **mutastic runs on a corpus of real crates every night.** The fixtures hold the shapes we thought
  of, and the bugs that mattered turned up in real crates instead: a crate its own dev-dependencies
  pulled in again, modules declared inside a macro, a mutant no limit stopped. `corpus/corpus.yaml`
  pins 24 packages, each to a commit, with what a healthy run does on it: a score, or the refusal
  that's right today. `.github/workflows/corpus.yml` runs a share of each one's mutants nightly and
  on demand before a release (`scripts/corpus-run.sh`, through `just corpus`), checks the agent
  format's head on each score, and tables the results on the run's page against the last good
  night's: the run is red when a crate that passed then fails now. CONTRIBUTING.md says how to add
  a crate, and that each corpus failure becomes a fixture in the pull request that fixes it.
