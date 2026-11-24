- **CI runs the build through `just`.** Each job calls a recipe, `just lint`, `just test`, `just
  clippy`, `just msrv`, `just fuzz`, `just miri`, `just asan`, `just contract`, and Self's `just
  self-build`, `just self-shard`, `just self-merge` and `just self-terminal`, in place of cargo
  commands written into the workflows, so CI and a contributor type the same words and a job that
  fails can be run as CI ran it. Commits checks the justfile's format and its list of verbs, so
  none goes unseen.
