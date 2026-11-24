- **A release is checked before it's tagged, and its draft has notes.** 1.2.0, 1.2.1 and 1.3.0
  were drafted with empty notes; the draft's are now the release's section of `CHANGELOG.md`
  (`scripts/changelog.sh notes X.Y.Z`), and a tag without one fails within a minute.
  `scripts/release-check.sh X.Y.Z` checks what a release needs at once and lists what's wrong: no
  scoring change in a patch and no release without entries (`scripts/changelog.sh check --for
  X.Y.Z`), the version in `Cargo.toml`, only the newest contract pin, and ids all kept. CI runs it
  on a release pull request.
