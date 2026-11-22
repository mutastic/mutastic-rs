- **Annotations and a job summary on GitHub Actions.** Where `GITHUB_ACTIONS=true`, `mutastic run`
  now annotates the line of each of its first ten misses, ranked as the agent format ranks them,
  so a pull request's changed files show them, and adds a Markdown summary to the job's page
  (`$GITHUB_STEP_SUMMARY`): the verdict, the score, the counts and the first misses with where they
  are. It needs no token or permission, and the annotations go to stderr, so a format's own output
  is untouched. `MUTASTIC_GITHUB=off` turns both off. The Java edition does the same. See [On
  GitHub Actions](docs/reference/reports.md#on-github-actions).
