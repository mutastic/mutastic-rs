#!/usr/bin/env bash
# What the approver of a release reads before approving it: the draft release TAG as it is on
# GitHub, written by this script, not by a person or an agent. The version and a link to the draft;
# the Java edition it pairs with; each asset with its size and the digest GitHub took as it was
# uploaded; each asset held to its attestation from the workflow that built it, at TAG; and the
# notes, the release's section of CHANGELOG.md. release.yml writes it as the draft job's summary,
# where it waits beside the publish job's *Review deployments*.
#
#   GH_TOKEN=... .github/release-summary.sh vX.Y.Z vJAVA [owner/repo]
#
# The attestations are held to the workflow running this one (GITHUB_WORKFLOW_REF), release.yml
# when it's run by hand. It fails when an asset's attestation doesn't verify, after writing the
# rest, so the approver sees what's wrong.
set -u

tag=${1:?"usage: release-summary.sh vX.Y.Z vJAVA [owner/repo]"}
java=${2:?"usage: release-summary.sh $tag vJAVA [owner/repo]"}
repo=${3:-${GITHUB_REPOSITORY:?"name the repository: release-summary.sh $tag $java owner/repo"}}
summary=${GITHUB_STEP_SUMMARY:-/dev/stdout}
workflow_ref=${GITHUB_WORKFLOW_REF:-$repo/.github/workflows/release.yml@refs/tags/$tag}
signer=${workflow_ref%@*}
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
failed=0

say() { printf '%s\n' "$@" >> "$summary"; }

release=$(gh release view "$tag" --repo "$repo" --json isDraft,url,body,assets) ||
  { echo "::error::there's no release $tag in $repo"; exit 1; }
if [ "$(jq -r .isDraft <<< "$release")" != true ]; then
  echo "::error::$tag is already published"
  exit 1
fi

say "## mutastic ${tag#v}, a draft to publish" ""
say "- **The draft:** $(jq -r .url <<< "$release")"
version=${tag#v}
say "- **The Java edition it pairs with:** [$java](https://github.com/mutastic/mutastic/releases/tag/$java), the newest Java ${version%.*}.* published with its CLI, which \`mutastic $version\` fetches"
say "- **Publishing it:** approve the publish job's deployment to \`release\` (*Review deployments*). The job checks the Java edition again, then publishes the draft as the latest release and starts \`release-verify.yml\`." ""

say "### Assets" "" "| Asset | Size | SHA-256, as GitHub received it |" "| --- | ---: | --- |"
jq -r '.assets[] | "| `\(.name)` | \(.size) | `\(.digest // "none" | ltrimstr("sha256:"))` |"' <<< "$release" >> "$summary"
say ""

say "### Attestations, by \`${signer#"$repo"/}\` at \`$tag\`" ""
if ! gh release download "$tag" --repo "$repo" --dir "$dir" > /dev/null; then
  say "- ✗ the draft's assets couldn't be downloaded to check"
  failed=1
fi
for file in "$dir"/*; do
  [ -f "$file" ] || continue
  name=$(basename "$file")
  if gh attestation verify "$file" --repo "$repo" --signer-workflow "$signer" \
    --source-ref "refs/tags/$tag" > /dev/null 2>&1; then
    say "- ✓ \`$name\`"
  else
    say "- ✗ \`$name\` has no attestation that verifies"
    echo "::error::$name: gh attestation verify fails"
    failed=1
  fi
done
for file in "$dir"/*.tar.gz "$dir"/*.zip; do
  [ -f "$file" ] || continue
  name=$(basename "$file")
  if gh attestation verify "$file" --repo "$repo" --signer-workflow "$signer" \
    --source-ref "refs/tags/$tag" --predicate-type https://cyclonedx.org/bom > /dev/null 2>&1; then
    say "- ✓ \`$name\`'s SBOM"
  else
    say "- ✗ \`$name\` has no SBOM attestation that verifies"
    echo "::error::$name: its SBOM attestation doesn't verify"
    failed=1
  fi
done
say ""

say "### The notes, from CHANGELOG.md" ""
jq -r .body <<< "$release" >> "$summary"

exit "$failed"
