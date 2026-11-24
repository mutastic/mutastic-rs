# Releasing mutastic for Rust

A release is a version tag, `v<version>`, matching the version in `Cargo.toml`. The release
workflow (`.github/workflows/release.yml`) builds the native `mutastic` for each target from it, as
a draft release to read and publish. Nothing goes to crates.io yet.

From 1.0.0 the Rust and Java editions release each **minor** version together, **Java published
first, then Rust tagged**: the native `mutastic X.Y.z` runs the newest Java edition `X.Y.*`, and a
Rust `X.Y.0` without its Java `X.Y.0` would find nothing to fetch. The release workflow checks:
before it builds anything, its `java` job looks in the Java releases' feed for an `X.Y.*` with its
CLI attached (`scripts/java-paired.sh`), and without one fails within a minute, saying to tag the
Rust edition after it's published. It runs the Java sample of that release, too. A **patch** may ship alone, in
either edition: a Rust `X.Y.1` keeps running the newest Java `X.Y.*`, and a Java `X.Y.1` reaches
everyone on Rust `X.Y.*` within a day, as `mutastic` looks up the Java releases at most once a
day.

## Each release

1. **Check the promises** ([Compatibility](docs/reference/compatibility.md)). The tests already fail
   on a moved mutant id (`tests/ids.rs`), on an event the schema doesn't describe (the schema is
   pinned in `crates/mutastic/tests/schema/`), and, from 1.0.0, on a command, option, variable,
   exit status, `mutastic.yaml` or rules file the last 1.0+ release took and this one doesn't
   (`tests/compat.rs`). `scripts/release-check.sh X.Y.Z` checks what a release needs beyond that,
   all at once, and lists what's wrong: the changelog's rules, the version, the contract's pins and
   the ids. It writes nothing tracked and publishes nothing, so run it as often as you like; CI
   runs it on the release pull request too, the part that needs no build in Commits and the ids in
   CI. What it can't judge is left to you:
   - **Scores.** Read the *Scoring changes* and *Score corrections* that
     `scripts/changelog.sh preview` prints, the entries waiting in `changes/`. A patch release
     (`X.Y.Z+1`) may have score corrections, which only drop mutants that could never run, such as
     code the build never compiles counted by mistake: read each, and hold the patch back if one
     could lower a score, add a mutant or fail a run that passed. It has no scoring changes:
     anything else that moves a score (an operator into or out of the default set, a filter that
     skips more or less, a change to timeouts, mutants newly found) waits for a minor release. A
     minor release's changelog says which scores move, and how. `release-check` fails a patch with
     a scoring change, and a release with no entries at all.
   - **Ids.** When `release-check` says `MUTASTIC_IDS_WRITE=1 cargo test --test ids` changed
     `tests/ids/ids.tsv`, only by lines added for an operator new since the last release, commit
     it, so its ids are held from this release on.
   - **The Java edition.** From 1.0.0, for a minor release (`X.Y.0`), the Java edition's `X.Y.0`
     is ready to publish after this one's release commit (the Java repository's `RELEASING.md`).
     A patch needs nothing from it.
   - **The contract's pins.** `scripts/pin-contract.sh prune` deletes all but the newest
     ([contract/README.md](contract/README.md)); `release-check` fails until it's run.
2. **The release commit,** `chore(release): version X.Y.Z`: the version in `Cargo.toml` (and
   `Cargo.lock`), the changelog's section for it, and the docs' kept outputs that print the
   version, written again with `MUTASTIC_DOCS_WRITE=1 cargo test --test docs`. Each entry since the
   last release is a file in `changes/` ([changes/README.md](changes/README.md)), so pull requests
   don't conflict over `CHANGELOG.md`. Read them as one section, then gather them into
   `CHANGELOG.md` as `## X.Y.Z (YYYY-MM-DD)`, dated today, which deletes the files:
   ```sh
   scripts/changelog.sh preview X.Y.Z   # prints the section, writes nothing
   scripts/changelog.sh release X.Y.Z   # writes it below Unreleased, deletes the entries
   ```
   Edit the section if it needs it, a paragraph above a section's entries or one at the top. Only
   a commit with the `release` scope may edit `CHANGELOG.md`. From 1.0.0, keep what this release
   takes, so the next is held to it:
   ```sh
   MUTASTIC_RELEASED_WRITE=1 cargo test --test compat   # writes crates/mutastic/tests/released/X.Y.Z
   ```
   Before 1.0.0 skip that: anything may change, and there's nothing to hold the next to. Then
   check the commit: `scripts/release-check.sh X.Y.Z` says it's ready, or lists what isn't.
3. **Move the docs site's pin** (`docs/rust-edition.json` in the Java repository) to the release
   commit, with `main` green: in the Java repository, `scripts/docs-pin-rust.sh <release commit>`
   sets it and stamps the shared pages whose Rust sources changed (its `RELEASING.md` says what to
   read before committing). The docs a Java tag deploys keep the pin as it is at that tag, so for
   `X.Y.0` the pin moves before the Java edition's `vX.Y.0` is tagged, and names this commit, as
   its tag doesn't exist yet. For a Rust patch alone, the pin reaches docs.mutastic.io with the
   next Java release.
4. For `X.Y.0`, wait for the Java edition's `vX.Y.0` to be published with its CLI attached. Then,
   with `main` green, tag the release commit and push the tag:
   ```sh
   git tag vX.Y.Z <release commit> && git push origin vX.Y.Z
   ```
   Tagged too early, the workflow's `java` job fails within a minute and builds nothing: once the
   Java release is published, re-run the workflow.
5. **Read the draft release the workflow makes** (each target's archive, `SHA256SUMS` and Scoop's
   `mutastic.json`), then publish it. Its notes are the release's section of `CHANGELOG.md`
   (`scripts/changelog.sh notes X.Y.Z`): a tag whose commit has no section fails within a minute,
   before anything is built.

## Withdrawing a release

A release can't be taken back from anyone who has it: an archive downloaded stays downloaded. What
can be stopped is anyone else getting it, and the fix is the next patch, released as quickly as a
release can be. Do both: withdraw it, then release `X.Y.Z+1` with the fault fixed or reverted.

### A bad Rust release

1. **Make the GitHub release a draft again** (edit it, *Set as a draft*), so nothing finds it as
   the latest: `install.sh`, `install.ps1` and Scoop's `releases/latest/download/mutastic.json` all
   go back to the release before it. Keep the tag: deleting it breaks whatever pinned it with
   `MUTASTIC_NATIVE_VERSION`, and its archives are the evidence of what went out.
2. **Move the docs site's pin back** (`docs/rust-edition.json` in the Java repository) to the
   release before it, and rebuild the docs (`gh workflow run docs.yml --ref main -f release=true`
   there).
3. **cargo-binstall** builds its download address from the version in `Cargo.toml` on `main`, so
   until the next patch's release commit moves it, `cargo binstall --git` finds no archive for a
   draft and falls back to building from source. Release the patch.
4. Say so in the changelog's entry for the patch: which release was withdrawn, and why.

### A bad Java patch

Every native `mutastic X.Y.*` runs the newest Java `X.Y.*` it can find, and looks the releases up
at most once a day (`java.rs`, from GitHub's releases feed, which lists only published releases
and whose pre-release tags, `v1.2.3-rc1`, aren't plain versions and are skipped). So a Java patch
reaches everyone on that minor version within a day, and a bad one does too.

1. **Make the Java GitHub release a draft again,** so it leaves the feed: a native that hasn't
   fetched it yet won't. Maven Central can't withdraw a release, so the Gradle and Maven plugins
   keep finding it where a build names that version; the next patch supersedes it for anyone who
   follows the latest.
2. **Release the next Java patch** with the fault fixed or reverted. A native that already fetched
   the bad one keeps it in `~/.mutastic/mutastic-X.Y.Z` and runs it, as the newest kept, until a
   newer patch arrives: the next patch is what reaches them, within a day.
3. **Until it's out,** say in the release notes and the issue how to step back: set
   `MUTASTIC_VERSION` to the patch before, or delete `~/.mutastic/mutastic-X.Y.Z`.

Never tag a pre-release with a plain version (`v1.2.4` marked *pre-release*): the feed lists it,
and every native would fetch it as the newest patch. Give it a suffix, `v1.2.4-rc1`.
