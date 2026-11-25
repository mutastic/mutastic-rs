- **Arguments for Cargo.** What follows `--` goes to Cargo on each build call, and never to the
  tests: `mutastic run -- --features postgres`. The features it turns on choose the code mutated
  too, and each command a run offers keeps it. `MUTASTIC_BUILD_ARGS`, or `build:`'s `args:` in
  `mutastic.yaml`, gives the same to every run, and no command repeats those, so a secret goes
  there. An argument that fights how mutastic drives Cargo is refused, with the reason.
