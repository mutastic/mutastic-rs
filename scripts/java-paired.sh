#!/bin/sh
# Prints the tag of the newest Java edition release X.Y.*, `v1.3.2`, for the Rust edition X.Y.Z,
# from the Java releases' feed; or fails, saying to tag the Rust edition once there is one:
#   sh scripts/java-paired.sh 1.3.0 [https://github.com/mutastic/mutastic/releases.atom | feed.atom]
# The native `mutastic X.Y.Z` runs the newest Java X.Y.*, so the release workflow checks for it
# before building anything, rather than failing half an hour later when its install runs one. The
# feed lists published releases only, never a draft, as the native `mutastic` reads it; a feed
# fetched from GitHub also needs the release's CLI attached.

set -eu

version=$1
feed=${2:-https://github.com/mutastic/mutastic/releases.atom}
line=$(printf '%s\n' "$version" | sed -n 's/^\([0-9][0-9]*\.[0-9][0-9]*\)\.[0-9][0-9]*$/\1/p')
[ -n "$line" ] || { printf 'java-paired: %s is not a version X.Y.Z\n' "$version" >&2; exit 2; }

case $feed in
  http://* | https://*) entries=$(curl -fsSL --retry 3 --max-time 30 "$feed") ;;
  *) entries=$(cat "$feed") ;;
esac

# Each release the feed links to, `.../releases/tag/v1.3.2`, newest patch of the line last.
newest=$(printf '%s\n' "$entries" |
  grep -o '/releases/tag/v[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*["<]' |
  sed 's|^/releases/tag/v||; s|.$||' |
  awk -F. -v line="$line" '$1 "." $2 == line' |
  sort -t. -k3,3n | tail -n 1)

if [ -z "$newest" ]; then
  printf 'tag the Rust edition after Java %s.* is published: mutastic %s runs the newest Java %s.*, and %s lists none\n' \
    "$line" "$version" "$line" "$feed" >&2
  exit 1
fi

# The native `mutastic` fetches the release's CLI, which the Java release job attaches only once
# it's approved: until then the release is listed, and there's nothing to run.
case $feed in
  http://* | https://*)
    cli="${feed%.atom}/download/v$newest/mutastic-$newest.tar"
    if ! curl -fsIL --retry 3 --max-time 30 -o /dev/null "$cli"; then
      printf 'tag the Rust edition after Java %s.* is published: Java %s is listed, but has no %s attached yet\n' \
        "$line" "$newest" "mutastic-$newest.tar" >&2
      exit 1
    fi
    ;;
esac
printf 'v%s\n' "$newest"
