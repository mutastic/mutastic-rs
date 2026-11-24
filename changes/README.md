# Changes for the next release

When every pull request added its entry to the end of [CHANGELOG.md](../CHANGELOG.md), any two
that merged close together conflicted, and the merge queue dropped one. So each entry is a file of
its own here until a release gathers them into CHANGELOG.md.

## Adding an entry

A change someone using mutastic would notice gets an entry: a file
`changes/<section>/<slug>.md`, the slug a few words of your own in lower case with hyphens, such as
`changes/fixes/wrapper-path-with-space.md`. The file is the entry as CHANGELOG.md will show it: a
bullet with its lead sentence in bold, then the paragraph, wrapped at 100 columns, each line after
the first indented two spaces.

```markdown
- **A wrapper in a directory with a space gets commands that run as they're copied.** A wrapper
  under `/Users/me/My Projects` started a command a shell splits at the space. Its path is now
  quoted, `'/Users/me/My Projects/gradlew'`.
```

Link to a page here as CHANGELOG.md would, from the top of the repository: `docs/concepts/output.md`,
not `../../docs/concepts/output.md`.

The sections, in the order a release lists them:

| Directory          | Heading          | For                                                          |
| ------------------ | ---------------- | ------------------------------------------------------------ |
| `breaking-changes` | Breaking changes | something that worked and now doesn't, for a major release   |
| `card-changes`     | Card changes     | what a card says                                             |
| `scoring-changes`  | Scoring changes  | what moves a score: an operator into or out of the defaults, a filter that skips more or less; a minor release only |
| `score-corrections` | Score corrections | a correction that drops mutants that could never run, so a score can only rise; a patch may have it |
| `security`         | Security         | a fix that keeps a project or a machine safe                 |
| `additions`        | Additions        | something new                                                |
| `changes`          | Changes          | something that works differently                             |
| `fixes`            | Fixes            | something that was wrong                                     |
| `internal`         | Internal         | what only someone working on mutastic sees                   |

In a section, entries come in the order they reached `main`. A section's `_intro.md`, if it has
one, is a paragraph above its entries.

## Checking and releasing

```sh
scripts/changelog.sh check                     # each file is in a section and shaped as an entry
scripts/changelog.sh preview                   # the next release's section, as it would read
scripts/changelog.sh release X.Y.Z             # writes it into CHANGELOG.md, deletes the files
```

CI runs `check` on every pull request, and fails one that edits CHANGELOG.md unless it's a
release, its subject `chore(release): version X.Y.Z`. [RELEASING.md](../RELEASING.md) has the
release's steps.
