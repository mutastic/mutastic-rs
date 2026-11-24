#!/bin/sh
# Tests changelog.sh's notes, the body a GitHub release is given, and check --for, the rule that a
# patch has no scoring changes and no release has nothing to say, on a CHANGELOG.md and changes/ of
# its own in a temporary directory. Run it after changing changelog.sh:
# sh scripts/test-changelog.sh

here=$(cd "$(dirname "$0")" && pwd) || exit 2
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
failures=0
cases=0

# A project with changelog.sh, a CHANGELOG.md with two releases, and nothing waiting in changes/.
fresh() {
  rm -rf "$work/project"
  mkdir -p "$work/project/scripts" "$work/project/changes"
  cp "$here/changelog.sh" "$work/project/scripts/"
  cat > "$work/project/CHANGELOG.md" <<'EOF'
# Changes

## Unreleased

What's merged waits in changes/.

## 1.2.0 (20 Nov 2026)

### Scoring changes

- **A new operator.** It finds more.

### Fixes

- **A fix.** It works.
  On two lines.

## 1.1.0 (12 Nov 2026)

### Fixes

- **An older fix.** It works too.
EOF
}

# entry <section> <slug>: a file in changes/, shaped as an entry.
entry() {
  mkdir -p "$work/project/changes/$1"
  printf -- '- **The %s entry.** It says what changed.\n' "$2" > "$work/project/changes/$1/$2.md"
}

# released <heading line> <section heading>: CHANGELOG.md gains a release with one entry.
released() {
  awk -v heading="$1" -v section="$2" '
    !done && /^## [0-9]/ { print heading; print ""; print section; print ""; print "- **An entry.** Text."; print ""; done = 1 }
    { print }
  ' "$work/project/CHANGELOG.md" > "$work/CHANGELOG.md"
  mv "$work/CHANGELOG.md" "$work/project/CHANGELOG.md"
}

# passes <name> <args...>: changelog.sh exits 0.
passes() {
  name=$1
  shift
  cases=$((cases + 1))
  if ! sh "$work/project/scripts/changelog.sh" "$@" > "$work/out" 2> "$work/err"; then
    echo "FAIL $name: exited $?: $(cat "$work/err")"
    failures=$((failures + 1))
  fi
}

# stops <name> <expected error> <args...>: changelog.sh fails, saying so.
stops() {
  name=$1
  expected=$2
  shift 2
  cases=$((cases + 1))
  if sh "$work/project/scripts/changelog.sh" "$@" > "$work/out" 2> "$work/err"; then
    echo "FAIL $name: passed"
    failures=$((failures + 1))
  elif ! grep -qF -- "$expected" "$work/err"; then
    echo "FAIL $name: expected \"$expected\", got: $(cat "$work/err")"
    failures=$((failures + 1))
  fi
}

# prints <name> <expected output file>: the last command printed exactly that.
prints() {
  cases=$((cases + 1))
  if ! diff "$2" "$work/out" > "$work/diff"; then
    echo "FAIL $1: printed something else:"
    sed 's/^/  /' "$work/diff"
    failures=$((failures + 1))
  fi
}

fresh
passes "notes, a release" notes 1.2.0
cat > "$work/expected" <<'EOF'
### Scoring changes

- **A new operator.** It finds more.

### Fixes

- **A fix.** It works.
  On two lines.
EOF
prints "notes, without its heading or the blank lines around it" "$work/expected"
passes "notes, the oldest release" notes 1.1.0
cat > "$work/expected" <<'EOF'
### Fixes

- **An older fix.** It works too.
EOF
prints "notes, up to the end of the file" "$work/expected"
stops "notes, a release there isn't" "has no section for 1.3.0" notes 1.3.0
stops "notes, not a version" "isn't a version" notes v1.2.0
stops "notes, Unreleased" "isn't a version" notes Unreleased
printf '## 1.3.0 (23 Nov 2026)\n\n## 1.2.0 (20 Nov 2026)\n' > "$work/project/CHANGELOG.md"
stops "notes, an empty section" "section for 1.3.0 is empty" notes 1.3.0
printf '## 1.2.10 (23 Nov 2026)\n\n- **A fix.** Text.\n' > "$work/project/CHANGELOG.md"
stops "notes, not a prefix of another" "has no section for 1.2.1" notes 1.2.1

# Before the release commit: the entries waiting in changes/.
fresh
passes "check, nothing waiting" check
stops "a release with nothing waiting" "1.2.1 has no entries" check --for 1.2.1
stops "a minor release with nothing waiting" "1.3.0 has no entries" check --for 1.3.0
entry fixes a-fix
passes "a patch with a fix" check --for 1.2.1
entry score-corrections never-compiled
passes "a patch with a score correction" check --for 1.2.1
entry scoring-changes new-operator
passes "check, a scoring change, no release named" check
stops "a patch with a scoring change" "changes/scoring-changes/new-operator.md is a scoring change: it waits for 1.3.0" check --for 1.2.1
stops "a patch of 1.9 waits for 1.10" "it waits for 1.10.0" check --for 1.9.3
passes "a minor release with a scoring change" check --for 1.3.0
passes "a major release with a scoring change" check --for 2.0.0
stops "check --for, not a version" "isn't a version" check --for 1.2
mkdir -p "$work/project/changes/nowhere"
printf -- '- **Lost.** Text.\n' > "$work/project/changes/nowhere/lost.md"
stops "check --for still checks the files" "isn't in a section" check --for 1.3.0

# After the release commit: its section in CHANGELOG.md.
fresh
released "## 1.2.1 (23 Nov 2026)" "### Fixes"
passes "a patch's section with a fix" check --for 1.2.1
fresh
released "## 1.2.1 (23 Nov 2026)" "### Score corrections"
passes "a patch's section with a score correction" check --for 1.2.1
fresh
released "## 1.2.1 (23 Nov 2026)" "### Scoring changes"
stops "a patch's section with scoring changes" "1.2.1 is a patch, and its section in CHANGELOG.md has Scoring changes" check --for 1.2.1
passes "a minor's section with scoring changes" check --for 1.2.0
fresh
printf '## 1.2.1 (23 Nov 2026)\n\nNothing yet.\n\n' > "$work/project/CHANGELOG.new"
cat "$work/project/CHANGELOG.md" >> "$work/project/CHANGELOG.new"
mv "$work/project/CHANGELOG.new" "$work/project/CHANGELOG.md"
stops "a section with no entries" "section for 1.2.1 has no entries" check --for 1.2.1

echo "test-changelog: $cases cases, $failures failed"
[ "$failures" -eq 0 ]
