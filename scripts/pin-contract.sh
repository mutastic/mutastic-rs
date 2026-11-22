#!/bin/sh
# Pins the editions' contract for one pull request: writes contract/pins/<slug>.json, naming a
# commit of the Java repository, its committer date and its position on main.
# tests/contract.rs reads the contract at the newest pin, the one furthest along main, so pull
# requests each add a file and none edits another's. contract/README.md says how it works.
#
#   scripts/pin-contract.sh <slug> [<commit or branch>]   # mutastic main when not given
#   scripts/pin-contract.sh prune                         # at a release: delete all but the newest
set -eu
usage="usage: scripts/pin-contract.sh <slug> [<commit or branch>] | prune"
slug=${1:?$usage}
cd "$(dirname "$0")/.."
if [ "$slug" = prune ]; then
  newest=$(for pin in contract/pins/*.json; do
    echo "$(jq -r '"\(.position) \(.date)"' "$pin") $pin"
  done | sort -k1,1n -k2,2 | tail -n 1 | cut -d' ' -f3)
  for pin in contract/pins/*.json; do
    [ "$pin" = "$newest" ] || rm "$pin"
  done
  echo "kept $newest"
  exit 0
fi
case $slug in
  *[!a-z0-9-]*)
    echo "pin-contract: a slug is lower case letters, digits and hyphens, not '$slug'" >&2
    exit 1
    ;;
esac
at=${2:-main}
repository=$(jq -r .repository contract/source.json)
api=https://api.github.com/repos/${repository#https://github.com/}
anchor=$(jq -r .anchor contract/source.json)
found=$(curl --fail --silent --show-error --location "$api/commits/$at" |
  jq -r '.sha + " " + .commit.committer.date')
commit=${found% *}
date=${found#* }
# Where the commit sits on main: the commits since the anchor. Dates don't order main's commits,
# as the merge queue lands them, and a commit the anchor isn't behind can't be placed.
placed=$(curl --fail --silent --show-error --location "$api/compare/$anchor...$commit" |
  jq -r '"\(.ahead_by) \(.behind_by)"')
if [ "${placed#* }" != 0 ]; then
  echo "pin-contract: $commit doesn't come after the anchor $anchor on mutastic main" >&2
  exit 1
fi
cat > "contract/pins/$slug.json" <<JSON
{
  "commit": "$commit",
  "date": "$date",
  "position": ${placed% *}
}
JSON
echo "pinned the contract at $commit (position ${placed% *}, $date) in contract/pins/$slug.json"
