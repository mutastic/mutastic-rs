- **`--at` runs, or lists, the mutants on a line.** Checking one line took listing the mutants,
  finding their ids and passing them to `--only`, though a person fixing a line, an agent on one
  function or an editor has a file and a line, not ids. `mutastic run --at=<file>:<line>`, or
  `<file>:<from>-<to>`, runs the mutants whose cards point at those lines, under the run's
  operators, and says each one's fate after a line that counts them:
  `lines 26–29 of lib.rs: 6 mutants, 5 caught, 1 missed`. `mutastic mutants --at` lists them.
  Lines with none are an error naming the nearest that have one, never a run of none. At a
  workspace's root the file picks its member. As in the Java edition. See `docs/scoping.md`.
