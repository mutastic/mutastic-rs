# The build's verbs, one word each, the same in both editions (mutastic has the Java one): `just`
# lists them. A recipe is one line over cargo or a script, so the words outlive the tool beneath.

set minimum-version := "1.59.0"
set default-list
# A recipe's arguments reach its commands as "$@", each as it was typed: a glob or a space in one
# isn't split or expanded again.
set positional-arguments

# The mutastic that self-shard and self-merge run: self-build's, or CI's download of it.
self-bin := env('MUTASTIC_SELF_BIN', 'target/release/mutastic')

# Turns on the commit message hook
[group('setup')]
setup:
    git config core.hooksPath .githooks

# Compiles mutastic
[group('build')]
build:
    cargo build --locked

# Runs mutastic from here: just run run fixtures/pricing
[group('build')]
run *args:
    cargo run --locked --bin mutastic -- "$@"

# Installs the mutastic command built from here
[group('build')]
install:
    cargo install --locked --path crates/mutastic

# Formats the code
[group('check')]
fmt:
    cargo fmt --all

# Checks the format and every lint, as errors, without the tests
[group('check')]
lint: && clippy
    cargo fmt --all --check

# Lints every target, warnings as errors
[group('check')]
clippy:
    cargo clippy --all-targets --locked -- -D warnings

# Runs the tests, or those the arguments pick: just test --test docs
[group('check')]
test *args:
    cargo test --locked "$@"

# Everything a pull request needs to pass: the format, the lints and the tests
[group('check')]
check: lint test

# (MUTASTIC_DOCS_COVERAGE makes the coverage page's test fail without cargo-llvm-cov, not skip.)

# What CI runs on a pull request: the checks, the oldest toolchain and the audit
[env('MUTASTIC_DOCS_COVERAGE', '1')]
[env('MUTASTIC_GITHUB', 'off')]
[group('check')]
ci: lint && msrv audit
    cargo test --locked

# Builds on the oldest toolchain rust-version in Cargo.toml names
[group('check')]
msrv:
    cargo +"$(sed -n 's/^rust-version = "\(.*\)"/\1/p' Cargo.toml)" build --all-targets --locked

# Checks the dependencies' advisories, licences and sources with cargo-deny
[group('check')]
audit:
    cargo deny check

# Fuzzes one target on nightly: just fuzz config -- -max_total_time=300
[env('ASAN_OPTIONS', 'detect_leaks=0')]
[group('unsafe')]
fuzz TARGET *args:
    cargo +nightly fuzz run "$@"

# Runs the terminal crate's tests under Miri, on nightly
[group('unsafe')]
miri *args:
    cargo +nightly miri test -p mutastic-terminal --locked "$@"

# Runs the terminal crate's tests under AddressSanitizer, on nightly, from a terminal
[env('RUSTFLAGS', '-Zsanitizer=address')]
[group('unsafe')]
asan:
    cargo +nightly test -p mutastic-terminal --locked --lib --target "$(rustc -vV | sed -n 's/^host: //p')"

# Checks this edition against the editions' contract at its newest pin
[group('contract')]
contract:
    cargo test --locked --test contract

# Pins the contract at a mutastic commit or branch, main by default, for this pull request
[group('contract')]
pin-contract SLUG *REF:
    sh scripts/pin-contract.sh "$@"

# Vendors the HTML report's page from a mutastic commit
[group('contract')]
pin-report-page COMMIT:
    sh scripts/pin-report-page.sh "$@"

# Writes the docs' copies of what mutastic prints and takes again: read the diff
[env('MUTASTIC_DOCS_WRITE', '1')]
[group('docs')]
docs-output:
    cargo test --locked --test docs

# Writes the kept ids again, after a new operator or shape: lines are only added
[env('MUTASTIC_IDS_WRITE', '1')]
[group('docs')]
ids:
    cargo test --locked --test ids

# Prints the next release's changelog section, as it would read
[group('merge')]
changelog-preview *args:
    sh scripts/changelog.sh preview "$@"

# Checks the changelog's entries, and that only a release edits CHANGELOG.md
[group('merge')]
changelog-check *args:
    sh scripts/changelog.sh check "$@"

# Merges a pull request through the merge queue: just merge 59 body.txt [--wait]
[group('merge')]
merge N *body:
    sh .githooks/merge-pr "$@"

# The release, in RELEASING.md's order. Agents may run release-prepare, release-check,
# release-verify and candidate; release is Brian's. [confirm] only asks: `--yes` skips it, so the
# `release` environment's approval in CI is what stands between a tag and a published release.

# Runs the release candidate, the native mutastic from here over a share of every fixture: just candidate --of=8
[group('release')]
candidate *args:
    bash scripts/candidate-run.sh "$@"

# Makes the release commit on a branch and opens its pull request: just release-prepare 1.4.0
[group('release')]
release-prepare VERSION:
    sh scripts/release-prepare.sh "$@"

# Checks what a release needs, all at once, writing nothing tracked: just release-check 1.4.0
[group('release')]
release-check VERSION:
    sh scripts/release-check.sh "$@"

# Brian's: tags main's release commit vVERSION and pushes the tag, which builds the draft release
[confirm("Push the tag vVERSION on main's release commit? release.yml then builds the draft release, waiting on the release environment's approval")]
[group('release')]
release VERSION:
    sh scripts/release-tag.sh "$@"

# Checks a release as a person gets it, on Linux, macOS and Windows: just release-verify 1.4.0
[group('release')]
release-verify VERSION:
    gh workflow run release-verify.yml -f version="$1"

# CI's jobs that `just ci` doesn't cover, so a job can be run here as CI runs it.

# Builds the release mutastic that self-shard and self-merge run on mutastic
[group('ci')]
self-build:
    cargo build --release --locked --bin mutastic

# Runs a share of mutastic's mutants on itself, no threshold: just self-shard 1/28
[group('ci')]
self-shard SHARE:
    {{ self-bin }} run crates/mutastic --shard="$1" --workers=2 --threshold=0 --format=concise --progress=counter

# Puts the shares in DIR together as mutastic's whole run on itself
[group('ci')]
self-merge DIR:
    {{ self-bin }} run crates/mutastic --from-shards="$1" --threshold=0 --format=concise

# Runs mutastic on the terminal crate, whole, no threshold
[group('ci')]
self-terminal:
    {{ self-bin }} run crates/mutastic-terminal --workers=2 --threshold=0 --format=concise --progress=counter

# Runs a crate of corpus/corpus.yaml as the nightly does, in CI; here, only a file:// fixture's entry
[group('ci')]
corpus *args:
    MUTASTIC_BIN="{{ self-bin }}" bash scripts/corpus-run.sh "$@"

# Checks the justfile's format, and that its verbs are those .github/just-verbs.txt lists
[group('ci')]
[script('bash')]
justfile-check:
    set -euo pipefail
    just="{{ just_executable() }}"
    "$just" --justfile justfile --fmt --check
    if ! "$just" --justfile justfile --summary | tr ' ' '\n' | diff -u .github/just-verbs.txt -; then
      echo "The verbs aren't those .github/just-verbs.txt lists (-), but the justfile's (+)." >&2
      echo "People and agents type them: keep a verb, or say it's gone in the list and the docs." >&2
      exit 1
    fi
