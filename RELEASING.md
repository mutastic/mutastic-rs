# Releasing mutastic for Rust

A release is a version tag, `v<version>`, matching the version in `Cargo.toml`. The release
workflow (`.github/workflows/release.yml`) builds the native `mutastic` for each target from it,
drafts the release with a summary for its approver, and publishes it once they approve. Nothing
goes to crates.io yet.

From 1.0.0 the Rust and Java editions release each **minor** version together, **Java published
first, then Rust tagged**: the native `mutastic X.Y.z` runs the newest Java edition `X.Y.*`, and a
Rust `X.Y.0` without its Java `X.Y.0` would find nothing to fetch. The release workflow checks:
before it builds anything, its `java` job looks in the Java releases' feed for an `X.Y.*` with its
CLI attached (`scripts/java-paired.sh`), and without one fails within a minute, saying to tag the
Rust edition after it's published. It runs the Java sample of that release, too. A **patch** may ship alone, in
either edition: a Rust `X.Y.1` keeps running the newest Java `X.Y.*`, and a Java `X.Y.1` reaches
everyone on Rust `X.Y.*` within a day, as `mutastic` looks up the Java releases at most once a
day.

## The release train

From 1.4.0, a minor release leaves every second Tuesday, both editions together for `X.Y.0`, the
Java edition first, then this one, as above; the Java edition's
[RELEASING.md](https://github.com/mutastic/mutastic/blob/main/RELEASING.md#the-release-train) has
this same section. Tuesday is a proposal: the day is Brian's to change. A change that misses a
train waits for the next one.

- **The cut-off.** A train takes the `changes/` entries merged by the day before it.
- **The gates.** It leaves only when this repository's nightly corpus (`corpus.yml`) is green, the
  Java edition's corpus to come; `scripts/release-check.sh X.Y.0` passes in both editions
  ([Each release](#each-release)); and the candidate run passes (`candidate-run.yml`,
  [Before tagging](#before-tagging)).
- **A release candidate,** `vX.Y.0-rc1`, is a GitHub pre-release only, never a Central release:
  each one costs about 244 files of Central's monthly budget. The native `mutastic` already skips
  `-rc` tags, as they aren't plain versions ([A bad Java patch](#a-bad-java-patch)), so nobody is
  moved onto one. Here its tag starts no release workflow, which takes only `vX.Y.Z` tags, and
  `release-verify.yml` skips a pre-release.
- **Patches,** between trains, only for a regression or a security fix, never a feature.
- **Central's budget.** This edition has nothing on Central, but its `X.Y.0` waits on the Java
  edition's. Before a second release in a month, check the
  [Usage Center](https://central.sonatype.com/publishing/usage). Before an urgent release that
  would go over a limit, write to central-support first.
- **Who decides.** Brian decides a patch. Agents never cut a release on their own: when asked,
  they prepare the release commit.

## Each release

The release commit's pull request, then the tag, then the summary to read, one approval, and
`release-verify.yml` to check. In full:

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
2. **The release commit,** `chore(release): version X.Y.Z`, in a pull request: the version in
   `Cargo.toml` (and `Cargo.lock`), the changelog's section for it, and the docs' kept outputs
   that print the version, written again with `MUTASTIC_DOCS_WRITE=1 cargo test --test docs`.
   Each entry since the last release is a file in `changes/`
   ([changes/README.md](changes/README.md)), so pull requests don't conflict over `CHANGELOG.md`.
   Read them as one section, then gather them into `CHANGELOG.md` as `## X.Y.Z (YYYY-MM-DD)`,
   dated today, which deletes the files:
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
4. **The tag.** For `X.Y.0`, wait for the Java edition's `vX.Y.0` to be published with its CLI
   attached. Then, with the release commit merged and `main` green, tag it and push the tag:
   ```sh
   git tag vX.Y.Z <release commit> && git push origin vX.Y.Z
   ```
   Tagged too early, the workflow's `java` job fails within a minute and builds nothing: once the
   Java release is published, re-run the workflow.
5. **Read the summary.** The workflow builds and checks every target, drafts the release, and
   writes what its approver needs as the *draft* job's summary, in the run's page: the version and
   a link to the draft; each asset (each target's archive, `SHA256SUMS`, Scoop's `mutastic.json`
   and the SBOM) with its size and the digest GitHub took; each one's attestation, checked
   ([Checking a download](#checking-a-download)); the notes, the release's section of
   `CHANGELOG.md` (`scripts/changelog.sh notes X.Y.Z`); and the Java edition it pairs with.
   `.github/release-summary.sh` writes it from the draft as it is on GitHub, so it's the
   workflow's account, not a person's or an agent's. A tag whose commit has no section in
   `CHANGELOG.md` fails within a minute, before anything is built.
6. **Approve once.** The *publish* job waits in the `release` environment: approve it (*Review
   deployments*). It checks again that the Java edition `X.Y.*` is published with its CLI
   (`scripts/java-paired.sh`), and refuses, saying so, if it isn't; then it publishes the draft as
   the latest release and starts `release-verify.yml` on it (`.github/release-publish.sh`). It
   refuses, too, while the environment has no required reviewer, so nothing is published that
   nobody approved: give `release` one, with its deployments limited to `v*` tags (*Settings*,
   *Environments*), and re-run the job. Rejected or refused, the release stays a draft, which
   nobody installs: re-run the job once it can pass, or delete the draft and release the next
   patch. Never tag the same version again.
7. **Check `release-verify.yml`** ([After a release](#after-a-release)).

## Before tagging

Run the candidate: the native `mutastic` built from the release commit, over a share of every
fixture's mutants (`scripts/candidate-run.sh`, a few minutes). It fails when a run crashes
(`internal`), can't run (`run-failed`), or finds the tests fail without any mutant, outside the
fixtures built to. Run it on the release's branch, or locally:
```sh
gh workflow run candidate-run.yml --ref <branch>
scripts/candidate-run.sh             # --of=N for a smaller share, or name fixtures to run
```
Don't tag a candidate that fails: its run's summary and artifact say which fixture, and how.

Then run the corpus on the release's branch, real crates each pinned to a commit
([CONTRIBUTING.md](CONTRIBUTING.md#the-corpus)), an hour or so: `gh workflow run corpus.yml --ref
<branch>`. Its summary compares each crate with the last good nightly; don't tag while a crate that
passed there fails.

## After a release

`release-verify.yml` checks the release as a person gets it, on Linux, macOS and Windows, on a
machine that never had mutastic: installed by the `install.sh` and `install.ps1` mutastic.io
serves and from Scoop's `mutastic.json`, each asset held to GitHub's record of it and to its
attestation, `mutastic --version` naming both editions, a run on `fixtures/pricing`, and the Java
edition it pairs with fetched and run on that release's `samples/pricing`. The publish job starts
it once the draft is published, as a draft published by hand does: look at it in *Actions*. To
check a release again:
```sh
gh workflow run release-verify.yml -f version=X.Y.Z   # -f java=X.Y.Z for another Java edition
```
A failure's summary says what failed and where. Anyone who installed it has it, so fix forward:
card it, withdraw the release if it can't be used (below), and release `X.Y.Z+1`.

## Checking a download

`SHA256SUMS` is made beside the archives, so a file swapped along with it still matches. Each
asset of every release after 1.3.1 is also attested: GitHub's signed record that the release
workflow built it, at the release's tag. Check one with the GitHub CLI:
```sh
gh attestation verify mutastic-X.Y.Z-<target>.tar.gz --repo mutastic/mutastic-rs
```
`--signer-workflow mutastic/mutastic-rs/.github/workflows/release.yml --source-ref
refs/tags/vX.Y.Z` holds it to that workflow and tag, as `release-verify.yml` does, and
`--predicate-type https://cyclonedx.org/bom` checks the archive's SBOM attestation instead.

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
