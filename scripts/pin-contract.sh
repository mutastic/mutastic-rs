#!/bin/sh
# Pins the editions' contract for one pull request: writes contract/pins/<slug>.json, naming a
# commit of the Java repository and its committer date. tests/contract.rs reads the contract at
# the newest pin, so pull requests each add a file and none edits another's. contract/README.md
# says how it works.
#
#   scripts/pin-contract.sh <slug> [<commit or branch>]   # mutastic main when not given
#   scripts/pin-contract.sh prune                         # at a release: delete all but the newest
set -eu
usage="usage: scripts/pin-contract.sh <slug> [<commit or branch>] | prune"
slug=${1:?$usage}
cd "$(dirname "$0")/.."
if [ "$slug" = prune ]; then
  newest=$(for pin in contract/pins/*.json; do
    echo "$(jq -r .date "$pin") $pin"
  done | sort | tail -n 1 | cut -d' ' -f2)
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
found=$(curl --fail --silent --show-error --location "$api/commits/$at" |
  jq -r '.sha + " " + .commit.committer.date')
commit=${found% *}
date=${found#* }
cat > "contract/pins/$slug.json" <<JSON
{
  "commit": "$commit",
  "date": "$date"
}
JSON
echo "pinned the contract at $commit ($date) in contract/pins/$slug.json"
