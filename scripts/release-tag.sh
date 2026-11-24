#!/bin/sh
# Tags the release commit and pushes the tag, which starts the release workflow (RELEASING.md):
# `just release X.Y.Z` runs it, and it's Brian's to run.
#
#   scripts/release-tag.sh [--dry-run] X.Y.Z
#
# It tags the commit on main as GitHub has it whose subject is "chore(release): version X.Y.Z",
# with or without its " (#N)", once its Cargo.toml says X.Y.Z and no tag vX.Y.Z exists. For X.Y.0
# it first asks the Java releases' feed for a published X.Y.* with its CLI (java-paired.sh), as the
# workflow would, so a tag isn't pushed only to fail within a minute. --dry-run checks all of that
# and prints the commands, without tagging or pushing.
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
  echo "release-tag: '$version' isn't a version, X.Y.Z" >&2
  exit 2
fi
tag=v$version

git fetch --quiet origin main
if git ls-remote --exit-code --tags origin "refs/tags/$tag" > /dev/null; then
  echo "release-tag: $tag is tagged already, and a tag is never moved: release the next patch" >&2
  exit 1
fi
escaped=$(printf '%s' "$version" | sed 's/\./\\./g')
commit=$(git log origin/main --format='%H %s' |
  grep -E "^[0-9a-f]+ chore\(release\): version $escaped( \(#[0-9]+\))?$" |
  head -n 1 | cut -d' ' -f1)
if [ -z "$commit" ]; then
  echo "release-tag: main has no \"chore(release): version $version\": merge its pull request first" >&2
  exit 1
fi
cargo=$(git show "$commit:Cargo.toml" | sed -n 's/^version = "\(.*\)"/\1/p' | head -n 1)
if [ "$cargo" != "$version" ]; then
  echo "release-tag: $(git log -1 --format='%h %s' "$commit") has Cargo.toml $cargo, not $version" >&2
  exit 1
fi
case $version in
  *.0)
    java=$(sh scripts/java-paired.sh "$version") || exit 1
    echo "release-tag: the Java edition's $java is published, for the native $version to run"
    ;;
esac

echo "release-tag: $tag at $(git log -1 --format='%h %s' "$commit")"
if [ "$dry_run" = true ]; then
  echo "release-tag: a dry run, so nothing is tagged or pushed. It would run:"
  echo "  git tag $tag $commit && git push origin $tag"
  exit 0
fi
git tag "$tag" "$commit"
git push origin "$tag"
