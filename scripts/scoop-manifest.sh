#!/bin/sh
# Prints Scoop's manifest for a release, scoop/mutastic.json with the release's version and the
# hashes its SHA256SUMS gives the Windows zips, for a bucket:
#   sh scripts/scoop-manifest.sh 0.2.1 SHA256SUMS > mutastic.json
# The release workflow attaches it to each release. Needs jq.

set -eu

version=$1 sums=$2
manifest=$(dirname "$0")/../scoop/mutastic.json

# The checksum SHA256SUMS gives a file, by its whole name.
sum() {
  found=$(awk -v file="$1" '$2 == file || $2 == "*" file { print $1 }' "$sums")
  [ -n "$found" ] || { printf 'scoop-manifest: SHA256SUMS lists no %s\n' "$1" >&2; exit 1; }
  printf '%s\n' "$found"
}

x64=$(sum "mutastic-$version-x86_64-pc-windows-msvc.zip")
arm64=$(sum "mutastic-$version-aarch64-pc-windows-msvc.zip")
jq --arg version "$version" --arg x64 "$x64" --arg arm64 "$arm64" '
  def release($arch; $hash):
    .autoupdate.architecture[$arch] | map_values(gsub("\\$version"; $version)) + {hash: $hash};
  .version = $version
  | .architecture["64bit"] = release("64bit"; $x64)
  | .architecture.arm64 = release("arm64"; $arm64)' "$manifest"
