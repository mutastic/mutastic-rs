#!/usr/bin/env bash
# Every asset of a published GitHub release, downloaded and held to GitHub's own record of it: the
# digest GitHub took as it was uploaded, the release's SHA256SUMS, and, for an immutable release,
# GitHub's signed attestation (`gh release verify-asset`), and release.yml's attestation of where it
# was built (`gh attestation verify`). A SHA256SUMS made from the same files can't catch a file
# swapped along with it; the digests and attestations can. release-verify.yml
# runs it once a release is out.
#
#   GH_TOKEN=... .github/release-assets.sh vX.Y.Z [owner/repo]
set -u

tag=${1:?"usage: release-assets.sh vX.Y.Z [owner/repo]"}
repo=${2:-${GITHUB_REPOSITORY:?"name the repository: release-assets.sh $tag owner/repo"}}
summary=${GITHUB_STEP_SUMMARY:-/dev/null}
dir=$(mktemp -d)
failed=0

note() {
  printf '%s\n' "$*"
  printf '%s\n' "$*" >> "$summary"
}
fail() {
  echo "::error::$tag: $*"
  printf -- '- ✗ %s\n' "$*" >> "$summary"
  failed=1
}
sha256() {
  # From stdin: given a path with a backslash, sha256sum escapes its line with one.
  if command -v sha256sum > /dev/null; then sha256sum < "$1"; else shasum -a 256 < "$1"; fi | cut -d' ' -f1
}

note "### The assets of $repo $tag"
gh release download "$tag" --repo "$repo" --dir "$dir" ||
  { fail "its assets couldn't be downloaded"; exit 1; }

# GitHub's digest of each asset, `sha256:<hex>`, taken as it was uploaded.
while read -r digest name; do
  [ -n "$name" ] || continue
  actual=$(sha256 "$dir/$name")
  if [ "$digest" = "sha256:$actual" ]; then
    note "- ✓ $name is as GitHub received it ($actual)"
  else
    fail "$name is $actual, but GitHub took it as ${digest:-no digest}"
  fi
done < <(gh release view "$tag" --repo "$repo" --json assets --jq '.assets[] | "\(.digest // "") \(.name)"')

if [ -f "$dir/SHA256SUMS" ]; then
  while read -r listed name; do
    name=${name#\*}
    [ -n "$name" ] || continue
    if [ ! -f "$dir/$name" ]; then
      fail "SHA256SUMS lists $name, which the release doesn't have"
    elif [ "$(sha256 "$dir/$name")" != "$listed" ]; then
      fail "$name doesn't match SHA256SUMS"
    fi
  done < "$dir/SHA256SUMS"
  [ "$failed" -eq 0 ] && note "- ✓ every file SHA256SUMS lists matches it"
else
  fail "the release has no SHA256SUMS"
fi

# Attestations exist only for an immutable release, and only a gh that has verify-asset checks them.
immutable=$(gh release view "$tag" --repo "$repo" --json isImmutable --jq .isImmutable 2> /dev/null)
if ! gh release verify-asset --help > /dev/null 2>&1; then
  note "- ○ \`gh release verify-asset\` skipped: this runner's gh ($(gh --version | head -n 1)) doesn't have it"
elif [ "$immutable" != true ]; then
  note "- ○ \`gh release verify-asset\` skipped: $tag isn't an immutable release, so GitHub's release attestation covers none of its assets"
else
  gh release verify "$tag" --repo "$repo" > /dev/null || fail "\`gh release verify $tag\` failed"
  for file in "$dir"/*; do
    name=$(basename "$file")
    if gh release verify-asset "$tag" "$file" --repo "$repo" > /dev/null; then
      note "- ✓ $name: \`gh release verify-asset\` passes"
    else
      fail "$name: \`gh release verify-asset\` fails"
    fi
  done
fi

# Where each asset was built: release.yml attests every asset at the tag's commit (actions/attest),
# and the SBOM as what each archive is made of. A release whose release.yml attests nothing, from
# before it did, is skipped; one whose release.yml does must have them all.
signer="$repo/.github/workflows/release.yml"
if ! workflow=$(gh api "repos/$repo/contents/.github/workflows/release.yml?ref=$tag" \
  -H 'Accept: application/vnd.github.raw'); then
  fail "its release.yml couldn't be read, to say whether it attests"
elif ! grep -q 'uses: actions/attest@' <<< "$workflow"; then
  note "- ○ \`gh attestation verify\` skipped: $tag's release.yml attests nothing"
else
  for file in "$dir"/*; do
    name=$(basename "$file")
    if gh attestation verify "$file" --repo "$repo" --signer-workflow "$signer" \
      --source-ref "refs/tags/$tag" > /dev/null; then
      note "- ✓ $name: \`gh attestation verify\` passes, built by release.yml at $tag"
    else
      fail "$name: \`gh attestation verify\` fails"
    fi
  done
  for file in "$dir"/*.tar.gz "$dir"/*.zip; do
    [ -f "$file" ] || continue
    name=$(basename "$file")
    if gh attestation verify "$file" --repo "$repo" --signer-workflow "$signer" \
      --source-ref "refs/tags/$tag" --predicate-type https://cyclonedx.org/bom > /dev/null; then
      note "- ✓ $name: its SBOM is attested"
    else
      fail "$name: its SBOM attestation doesn't verify"
    fi
  done
fi

rm -rf "$dir"
exit "$failed"
