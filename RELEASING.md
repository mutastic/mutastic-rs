# Releasing mutastic for Rust

A release is a version tag, `v<version>`, matching the version in `Cargo.toml`. The release
workflow (`.github/workflows/release.yml`) builds the native `mutastic` for each target from it, as
a draft release to read and publish. Nothing goes to crates.io yet.

From 1.0.0 the Rust and Java editions release each **minor** version together: `X.Y.0` of both, at
the same time, as the native `mutastic X.Y.z` runs the newest Java edition `X.Y.*`, and a Rust
`X.Y.0` without its Java `X.Y.0` would find nothing to fetch. A **patch** may ship alone, in
either edition: a Rust `X.Y.1` keeps running the newest Java `X.Y.*`, and a Java `X.Y.1` reaches
everyone on Rust `X.Y.*` within a day, as `mutastic` looks up the Java releases at most once a
day.

## Each release

1. **Check the promises** ([Compatibility](docs/reference/compatibility.md)). The tests already fail
   on a moved mutant id (`tests/ids.rs`), on an event the schema doesn't describe (the schema is
   pinned in `crates/mutastic/tests/schema/`), and, from 1.0.0, on a command, option, variable,
   exit status, `mutastic.yaml` or rules file the last 1.0+ release took and this one doesn't
   (`tests/compat.rs`). These are left to you:
   - **Scores.** Read the changelog's *Scoring changes*. A patch release (`X.Y.Z+1`) has none:
     anything that moves a score (an operator into or out of the default set, a filter that skips
     more or less) waits for a minor release. A minor release's changelog says which scores move,
     and how.
   - **Ids.** Run `MUTASTIC_IDS_WRITE=1 cargo test --test ids`. If `tests/ids/ids.tsv` changes, only
     by lines added for an operator new since the last release, commit it, so its ids are held
     from this release on.
   - **The Java edition.** From 1.0.0, for a minor release (`X.Y.0`), the Java edition's `X.Y.0`
     is ready to publish beside this one (the Java repository's `RELEASING.md`). A patch needs
     nothing from it.
2. **The release commit,** `chore(release): version X.Y.Z`: the version in `Cargo.toml` (and
   `Cargo.lock`), the changelog's *Unreleased* moved under the version and date, and the docs' kept
   outputs that print the version, written again with `MUTASTIC_DOCS_WRITE=1 cargo test --test
   docs`. From 1.0.0, keep what this release takes, so the next is held to it:
   ```sh
   MUTASTIC_RELEASED_WRITE=1 cargo test --test compat   # writes crates/mutastic/tests/released/X.Y.Z
   ```
   Before 1.0.0 skip that: anything may change, and there's nothing to hold the next to.
3. With `main` green, tag it and push the tag:
   ```sh
   git tag vX.Y.Z && git push origin vX.Y.Z
   ```
4. **Move the docs site's pin** (`docs/rust-edition.json` in the Java repository) to the new tag,
   while the release is still a draft: the docs a Java tag deploys keep the pin as it is at that
   tag, so for `X.Y.0` the pin moves before the Java edition's `vX.Y.0` is tagged (its
   `RELEASING.md` says how, with the shared pages' stamps). For a Rust patch alone, the pin
   reaches docs.mutastic.io with the next Java release.
5. **Read the draft release the workflow makes** (each target's archive, `SHA256SUMS` and Scoop's
   `mutastic.json`), then publish it. For `X.Y.0`, publish it only once the Java edition's
   `vX.Y.0` GitHub release has its CLI attached: until then the native `mutastic X.Y.0` finds no
   Java edition to run in a Maven or Gradle project.

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
