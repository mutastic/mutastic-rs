#!/bin/sh
# Makes the release commit and opens its pull request (RELEASING.md), so no step of it is
# remembered by hand: `just release-prepare X.Y.Z` runs it.
#
#   scripts/release-prepare.sh [--dry-run] X.Y.Z
#
# On a branch release-X.Y.Z from main as GitHub has it, it sets the version in Cargo.toml and
# Cargo.lock, gathers changes/ into CHANGELOG.md's section for X.Y.Z (changelog.sh release),
# writes again the docs' kept outputs, which print the version (MUTASTIC_DOCS_WRITE=1 cargo test
# --test docs), and the pages' copies of the lines that do, keeps what a 1.0+ release takes for
# the next to be held to (MUTASTIC_RELEASED_WRITE=1 cargo test --test compat) and deletes all but
# the newest contract pin (pin-contract.sh prune).
# It commits that as "chore(release): version X.Y.Z", checks the commit with
# `release-check.sh --no-build`, then pushes the branch and opens the pull request.
#
# --dry-run does all of it here, from whatever is checked out, and stops before the push: the
# branch and its commit stay, to read. It never tags or releases; `just release` does that.
set -eu
usage() {
  sed -n '5s/^# *//p' "$0" >&2
  exit 2
}
cd "$(dirname "$0")/.."

dry_run=false
if [ "${1:-}" = --dry-run ]; then
  dry_run=true
  shift
fi
[ "$#" -eq 1 ] || usage
version=$1
if ! printf '%s\n' "$version" | grep -Eqx '[0-9]+\.[0-9]+\.[0-9]+'; then
  echo "release-prepare: '$version' isn't a version, X.Y.Z" >&2
  exit 2
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
body=$tmp/body
branch=release-$version
subject="chore(release): version $version"
if [ -n "$(git status --porcelain)" ]; then
  echo "release-prepare: the tree has changes: commit or stash them first" >&2
  exit 1
fi
if git rev-parse --verify --quiet "refs/heads/$branch" > /dev/null; then
  echo "release-prepare: there's a branch $branch already: delete it, or finish it" >&2
  exit 1
fi
git fetch --quiet origin main
if [ "$(git rev-parse HEAD)" != "$(git rev-parse origin/main)" ]; then
  if [ "$dry_run" = false ]; then
    echo "release-prepare: a release starts from main as GitHub has it: git switch main && git pull" >&2
    exit 1
  fi
  echo "release-prepare: not on origin/main, which a dry run allows"
fi
was=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)

git switch --quiet -c "$branch"

echo "release-prepare: the version, $was to $version, in Cargo.toml and Cargo.lock"
awk -v version="$version" '
  /^\[/ { in_package = ($0 == "[workspace.package]") }
  in_package && !done && /^version = "/ { $0 = "version = \"" version "\""; done = 1 }
  { print }
' Cargo.toml > "$tmp/Cargo.toml"
cat "$tmp/Cargo.toml" > Cargo.toml
cargo update --quiet --workspace --offline

sh scripts/changelog.sh release "$version"
entries=$(git status --porcelain -- changes | wc -l | tr -d ' ')

echo "release-prepare: the docs' kept outputs, with MUTASTIC_DOCS_WRITE=1 cargo test --test docs"
MUTASTIC_DOCS_WRITE=1 cargo test --quiet --locked --test docs
# The pages' own copies of a line that prints the version, which the docs test doesn't write.
# shellcheck disable=SC2016 # $1 is perl's
git ls-files 'docs/*.md' | xargs perl -pi -e \
  's/^(mutastic|Rust edition) \d+\.\d+\.\d+(?=( · |, built in$|$))/$1 '"$version"'/'

major=${version%%.*}
if [ "$major" -ge 1 ]; then
  echo "release-prepare: what $version takes, with MUTASTIC_RELEASED_WRITE=1 cargo test --test compat"
  MUTASTIC_RELEASED_WRITE=1 cargo test --quiet --locked --test compat
fi

sh scripts/pin-contract.sh prune

{
  echo "The $version release commit: the version in Cargo.toml and Cargo.lock,"
  echo "CHANGELOG.md's section, gathered from the $entries entries waiting in"
  echo "changes/, the docs' kept outputs written again, the newest contract"
  if [ "$major" -ge 1 ]; then
    echo "pin alone, and what $version takes kept in tests/released."
  else
    echo "pin alone."
  fi
} > "$body"
git add -A
printf '%s\n\n%s\n' "$subject" "$(cat "$body")" | git commit --quiet --file -
echo "release-prepare: committed $subject on $branch"

if ! sh scripts/release-check.sh --no-build "$version"; then
  echo "release-prepare: put right what release-check lists, on $branch, then push it yourself" >&2
  exit 1
fi

if [ "$dry_run" = true ]; then
  echo "release-prepare: a dry run, so nothing is pushed and no pull request is opened."
  echo "  Read it: git show $branch"
  echo "  Throw it away: git switch - && git branch -D $branch"
  exit 0
fi

git push --quiet -u origin "$branch"
gh pr create --base main --head "$branch" --title "$subject" --body-file "$body"
