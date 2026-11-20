- **Code only `main` runs says so, and what to do.** A binary's `main` no test reaches read as not
  covered, like any other function. Its line in the Not covered list now says it's reached only
  from `main()`, and to move what it does into `run(args, out)`, which returns the exit code, test
  `run`, and leave `main()` a one-line call to it; a `main` whose mutants are all on one line gets
  no such advice. The Java edition says the same. See [Code reached only from
  main](docs/concepts/results.md#code-reached-only-from-main).
