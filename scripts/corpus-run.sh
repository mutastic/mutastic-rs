#!/usr/bin/env bash
# One crate of the corpus (corpus/corpus.yaml), run as the nightly runs it: cloned at its pinned
# commit, its own `cargo test` first, then a share of its mutants in events, then the same share
# for an agent. The fixtures hold the shapes we thought of; the corpus holds the ones people write,
# where the bugs that matter turned up.
#
#   scripts/corpus-run.sh NAME                       run one crate, print its result line
#   scripts/corpus-run.sh --matrix [NAMES]           check corpus.yaml, print its names as JSON
#   scripts/corpus-run.sh --summary DIR PREV NAME…   the results in DIR as a table, against PREV
#
# Corpus crates are other people's code, cloned and built: they run in GitHub Actions only. Outside
# it, this runs only an entry whose git is a file:// URL, a fixture standing in for a crate, from
# a corpus file CORPUS_FILE names.
#
# A crate passes when its expectation holds:
# - score: the run ends with run-finished, a score, at least one mutant and exit 0 (the threshold
#   is 0: a crate's own threshold isn't what's checked), with no internal error and no "tests fail
#   without any mutant"; and the agent run's head has the agent format's shape.
# - refused:<code>: the run ends with run-failed and that code.
# A crate whose own tests fail is "crate-red" and skipped, not failed, unless a refusal is what's
# expected (a red test is often why). Anything else fails, and exits 1. scripts/corpus.py judges.
#
# The result is one JSON line (name, expect, outcome, exit, mutants, score, seconds, codes, reason,
# agent), printed and written to $RUNNER_TEMP/corpus/out/NAME.result.json (else $TMPDIR's), beside
# the runs' events and logs. The clone is in $RUNNER_TEMP/corpus/src/NAME.
#
# CORPUS_LIMIT (600) is each mutastic run's time limit in seconds, CORPUS_TEST_LIMIT (600) the
# crate's own tests'. MUTASTIC_BIN is the mutastic to run, target/release/mutastic by default.
set -u

cd "$(dirname "$0")/.." || exit 2
corpus=${CORPUS_FILE:-corpus/corpus.yaml}
limit=${CORPUS_LIMIT:-600}
test_limit=${CORPUS_TEST_LIMIT:-600}
base="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/corpus"
root=$PWD

# The judgements, in Python: scripts/corpus.py.
corpus_py() {
  python3 -I "$root/scripts/corpus.py" "$@"
}

die() {
  echo "corpus: $1" >&2
  exit 2
}

for tool in yq jq python3 timeout git; do
  command -v "$tool" > /dev/null || die "needs $tool on the PATH"
done
[ -f "$corpus" ] || die "no corpus file $corpus"

# The corpus as JSON, one entry to a line.
entries() {
  yq -o=json -I=0 '.[]' "$corpus"
}

# Clones NAME at its commit, runs its tests and mutastic on a share of it, and prints its result.
run_one() {
  local name=$1 entry
  entries | corpus_py matrix "$name" > /dev/null || exit 2
  entry=$(CORPUS_NAME=$name yq -o=json -I=0 '.[] | select(.name == strenv(CORPUS_NAME))' "$corpus")
  field() { jq -r ".$1" <<< "$entry"; }
  local git commit package shard expect
  git=$(field git) commit=$(field commit) package=$(field package) shard=$(field shard)
  expect=$(field expect)
  if [ "${GITHUB_ACTIONS:-}" != true ] && [[ "$git" != file://* ]]; then
    die "$name is someone else's code: corpus crates run in GitHub Actions only. Here, only an entry whose git is a file:// URL runs."
  fi

  local mutastic=${MUTASTIC_BIN:-target/release/mutastic}
  [ -x "$mutastic" ] || die "no mutastic at $mutastic: just self-build, or set MUTASTIC_BIN"
  mutastic=$(cd "$(dirname "$mutastic")" && pwd)/$(basename "$mutastic")
  local src=$base/src/$name out=$base/out
  rm -rf "$src" && mkdir -p "$src" "$out"
  rm -f "$out/$name".*

  # The commit alone, not the history: GitHub serves any commit by its hash.
  echo "corpus: $name, $git at $commit" >&2
  if ! { git -C "$src" init -q && git -C "$src" fetch -q --depth=1 "$git" "$commit" &&
    git -C "$src" -c advice.detachedHead=false checkout -q FETCH_HEAD; } > "$out/$name.clone.log" 2>&1 ||
    [ "$(git -C "$src" rev-parse HEAD 2> /dev/null)" != "$commit" ]; then
    cat "$out/$name.clone.log" >&2
    die "couldn't fetch $commit from $git"
  fi
  local pkg=$src
  [ "$package" = . ] || pkg=$src/$package
  [ -f "$pkg/Cargo.toml" ] || die "$name: no Cargo.toml in $package at $commit"

  # A crate's rust-toolchain.toml once made rustup reinstall stable mid-run, which broke cargo on
  # the whole machine for a minute. The corpus runs on the stable the job installed.
  export RUSTUP_TOOLCHAIN=stable
  # A crate's misses aren't this repository's: no annotations or job summary from mutastic.
  export MUTASTIC_GITHUB=off

  echo "corpus: $name's own tests…" >&2
  local status red=""
  (cd "$pkg" && timeout -k 30 "$test_limit" cargo test) > "$out/$name.cargo-test.log" 2>&1
  status=$?
  if [ "$status" -ne 0 ] && [ "$expect" = score ]; then
    red="its own cargo test fails (exit $status), so mutastic didn't run"
    [ "$status" -eq 124 ] && red="its own cargo test took over ${test_limit}s, so mutastic didn't run"
    tail -n 20 "$out/$name.cargo-test.log" >&2
  fi

  local seconds=0 began agent_status=0
  : > "$out/$name.events.jsonl"
  : > "$out/$name.run.log"
  : > "$out/$name.agent.txt"
  if [ -z "$red" ]; then
    echo "corpus: $name, share 1/$shard as events…" >&2
    began=$SECONDS
    timeout -k 30 "$limit" "$mutastic" run "$pkg" --shard=1/"$shard" --threshold=0 --format=jsonl \
      > "$out/$name.events.jsonl" 2> "$out/$name.run.log"
    status=$?
    seconds=$((SECONDS - began))
    if [ "$expect" = score ] && grep -q '"reason":"run-finished"' "$out/$name.events.jsonl"; then
      echo "corpus: $name, the same share for an agent…" >&2
      MUTASTIC_AGENT=1 timeout -k 30 "$limit" "$mutastic" run "$pkg" --shard=1/"$shard" --threshold=0 \
        > "$out/$name.agent.txt" 2> "$out/$name.agent.log"
      agent_status=$?
    fi
  fi

  local line verdict
  line=$(corpus_py judge "$name" "$expect" "$red" "$status" "$seconds" "$out/$name.events.jsonl" \
    "$out/$name.run.log" "$out/$name.agent.txt" "$agent_status")
  verdict=$?
  echo "$line" | tee "$out/$name.result.json"
  if [ "$verdict" -ne 0 ]; then
    tail -n 15 "$out/$name.run.log" | sed 's/^/    /' >&2
  fi
  return "$verdict"
}

case ${1:-} in
  --matrix) entries | corpus_py matrix "${2:-}" ;;
  --summary) shift && entries | corpus_py summary "$@" ;;
  "" | -*) die "scripts/corpus-run.sh NAME, --matrix [NAMES] or --summary DIR PREV NAME…" ;;
  *) run_one "$1" ;;
esac
