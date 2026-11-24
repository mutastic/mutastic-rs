- **A release is checked once it's out, and its candidate before it's tagged.**
  `release-verify.yml` installs the published release with mutastic.io's `install.sh` and
  `install.ps1` and with Scoop, on Linux, macOS and Windows, then runs it on a crate and, through
  the Java edition it fetches, on a Gradle project; `candidate-run.yml` runs the native `mutastic`
  built from the tree over a share of every fixture, and fails on a crash, a run that can't run, or
  tests that fail without any mutant.
