#!/usr/bin/env bash
# The release candidate, run before it's tagged: the native mutastic built from this tree, run over
# a share of every fixture's mutants, each from a copy of its own. A run must finish with its reports. It fails
# the candidate when it crashes (`internal`, or no last event), when it can't run (`run-failed`,
# whatever its code), or when the tests fail without any mutant: a release that does that on the
# fixtures would do it on a user's code. The Java edition's 1.3.0 shipped one (`duplicate element`),
# as nothing ran the release on a project before it shipped.
#
#   scripts/candidate-run.sh [--of=N] [fixture ...]
#
# Each run is the first of N shares of its fixture's mutants (--shard=1/N, dealt by id), 4 unless
# set, which keeps all of them to a few minutes: what it looks for is a run that breaks, not a
# score. Without a fixture named, it runs every directory in fixtures/. The runs' events and logs
# are kept in $RUNNER_TEMP/candidate-run/ (else $TMPDIR's), outside this workspace, which Cargo
# would take the copies for part of, with the copy of a fixture whose run broke (a passing one's
# copy, and its build, are deleted); the summary goes to $GITHUB_STEP_SUMMARY too, in CI.
set -u

shares=4
named=()
for arg in "$@"; do
  case $arg in
    --of=*) shares=${arg#--of=} ;;
    -*) echo "candidate-run: no option $arg; --of=N" >&2; exit 2 ;;
    *) named+=("${arg%/}") ;;
  esac
done
[[ "$shares" =~ ^[1-9][0-9]*$ ]] || { echo "candidate-run: --of takes a count, not $shares" >&2; exit 2; }

cd "$(dirname "$0")/.."
work="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/candidate-run"
rm -rf "$work" && mkdir -p "$work/fixtures"
summary=${GITHUB_STEP_SUMMARY:-/dev/null}

echo "Building mutastic from the tree…"
cargo build --release --locked -p mutastic || { echo "candidate-run: mutastic didn't build" >&2; exit 1; }
mutastic=$PWD/target/release/mutastic

# A workspace's root isn't one package, so its packages run one at a time.
packages() {
  case $1 in
    workspace) echo "billing shop" ;;
    *) echo "" ;;
  esac
}

# Fixtures built so their tests fail on their own, with no mutant in: there, a run that says so is
# the run working. Anywhere else it fails the candidate.
fails_on_its_own() {
  case $1 in
    # Tests that pass in the suite, then fail on the unmutated code when run again alone.
    false-catches) return 0 ;;
    # A test that fails now and then.
    flaky) return 0 ;;
    *) return 1 ;;
  esac
}

if [ ${#named[@]} -eq 0 ]; then
  for dir in fixtures/*/; do named+=("$(basename "$dir")"); done
fi

# The last event of a run: `run-finished`, or `run-failed` and its code; `none` if it wrote none.
last_event() {
  python3 -c '
import json, sys
last = None
for line in open(sys.argv[1], encoding="utf-8"):
    line = line.strip()
    if line.startswith("{"):
        try:
            last = json.loads(line)
        except ValueError:
            pass
if last is None or last.get("reason") not in ("run-finished", "run-failed"):
    print("none")
elif last["reason"] == "run-failed":
    print("run-failed", last.get("code", "other"), last.get("message", "").splitlines()[0] if last.get("message") else "")
else:
    count = last.get("mutants", "?")
    print("run-finished", count, "mutant" if count == 1 else "mutants")
' "$1"
}

failures=0
started=$(date +%s)
{
  echo "## Candidate run: every fixture, share 1 of $shares"
  echo
  echo "| Run | Took | Result |"
  echo "| --- | ---: | --- |"
} >> "$summary"

run_one() {
  local sample=$1 package=$2
  local name=$sample${package:+/$package} slug=$sample${package:+-$package}
  local copy=$work/fixtures/$sample
  [ -d "$copy" ] || cp -R "fixtures/$sample" "$copy"
  local began=$SECONDS status verdict
  touch "$work/$slug.started"
  "$mutastic" run "$copy${package:+/$package}" --shard=1/"$shares" --threshold=0 --format=jsonl \
    > "$work/$slug.jsonl" 2> "$work/$slug.log"
  status=$?
  local took=$((SECONDS - began))s
  local last
  last=$(last_event "$work/$slug.jsonl")
  local own=""
  grep -q 'without any mutant' "$work/$slug.log" "$work/$slug.jsonl" && own=yes
  if [ "$status" -eq 4 ] || [[ "$last" == "run-failed internal"* ]]; then
    verdict="✗ crashed (internal): ${last#run-failed internal }"
  elif [ "$last" = none ] || [ "$status" -eq 101 ]; then
    verdict="✗ ended without a last event (exit $status): it crashed or was stopped"
  elif [[ "$last" == run-failed* ]]; then
    if [ -n "$own" ] && fails_on_its_own "$sample"; then
      verdict="✓ the tests fail without any mutant, as this fixture is built to"
    else
      verdict="✗ $last (exit $status)"
    fi
  elif [ "$status" -eq 3 ] || [ -n "$own" ]; then
    if fails_on_its_own "$sample"; then
      verdict="✓ ${last#run-finished }; tests fail without any mutant, as this fixture is built to"
    else
      verdict="✗ the tests fail without any mutant (exit $status)"
    fi
  elif [ "$status" -ne 0 ]; then
    verdict="✗ ${last} but exited $status"
  elif [ -z "$(find "$copy" -name mutation.json -size +0 -newer "$work/$slug.started")" ]; then
    verdict="✗ finished with no report"
  else
    verdict="✓ ${last#run-finished }"
  fi
  printf '%-36s %6s  %s\n' "$name" "$took" "$verdict"
  printf '| %s | %s | %s |\n' "$name" "$took" "$verdict" >> "$summary"
  if [[ "$verdict" == ✗* ]]; then
    failures=$((failures + 1))
    tail -n 15 "$work/$slug.log" | sed 's/^/    /'
  elif [ -z "$package" ]; then
    rm -rf "${copy:?}"
  fi
}

for sample in "${named[@]}"; do
  [ -d "fixtures/$sample" ] || { echo "candidate-run: no fixture $sample" >&2; exit 2; }
  list=$(packages "$sample")
  if [ -z "$list" ]; then
    run_one "$sample" ""
  else
    for package in $list; do run_one "$sample" "$package"; done
  fi
done

took=$(($(date +%s) - started))
if [ "$failures" -gt 0 ]; then
  echo "candidate-run: $failures runs broke, in ${took}s: don't tag this. Their events and logs are in $work/." >&2
  printf '\n**✗ %s runs broke: don'"'"'t tag this.** Each run'"'"'s events and log are in the artifact.\n' "$failures" >> "$summary"
  exit 1
fi
echo "candidate-run: every run finished, in ${took}s."
printf '\n✓ Every run finished, in %ss.\n' "$took" >> "$summary"
