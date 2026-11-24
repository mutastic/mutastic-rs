#!/bin/sh
# The changelog's next release waits in changes/, one file per entry, so two pull requests never
# edit the same lines of CHANGELOG.md. A release gathers them into its section:
#
#   scripts/changelog.sh preview [X.Y.Z [date]]   prints the section, writes nothing
#   scripts/changelog.sh release X.Y.Z [date]     writes it into CHANGELOG.md, deletes the files
#   scripts/changelog.sh notes X.Y.Z              prints its section's body, for its GitHub release
#   scripts/changelog.sh check [--for X.Y.Z] [base head [subject]]
#
# An entry is changes/<section>/<slug>.md, holding the entry as CHANGELOG.md shows it: "- **The
# lead sentence.**", then its paragraph, each line after the first indented two spaces. Sections
# come in the order below; in a section, entries come in the order they reached main, as when
# each was added to the end, then by name. A section's _intro.md, if it has one, is a paragraph
# above its entries. The date is today's unless given.
#
# check fails on a file in changes/ outside a known section, or not shaped as an entry. Given a
# range, it fails too if a commit in it changes CHANGELOG.md and isn't a release: a subject with
# the release scope, "chore(release): version X.Y.Z". With a subject (a pull request's title),
# the range is one change and the subject is its own.
#
# With --for X.Y.Z, check also fails when that release has nothing to say, or is a patch (Z isn't
# 0) with scoring changes, which wait for a minor release; score corrections are fine in a patch.
# Before the release commit it reads the entries waiting in changes/; after, the section in
# CHANGELOG.md. notes fails when CHANGELOG.md has no section for X.Y.Z, or an empty one, so no
# release goes out without its notes.

set -eu
cd "$(dirname "$0")/.."

date_format='%Y-%m-%d'

# <directory>|<heading>, in the order they appear in a release.
sections='breaking-changes|Breaking changes
card-changes|Card changes
scoring-changes|Scoring changes
score-corrections|Score corrections
security|Security
additions|Additions
changes|Changes
fixes|Fixes
internal|Internal'

usage() {
  sed -n '5,8s/^# *//p' "$0" >&2
  exit 2
}

# The entries of one section, in order: the time each was first committed (now, if it isn't
# yet), then its name.
entries() {
  [ -d "changes/$1" ] || return 0
  now=$(date +%s)
  for file in "changes/$1"/*.md; do
    [ -f "$file" ] && [ "$file" != "changes/$1/_intro.md" ] || continue
    added=$(git log --diff-filter=A --format=%ct -- "$file" 2>/dev/null | tail -n 1)
    printf '%s %s\n' "${added:-$now}" "$file"
  done | sort -k1,1n -k2,2 | cut -d' ' -f2-
}

# A file's text as it is, ending in one newline.
trim() {
  awk '{ lines[NR] = $0 } END { n = NR; while (n > 0 && lines[n] == "") n--; for (i = 1; i <= n; i++) print lines[i] }' "$1"
}

section() {
  heading=$1
  first=yes
  printf '%s\n' "$sections" | while IFS='|' read -r dir title; do
    files=$(entries "$dir")
    [ -n "$files" ] || continue
    printf '\n### %s\n\n' "$title"
    if [ -f "changes/$dir/_intro.md" ]; then
      trim "changes/$dir/_intro.md"
      echo
    fi
    printf '%s\n' "$files" | while read -r file; do
      trim "$file"
    done
  done > "$tmp/body"
  if [ ! -s "$tmp/body" ]; then
    echo "changelog: changes/ has no entries" >&2
    exit 1
  fi
  printf '%s\n' "$heading"
  cat "$tmp/body"
}

today() {
  LC_ALL=C date +"$date_format"
}

check_files() {
  failed=0
  known=$(printf '%s\n' "$sections" | cut -d'|' -f1)
  for file in $(find changes -type f 2>/dev/null | sort); do
    [ "$file" = changes/README.md ] && continue
    dir=$(printf '%s' "$file" | cut -d/ -f2)
    name=$(printf '%s' "$file" | cut -d/ -f3-)
    if ! printf '%s\n' "$known" | grep -qx "$dir" || [ "changes/$dir/$name" != "$file" ]; then
      echo "changelog: $file isn't in a section: changes/<section>/<slug>.md, a section one of" \
        $known >&2
      failed=1
      continue
    fi
    if [ "$name" = _intro.md ]; then
      if ! entries "$dir" | grep -q .; then
        echo "changelog: $file introduces a section with no entries" >&2
        failed=1
      fi
      continue
    fi
    case $name in
      *[!a-z0-9.-]* | .* | *.md.md) ok=no ;;
      *.md) ok=yes ;;
      *) ok=no ;;
    esac
    if [ $ok = no ]; then
      echo "changelog: $file: name it <slug>.md, in lower case letters, digits and hyphens" >&2
      failed=1
      continue
    fi
    if ! head -n 1 "$file" | grep -q '^- \*\*'; then
      echo "changelog: $file: an entry starts \"- **\", its lead sentence in bold" >&2
      failed=1
    fi
    if awk 'NR > 1 && $0 != "" && !/^  /' "$file" | grep -q .; then
      echo "changelog: $file: each line after the first is indented two spaces" >&2
      failed=1
    fi
  done
  return $failed
}

# The body of X.Y.Z's section in CHANGELOG.md, its heading and the blank lines around it left
# out, into $tmp/notes. Fails when there's no such section.
release_notes() {
  awk -v heading="## $1 " '
    index($0, heading) == 1 { on = 1; found = 1; next }
    on && /^## / { exit }
    on { lines[++n] = $0 }
    END {
      first = 1
      while (first <= n && lines[first] == "") first++
      while (n >= first && lines[n] == "") n--
      for (i = first; i <= n; i++) print lines[i]
      exit !found
    }
  ' CHANGELOG.md > "$tmp/notes"
}

version_shape() {
  if ! printf '%s\n' "$1" | grep -Eqx '[0-9]+\.[0-9]+\.[0-9]+'; then
    echo "changelog: '$1' isn't a version, X.Y.Z" >&2
    exit 2
  fi
}

# Fails if X.Y.Z has no entries, or is a patch with scoring changes.
check_release() {
  version=$1
  patch=${version##*.}
  minor=${version%.*}
  next="${minor%%.*}.$((${minor#*.} + 1)).0"
  if release_notes "$version"; then
    if ! grep -q '^- \*\*' "$tmp/notes"; then
      echo "changelog: CHANGELOG.md's section for $version has no entries" >&2
      return 1
    fi
    if [ "$patch" != 0 ] && grep -qx '### Scoring changes' "$tmp/notes"; then
      echo "changelog: $version is a patch, and its section in CHANGELOG.md has Scoring changes:" \
        "they wait for $next (a correction that can only raise a score is a Score correction)" >&2
      return 1
    fi
    return 0
  fi
  printf '%s\n' "$sections" | while IFS='|' read -r dir title; do
    entries "$dir"
  done > "$tmp/waiting"
  if [ ! -s "$tmp/waiting" ]; then
    echo "changelog: $version has no entries: changes/ is empty, and CHANGELOG.md has no section" \
      "for it" >&2
    return 1
  fi
  if [ "$patch" != 0 ] && grep -q '^changes/scoring-changes/' "$tmp/waiting"; then
    for file in $(grep '^changes/scoring-changes/' "$tmp/waiting"); do
      echo "changelog: $version is a patch, and $file is a scoring change: it waits for $next" \
        "(a correction that can only raise a score goes in changes/score-corrections/)" >&2
    done
    return 1
  fi
  return 0
}

# Fails if a change that isn't a release edits CHANGELOG.md.
check_range() {
  base=$1 head=$2 subject=${3:-}
  case $base in '' | *[!0]*) ;; *) base= ;; esac
  [ -n "$base" ] || return 0
  release='^[a-z]+\(release\)!?: '
  if [ -n "$subject" ]; then
    if git diff --name-only "$base...$head" -- CHANGELOG.md | grep -q . &&
      ! printf '%s\n' "$subject" | grep -Eq "$release"; then
      echo "changelog: \"$subject\" edits CHANGELOG.md. Add the entry as a file in changes/ instead" >&2
      echo "changelog: (changes/README.md); only a release, \"chore(release): ...\", edits CHANGELOG.md" >&2
      return 1
    fi
    return 0
  fi
  failed=0
  for commit in $(git rev-list --no-merges "$base..$head" -- CHANGELOG.md); do
    subject=$(git log -1 --format=%s "$commit")
    if ! printf '%s\n' "$subject" | grep -Eq "$release"; then
      echo "changelog: $(git rev-parse --short "$commit") \"$subject\" edits CHANGELOG.md, and isn't a release" >&2
      failed=1
    fi
  done
  return $failed
}

[ "$#" -ge 1 ] || usage
command=$1
shift
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

case $command in
  preview)
    [ "$#" -le 2 ] || usage
    if [ "$#" -ge 1 ]; then
      section "## $1 (${2:-$(today)})"
    else
      section "## Unreleased"
    fi
    ;;
  release)
    [ "$#" -ge 1 ] && [ "$#" -le 2 ] || usage
    check_files
    if grep -q "^## $1 " CHANGELOG.md; then
      echo "changelog: CHANGELOG.md already has $1" >&2
      exit 1
    fi
    section "## $1 (${2:-$(today)})" > "$tmp/section"
    # The new section goes above the newest release, below Unreleased's pointer to changes/.
    awk -v section="$tmp/section" '
      !done && /^## / && !/^## Unreleased/ {
        while ((getline line < section) > 0) print line
        print ""
        done = 1
      }
      { print }
      END { if (!done) { while ((getline line < section) > 0) print line } }
    ' CHANGELOG.md > "$tmp/CHANGELOG.md"
    cat "$tmp/CHANGELOG.md" > CHANGELOG.md
    printf '%s\n' "$sections" | while IFS='|' read -r dir title; do
      entries "$dir"
    done > "$tmp/files"
    xargs rm -f < "$tmp/files"
    rm -f changes/*/_intro.md
    find changes -mindepth 1 -type d -empty -delete
    echo "changelog: wrote $1 into CHANGELOG.md from $(wc -l < "$tmp/files" | tr -d ' ') entries in changes/"
    ;;
  notes)
    [ "$#" -eq 1 ] || usage
    version_shape "$1"
    if ! release_notes "$1"; then
      echo "changelog: CHANGELOG.md has no section for $1, \"## $1 (<date>)\"" >&2
      exit 1
    fi
    if [ ! -s "$tmp/notes" ]; then
      echo "changelog: CHANGELOG.md's section for $1 is empty" >&2
      exit 1
    fi
    cat "$tmp/notes"
    ;;
  check)
    for_version=
    if [ "${1:-}" = --for ]; then
      [ "$#" -ge 2 ] || usage
      version_shape "$2"
      for_version=$2
      shift 2
    fi
    [ "$#" -eq 0 ] || [ "$#" -eq 2 ] || [ "$#" -eq 3 ] || usage
    status=0
    check_files || status=1
    if [ -n "$for_version" ]; then
      check_release "$for_version" || status=1
    fi
    if [ "$#" -ge 2 ]; then
      check_range "$@" || status=1
    fi
    exit $status
    ;;
  *) usage ;;
esac
