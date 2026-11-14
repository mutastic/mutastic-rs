#!/bin/sh
# Vendors the HTML report's page at a mutastic commit: report-ui/report.html, built there by
# report-ui/build.mjs, into crates/mutastic/src/report/page/, and pins that commit and the page's
# sha256 in report-page.json. The Java edition embeds the same page; the tests hold the copy to the
# pin, and the pin to mutastic's own checksum at that commit.
#
#   scripts/pin-report-page.sh <commit>
set -eu
commit=${1:?usage: scripts/pin-report-page.sh <commit>}
cd "$(dirname "$0")/.."
raw=https://raw.githubusercontent.com/mutastic/mutastic
page=crates/mutastic/src/report/page/report.html
curl --fail --silent --show-error --location "$raw/$commit/report-ui/report.html" -o "$page"
sum=$(curl --fail --silent --show-error --location "$raw/$commit/report-ui/report.html.sha256" | cut -d' ' -f1)
actual=$( (sha256sum "$page" 2>/dev/null || shasum -a 256 "$page") | cut -d' ' -f1)
if [ "$sum" != "$actual" ]; then
  echo "pin-report-page: the page at $commit has sha256 $actual, but its checksum file says $sum" >&2
  exit 1
fi
cat > crates/mutastic/src/report/page/report-page.json <<JSON
{
  "repository": "https://github.com/mutastic/mutastic",
  "raw": "$raw",
  "path": "report-ui/report.html",
  "commit": "$commit",
  "sha256": "$sum"
}
JSON
echo "pinned report-ui/report.html at $commit ($sum)"
