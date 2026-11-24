- **A release is published with one approval.** After the tag, the release workflow drafts the
  release and writes a summary for its approver, from the draft as it is on GitHub: the version,
  each asset with its digest and its attestation checked, the notes and the Java edition it pairs
  with. Its `publish` job waits in the `release` environment; once approved, it checks again that
  the Java edition `X.Y.*` is published with its CLI, publishes the draft as the latest release and
  starts `release-verify.yml`. It refuses while the environment has no required reviewer, so
  nothing is published unapproved. Publishing a draft by hand is no longer a step.
