#!/bin/sh
# Publishes the draft release TAG once it's approved: release.yml's publish job, in the `release`
# environment, runs it after its approver has read the draft job's summary. It refuses, saying why,
# when TAG isn't a draft, when the Java edition X.Y.* the native `mutastic X.Y.Z` runs isn't
# published with its CLI (scripts/java-paired.sh, checked again, as a Java release can be withdrawn
# while the approval waits), or when the `release` environment has no required reviewer, so nobody
# approved it. Otherwise it publishes the draft as the latest release, and starts
# release-verify.yml on it: a release published with the workflow's token starts no workflow of its
# own.
#
#   GH_TOKEN=... sh .github/release-publish.sh vX.Y.Z X.Y.Z [--check]
#
# --check runs the checks and stops before publishing.
set -eu

tag=${1:?"usage: release-publish.sh vX.Y.Z X.Y.Z [--check]"}
version=${2:?"usage: release-publish.sh $tag X.Y.Z [--check]"}
mode=${3:-publish}
repo=${GITHUB_REPOSITORY:-mutastic/mutastic-rs}
environment=release
summary=${GITHUB_STEP_SUMMARY:-/dev/null}
said=$(mktemp)
trap 'rm -f "$said"' EXIT

note() {
  printf '%s\n' "$@"
  printf '%s\n' "$@" >> "$summary"
}
refuse() {
  echo "::error::$tag isn't published: $*"
  printf -- '- ✗ %s\n\n**Nothing was published.**\n' "$*" >> "$summary"
  exit 1
}

note "## Publishing mutastic $version" ""

draft=$(gh release view "$tag" --repo "$repo" --json isDraft --jq .isDraft) ||
  refuse "there's no release $tag to publish"
[ "$draft" = true ] || refuse "$tag is already published"
note "- ✓ $tag is a draft"

java=$(sh scripts/java-paired.sh "$version" 2> "$said") || refuse "$(cat "$said")"
note "- ✓ Java $java is published with its CLI, for mutastic $version"

reviewers=$(gh api "repos/$repo/environments/$environment" \
  --jq '[.protection_rules[]? | select(.type == "required_reviewers")] | length' 2> /dev/null) ||
  reviewers=0
[ "$reviewers" -gt 0 ] ||
  refuse "the \`$environment\` environment has no required reviewer, so nobody approved this: add one (Settings, Environments), then re-run the job"
note "- ✓ the \`$environment\` environment has a required reviewer, so this run was approved"

if [ "$mode" = --check ]; then
  note "" "Checked only (--check): $tag is still a draft."
  exit 0
fi

gh release edit "$tag" --repo "$repo" --draft=false --latest > /dev/null
note "- ✓ $tag is published, as the latest release: $(gh release view "$tag" --repo "$repo" --json url --jq .url)"

gh workflow run release-verify.yml --repo "$repo" --ref "$tag" -f version="$version"
note "- ✓ \`release-verify.yml\` started on it: look at it in *Actions*"
