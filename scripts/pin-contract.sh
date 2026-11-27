#!/bin/sh
# Pins the editions' contract for one pull request: writes contract/pins/<slug>.json, naming a
# commit of the Java repository, its committer date and its position on main.
# tests/contract.rs reads the contract at the newest pin, the one furthest along main, so pull
# requests each add a file and none edits another's. contract/README.md says how it works.
#
#   scripts/pin-contract.sh <slug> [<commit or branch>]   # mutastic main when not given
#   scripts/pin-contract.sh prune                         # at a release: delete all but the newest
#
# The pin logic is the contract's shared tooling, docs/scripts/contract.mjs in mutastic, which
# every edition calls: this fetches it from mutastic main and runs it here. It needs node.
set -eu
usage="usage: scripts/pin-contract.sh <slug> [<commit or branch>] | prune"
slug=${1:?$usage}
cd "$(dirname "$0")/.."
command -v node > /dev/null || {
  echo "pin-contract: node isn't on the PATH, and the contract's tooling needs it" >&2
  exit 1
}
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
raw=$(jq -r .raw contract/source.json)
curl --fail --silent --show-error --location "$raw/main/$(jq -r .tool contract/source.json)" \
  -o "$dir/contract.mjs"
if [ "$slug" = prune ]; then
  node "$dir/contract.mjs" prune
else
  node "$dir/contract.mjs" pin "$@"
fi
