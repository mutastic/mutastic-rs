# Contributing to mutastic for Rust

Thanks for helping. Bug reports, ideas and pull requests are all welcome.

## Reporting a bug or asking for something

Open an [issue](https://github.com/mutastic/mutastic-rs/issues/new/choose). For a bug, the form asks
for what helps most: `mutastic --version`, `rustc --version`, how you ran it (`mutastic` or `cargo
mutastic`), the command and what it printed. A small crate that shows the problem is best of all.

For a security problem, don't open an issue: see [SECURITY.md](SECURITY.md).

Everyone taking part in issues, pull requests and discussions agrees to the
[Code of Conduct](CODE_OF_CONDUCT.md). To report a breach of it, write to conduct@mutastic.io.

## Setting up

You need a stable Rust toolchain with `rustfmt` and `clippy`. `rust-version` in
[Cargo.toml](Cargo.toml) is the oldest that builds mutastic, and CI builds on it as well as on the
latest stable. The build's commands need [just](https://just.systems) 1.59.0 or later:
`brew install just`, `scoop install just`, `winget install --id Casey.Just` or
`cargo install just --locked`. On Windows, recipes run in Git Bash's `sh`.

```sh
git clone https://github.com/mutastic/mutastic-rs.git
cd mutastic-rs
just setup
just check
```

`just setup` turns on the commit message hook, once per clone. `just check` runs everything a pull
request needs to pass, as CI does on Linux and macOS: the format, every clippy lint as an error,
and the tests. The tests build and run mutastic on the crates in `fixtures/`, so the first run
takes a few minutes; later ones reuse the fixtures' builds. CI also builds on the oldest toolchain
(`just msrv`) and checks the dependencies' licences and advisories with
[cargo-deny](https://github.com/EmbarkStudios/cargo-deny), against [deny.toml](deny.toml)
(`just audit`); `just ci` runs all of it.

Useful along the way:

```sh
just fmt                       # format the code
just test --test docs          # one integration test file
just test --lib sites          # the unit tests in one module
just run run fixtures/pricing  # mutastic, built from here
```

[ARCHITECTURE.md](ARCHITECTURE.md) explains how the pieces fit, and `fixtures/` holds the small
crates the tests run mutastic on.

## Commands

A job spelled in cargo's words is spelled differently in the Java edition, and again after a
change of build tool. So the everyday ones are recipes in the [justfile](justfile), a word each,
meaning the same in both editions: `just --list` shows them, `just check` runs everything a pull
request needs to pass, and `just ci` what CI runs. Each is one line over cargo or a script, which
the justfile shows (`just test` is `cargo test --locked`), so the work can still be done without
it. Call `cargo` directly only for a job no recipe covers.

## Making a change

- **Follow [AGENTS.md](AGENTS.md).** It's the conventions the code is held to and why. Most are
  enforced by CI; the rest are judgement, written down.
- **mutastic's own rules:**
  - Its output never names other mutation-testing tools.
  - Every command it offers parses: the docs test checks each one a page shows.
  - A mutant's id never changes while its function's code doesn't. `tests/ids.rs` holds every
    fixture's mutants to the list in `crates/mutastic/tests/ids/ids.tsv`.
  - A catch it can't trust (a crash near unsafe code, a crash in std's own checks) doesn't count as
    caught.
  - A card says what its mutant does: `tests/truth.rs` and the docs test check it by running the
    mutants.
- **Tests:** a change in behaviour comes with a test that fails without it.
- **Docs:** user-facing changes update the pages in `docs/` (see [docs/README.md](docs/README.md)).
  After changing what mutastic prints or takes, run `just docs-output`, which writes the docs'
  copies of its output again, then read the diff.
- **The changelog:** a change someone using mutastic would notice gets an entry, as a file of its
  own in `changes/<section>/` ([changes/README.md](changes/README.md) has the sections and the
  shape). Don't edit `CHANGELOG.md`: a release gathers the files into it, and CI fails a pull
  request that edits it, as pull requests that all added to its end conflicted in the merge queue.
- **Ids:** after adding an operator, or a shape an operator finds, run `just ids`. Lines may be
  added to the list, never changed or
  removed: a changed line is an id that broke.
- **Dependencies:** commit `Cargo.lock` with the change. A new dependency's licence must be one
  [deny.toml](deny.toml) allows.

## Keeping the editions level

The Rust and Java editions are held to one contract, `mutastic.contract/v1`, kept in the Java
repository at
[`docs/public/contract/v1.json`](https://github.com/mutastic/mutastic/blob/main/docs/public/contract/v1.json).
It holds what both must agree on (the command line, error codes and exit codes, help's layout,
the summary's rows, the agent format's lines, the events' and mutation.json's key paths, the
schemas by hash) and a `differences` list: each way an edition differs, as `engine` (with a
reason), `dated` (with an `until`) or `pending` (with an issue).
[Keeping the editions level](https://github.com/mutastic/mutastic/blob/main/CONTRIBUTING.md#keeping-the-editions-level)
in the Java repository says how it works.

- **The check** is `tests/contract.rs`. It runs the built binary on a copy of `fixtures/pricing`,
  reads `docs/commands.json`, and fails on any difference the contract doesn't record for `rust`,
  and on a recorded one that has gone ("now matches").
- **The pin.** The check reads the contract at a commit of the Java repository, fetched once into
  the target directory: the newest pin in [`contract/pins`](contract/pins), one file to a pull
  request, so no two conflict ([contract/README.md](contract/README.md)).
  `MUTASTIC_CONTRACT=<file> just contract` checks against a working copy instead,
  while a contract change is being written.
- **Adding an option, or closing a gap:** change the contract in a mutastic pull request first
  (the option, or `rust` taken out of its pending entry), then pin that merge commit in the pull
  request that changes mutastic-rs: `just pin-contract <slug>` adds its pin, and `just contract`
  runs the check.
- **The weekly drift job** (`.github/workflows/contract-drift.yml`) runs the check against mutastic
  main, and fails visibly when the two have moved apart.
- **A new edition** built on this one (Swift) starts from `tests/contract.rs` and its own port of
  the pricing fixture, adds itself to the contract's `editions`, and records each failure as a
  difference.

## Pull requests

- One change per pull request, with a description of what it changes and why. The template asks.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org) and 50/72: a
  `type(scope): subject` of at most 50 characters, imperative and lower case, a blank line, then a
  body wrapped at 72 that explains why: what was wrong or missing, and what the change does about it.
- CI must pass.

## Commit messages

What [Pull requests](#pull-requests) asks of a commit message, in full:

- **Subject:** no full stop at the end. Imperative means "add", not "adds" or "added".
- **Type:** `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `build`, `ci`, `chore` or
  `revert`. A breaking change adds `!` before the colon (`feat(cli)!: …`), or a
  `BREAKING CHANGE:` footer.
- **Scope:** optional and lower case, the crate area changed: `engine` (building and running
  mutants: `engine`, `compile`, `testrun`, `workspace`), `sites` (where the operators find mutants),
  `report` (cards' words, report files, the events schema, suggested tests), `cli` (commands,
  config, ignores, `--since`, shares), `gate`, `java` (handing Maven and Gradle builds to the Java
  edition), `cargo` (`cargo mutastic`), `fixtures`, `docs`, `ci` or `release`. Leave it out when a
  change spans several.
- **Long lines:** URLs, indented or fenced code, and trailers such as `Co-Authored-By:` (last)
  may run past 72.

Two from the log, as they'd be written now:

```text
fix(report): write paths with / on Windows

Before a Windows build is promised, the tests have to pass there. A
windows-latest job found cards, report paths and the gate's lines
written with \, so they read differently from Linux and macOS. A source
file's path is kept with / from the start, which Windows takes too.
```

```text
feat(cli): split a run across CI jobs with --shard

The Java edition splits one run across CI jobs and puts it back
together, so the Rust edition does the same, in the same words and with
the same events: --shard=N/M runs one share of the mutants, and
--from-shards=DIR puts the shares back together as the whole run.
```

Commits before 9 Nov 2026 use the older style, a long subject that says why, and are not
rewritten.

`.githooks/commit-msg` checks each message as you commit, and lets merge, revert, `fixup!` and
`squash!` commits through as git writes them. `just setup` turns it on, once in each clone
(`git config core.hooksPath .githooks`, if you'd rather).

CI runs the same check on each new commit in a push or pull request (`.githooks/check-commits`),
counting from the commit that added the hook. `sh .githooks/test-commit-msg` tests the hook.

A pull request is squash-merged, and GitHub writes its title and " (#<number>)" as the subject on
`main`, where the check counts the whole line. So the title follows the same rules with the
suffix counted: at most 44 characters for #10 to #99, 43 for #100 to #999. CI checks it
(`.githooks/check-title`), again whenever the title is edited, and says the limit for the pull
request's number.

Pull requests land through a merge queue. A pull request that is green against an older `main`
can still turn `main` red once merged beside another one, so the queue tests each pull request
merged with the latest `main` and every pull request ahead of it, with the same checks, and
merges it only if that passes. One that fails there leaves the queue, and `main` stays green.

Merge with `just merge` (`.githooks/merge-pr`), not `gh pr merge` by hand. It writes the subject from the title,
takes the body from a file (wrapped at 72) or leaves it empty, checks the whole message with
`commit-msg`, and adds the pull request to the queue for the commit the checks ran on, once none has
failed: the queue takes it only once they all pass. The queue writes the squash commit from the
title and the pull request's description, so merge-pr makes the body the description first; don't
edit it after. In the queue, the `squash-title` check reads that commit, body and all. merge-pr
returns once the pull request is queued; with `--wait` it stays until it lands and prints the commit
on `main`, or says why it left the queue (run it with `--wait` again to wait for one already
queued). GitHub deletes the branch once it lands; local branches and worktrees are left alone.
With a queue, it waits only on the checks the rules require, as the queue does, so a check that
isn't required (a self-run share) doesn't stop it. `--admin` skips that wait; with a queue the pull
request still goes through it, as the rules let no one merge past it, and without one it merges
past the checks where the rules let an admin. The message is checked either way.

```sh
just merge 59                   # subject "<title> (#59)", no body; queued
just merge 59 body.txt          # with a body that says why
just merge 59 body.txt --wait   # and wait for it to land on main
```

## Releases

A release is a version tag, `v<version>`, matching the version in `Cargo.toml`: the release
workflow builds mutastic for each target from it, as a draft to read and publish. See
[How a release is laid out](docs/install.md#how-a-release-is-laid-out). Nothing is on crates.io
yet: the crate names `mutastic`, `cargo-mutastic` and `mutastic-rs` are reserved there, and every
crate here is `publish = false`.

## Licence

mutastic is under the [Apache License 2.0](LICENSE). Contributions are accepted under the same
licence: by opening a pull request, you agree yours is offered under it.
