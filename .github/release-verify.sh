#!/usr/bin/env bash
# A published release as a person gets it: release-verify.yml installs it the ways a person does
# (install.sh, install.ps1, Scoop) on Linux, macOS and Windows (Git Bash), then checks the
# installed command with these steps:
#
#   .github/release-verify.sh version X.Y.Z BIN              --version: this release, no Java yet
#   .github/release-verify.sh crate X.Y.Z BIN                a run on fixtures/pricing
#   .github/release-verify.sh java X.Y.Z BIN JAVA JAVA_DIR   the Java edition JAVA, fetched and
#                                                            run on JAVA_DIR's samples/pricing
#
# BIN is the folder the installer put `mutastic` in; MUTASTIC_HOME, where it keeps releases, should
# be a new one, so the Java edition is fetched as on a machine that never had it. JAVA_DIR is a
# checkout of the Java edition's tag vJAVA: its samples/pricing, and the score line its
# PricingSampleTest expects. Each step says what it found in $GITHUB_STEP_SUMMARY when there is
# one, and a failure says what failed and what to do.
set -u

step=${1:?"usage: release-verify.sh version|crate|java X.Y.Z BIN [JAVA JAVA_DIR]"}
version=${2:?"usage: release-verify.sh $step X.Y.Z BIN"}
bin=${3:?"usage: release-verify.sh $step $version BIN"}
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "release-verify: $version isn't X.Y.Z" >&2; exit 2; }

work="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/release-verify"
mkdir -p "$work"
summary=${GITHUB_STEP_SUMMARY:-/dev/null}
os=${RUNNER_OS:-$(uname -s)}
mutastic=$bin/mutastic
[ -x "$mutastic" ] || mutastic=$bin/mutastic.exe
[ -x "$mutastic" ] || { echo "release-verify: no mutastic in $bin" >&2; exit 2; }

# A path as a Windows program reads it: C:/Users/..., not Git Bash's /c/Users/....
native() {
  if command -v cygpath > /dev/null; then cygpath -m "$1"; else printf '%s\n' "$1"; fi
}

note() {
  printf '%s\n' "$*"
  printf '%s\n' "$*" >> "$summary"
}

# A failed step: what failed, the end of its log, and what to do, then exit 1.
failed() {
  local what=$1 log=${2:-}
  echo "::error::mutastic $version on $os: $what"
  {
    printf '### ✗ mutastic %s on %s: %s\n\n' "$version" "$os" "$what"
    if [ -n "$log" ] && [ -f "$log" ]; then
      printf '<details><summary>The end of its log</summary>\n\n```\n'
      tail -n 40 "$log"
      printf '```\n</details>\n\n'
    fi
    printf 'Anyone who installed it has it: fix forward. Card it on the board; withdraw the release\n'
    printf 'if it can'"'"'t be used (RELEASING.md, *Withdrawing a release*); fix the cause and release\n'
    printf 'the next patch; run release-verify on that.\n'
  } >> "$summary"
  [ -n "$log" ] && [ -f "$log" ] && tail -n 40 "$log"
  exit 1
}

# `mutastic --version`, its lines without Windows's carriage returns.
said_version() {
  "$mutastic" --version 2>&1 | tr -d '\r'
}

check_version() {
  local said
  said=$(said_version)
  grep -qxF "mutastic $version" <<< "$said" ||
    failed "\`mutastic --version\` doesn't say \`mutastic $version\`: $said"
  grep -qxF "Rust edition $version, built in" <<< "$said" ||
    failed "\`mutastic --version\` doesn't say \`Rust edition $version, built in\`: $said"
  grep -q '^Java edition: not fetched yet' <<< "$said" ||
    failed "\`mutastic --version\` says a Java edition is there before one was fetched: $said"
  note "- ✓ \`mutastic --version\`: mutastic $version, its Rust edition built in, no Java edition yet"
}

# A copy of a project, so its reports are this run's.
copy() {
  rm -rf "${work:?}/$2"
  cp -R "$1" "$work/$2"
  printf '%s\n' "$work/$2"
}

check_crate() {
  local project log=$work/crate.log
  project=$(copy fixtures/pricing crate-pricing)
  "$mutastic" run "$(native "$project")" --format=agent > "$log" 2>&1 ||
    failed "\`mutastic run\` on fixtures/pricing exited $?" "$log"
  local first
  first=$(tr -d '\r' < "$log" | grep -m 1 '^mutastic: ')
  [[ "$first" == "mutastic: PASS · score "* ]] ||
    failed "\`mutastic run\` on fixtures/pricing didn't pass: \`$first\`" "$log"
  for report in mutation.json mutation.html; do
    [ -s "$project/target/mutastic/$report" ] ||
      failed "\`mutastic run\` on fixtures/pricing wrote no $report" "$log"
  done
  note "- ✓ \`mutastic run\` on fixtures/pricing: \`$first\`, with its reports"
}

check_java() {
  local java=${4:?"usage: release-verify.sh java $version BIN JAVA JAVA_DIR"}
  local java_dir=${5:?"usage: release-verify.sh java $version BIN $java JAVA_DIR"}
  local expected
  expected=$(grep -o '"mutastic: [A-Z]* · score [^"]*"' \
    "$java_dir/mutastic-cli/src/test/java/io/mutastic/cli/PricingSampleTest.java" | head -n 1 | tr -d '"')
  [ -n "$expected" ] || failed "Java $java's PricingSampleTest names no score line to expect"
  local project log=$work/java.log
  project=$(copy "$java_dir/samples/pricing" java-pricing)
  MUTASTIC_VERSION=$java "$mutastic" run "$(native "$project")" --format=agent > "$log" 2>&1 ||
    failed "\`mutastic run\` on the Java edition's samples/pricing exited $?" "$log"
  grep -qxF "$expected" < <(tr -d '\r' < "$log") ||
    failed "\`mutastic run\` on the Java edition's samples/pricing didn't print \`$expected\`" "$log"
  for report in mutation.json mutation.html; do
    [ -s "$project/build/mutastic/$report" ] ||
      failed "\`mutastic run\` on the Java edition's samples/pricing wrote no $report" "$log"
  done
  note "- ✓ \`mutastic run\` fetched Java $java and ran it on samples/pricing: \`$expected\`, with its reports"
  local said
  said=$(said_version)
  grep -q "^Java edition $java, at " <<< "$said" ||
    failed "\`mutastic --version\` doesn't name the Java edition it fetched, $java: $said"
  note "- ✓ \`mutastic --version\` reports both editions: Rust $version, Java $java"
}

case $step in
  version) check_version ;;
  crate) check_crate ;;
  java) check_java "$@" ;;
  *) echo "release-verify: no step $step: version, crate or java" >&2; exit 2 ;;
esac
