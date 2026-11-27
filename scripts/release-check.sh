#!/bin/sh
# Checks what has to hold before vX.Y.Z is tagged, all at once: one exit code, and a list of what's
# wrong. RELEASING.md runs it on the release commit, and CI on a release pull request.
#
#   scripts/release-check.sh [--no-build | --build] X.Y.Z
#   scripts/release-check.sh [--no-build | --build] --subject <subject>
#
# - the changelog: X.Y.Z has entries, and a patch has no scoring changes (changelog.sh check --for);
# - the version: Cargo.toml and Cargo.lock say X.Y.Z, as the tag must;
# - the contract's pins: only the newest is left (scripts/pin-contract.sh prune);
# - the contract at that pin: Rust has no pending entry without an issue and a target, or with a
#   target gone by, and no dated one at its end (the shared docs/scripts/contract.mjs release);
# - ids: MUTASTIC_IDS_WRITE=1 cargo test --test ids leaves tests/ids as committed.
#
# The ids build, so take minutes: --no-build skips them, --build runs only them. With --subject,
# the version is the one a release's commit subject names, "chore(release): version X.Y.Z", with
# or without its " (#N)"; any other subject has nothing to check. It writes nothing that's
# tracked, and publishes and pushes nothing.
set -u
usage() {
  sed -n '5,6s/^# *//p' "$0" >&2
  exit 2
}
cd "$(dirname "$0")/.."

checks=all
case ${1:-} in
  --no-build) checks=quick && shift ;;
  --build) checks=build && shift ;;
esac
if [ "${1:-}" = --subject ]; then
  [ "$#" -eq 2 ] || usage
  version=$(printf '%s\n' "$2" | sed -nE 's/^chore\(release\): version ([0-9]+\.[0-9]+\.[0-9]+)( \(#[0-9]+\))?$/\1/p')
  if [ -z "$version" ]; then
    echo "release-check: \"$2\" isn't a release, so there's nothing to check"
    exit 0
  fi
else
  [ "$#" -eq 1 ] || usage
  version=$1
fi
if ! printf '%s\n' "$version" | grep -Eqx '[0-9]+\.[0-9]+\.[0-9]+'; then
  echo "release-check: '$version' isn't a version, X.Y.Z" >&2
  exit 2
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
: > "$tmp/problems"

# problem <what's wrong>
problem() {
  printf '%s\n' "$*" >> "$tmp/problems"
}

# step <name>: says what's being checked.
step() {
  echo "release-check: $1"
}

# A command's failure lines become problems, each line one; the rest of what it says goes to $tmp/log.
# run <prefix> <command...>
run() {
  prefix=$1
  shift
  if ! "$@" > "$tmp/log" 2>&1; then
    grep -E "^($prefix|  - )" "$tmp/log" | sed -E "s/^($prefix:? |  - )//" > "$tmp/found"
    if [ -s "$tmp/found" ]; then
      while IFS= read -r found; do problem "$found"; done < "$tmp/found"
    else
      tail -n 20 "$tmp/log" | sed 's/^/    /' >&2
      problem "$* failed: what it printed is above"
    fi
    return 1
  fi
}

# unchanged <command> <path>: running the command leaves the path as committed.
unchanged() {
  command=$1
  path=$2
  if [ -n "$(git status --porcelain -- "$path")" ]; then
    problem "$path has changes not committed: commit or stash them, then check again"
    return
  fi
  if ! sh -c "$command" > "$tmp/log" 2>&1; then
    tail -n 20 "$tmp/log" | sed 's/^/    /' >&2
    problem "$command failed: what it printed is above"
    return
  fi
  changed=$(git status --porcelain -- "$path")
  if [ -n "$changed" ]; then
    problem "$command changed $path: new ids, of an operator new since the last release; read the diff and commit it, so they're held from $version on: $(printf '%s' "$changed" | tr '\n' ' ')"
  fi
}

if [ "$checks" != build ]; then
  step "the changelog for $version"
  run changelog sh scripts/changelog.sh check --for "$version"

  step "the version in Cargo.toml and Cargo.lock"
  cargo=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
  locked=$(awk '/^name = "mutastic"$/ { getline; gsub(/^version = "|"$/, ""); print; exit }' Cargo.lock)
  if [ "$cargo" != "$version" ] || [ "$locked" != "$version" ]; then
    problem "Cargo.toml's version is $cargo and Cargo.lock's $locked, not $version: the release commit sets both (RELEASING.md)"
  fi

  step "the contract's pins"
  pins=$(find contract/pins -name '*.json' | wc -l | tr -d ' ')
  if [ "$pins" -gt 1 ]; then
    problem "contract/pins has $pins pins: scripts/pin-contract.sh prune keeps only the newest"
  elif [ "$pins" -eq 0 ]; then
    problem "contract/pins has no pin: the contract tests need the newest"
  else
    step "the contract at the pin: no pending entry for rust that isn't registered"
    if command -v node > /dev/null; then
      commit=$(jq -r .commit contract/pins/*.json)
      raw=$(jq -r .raw contract/source.json)
      if curl --fail --silent --show-error --location "$raw/$commit/$(jq -r .path contract/source.json)" -o "$tmp/contract.json" \
        && curl --fail --silent --show-error --location "$raw/$commit/$(jq -r .tool contract/source.json)" -o "$tmp/contract.mjs"; then
        # Its problems are its "  - " lines; no line starts with the prefix, so its count isn't one.
        run 'contract-release' node "$tmp/contract.mjs" release rust "$version" --contract "$tmp/contract.json"
      else
        problem "couldn't fetch the contract and its tooling at the pin, $commit"
      fi
    else
      problem "node isn't on the PATH, and the contract's release rule needs it"
    fi
  fi
fi

if [ "$checks" != quick ]; then
  step "ids, with MUTASTIC_IDS_WRITE=1 cargo test --test ids"
  unchanged "MUTASTIC_IDS_WRITE=1 cargo test --locked --test ids" crates/mutastic/tests/ids
fi

count=$(wc -l < "$tmp/problems" | tr -d ' ')
if [ "$count" -eq 0 ]; then
  echo "release-check: $version is ready to tag"
  exit 0
fi
echo "release-check: $version isn't ready, $count to put right:" >&2
sed 's/^/  - /' "$tmp/problems" >&2
exit 1
