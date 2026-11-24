# The build's verbs, one word each, the same in both editions (mutastic has the Java one): `just`
# lists them. A recipe is one line over cargo or a script, so the words outlive the tool beneath.

set minimum-version := "1.59.0"
set default-list
# A recipe's arguments reach its commands as "$@", each as it was typed: a glob or a space in one
# isn't split or expanded again.
set positional-arguments

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
lint:
    cargo fmt --all --check
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

# Merges a pull request through the merge queue: just merge 59 body.txt [--wait]
[group('merge')]
merge N *body:
    sh .githooks/merge-pr "$@"
