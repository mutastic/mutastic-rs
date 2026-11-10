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
