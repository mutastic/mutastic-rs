"""The corpus's judgements, for scripts/corpus-run.sh: which entries to run, what a run's output
says, and the night's table. The shell script runs things; this reads what they left.

    corpus.py matrix ONLY < entries        check the entries, print the names ONLY picks as JSON
    corpus.py judge NAME EXPECT RED STATUS SECONDS EVENTS LOG AGENT AGENT_STATUS
    corpus.py summary DIR PREV NAME… < entries

The entries are corpus.yaml as JSON, one to a line (yq -o=json -I=0 '.[]'). Python 3.9 or later.
"""

import json
import os
import re
import sys

# The exit statuses `timeout` gives a run it stopped: TERM, then KILL after the grace.
STOPPED = (124, 137)


def read_entries():
    return [json.loads(line) for line in sys.stdin if line.strip()]


def matrix(only):
    """Every entry checked, then the names `only` picks (all of them when it's empty), as a JSON
    array for a matrix."""
    picked = only.replace(",", " ").split()
    entries = read_entries()
    problems = []
    names = []
    for index, entry in enumerate(entries, 1):
        name = entry.get("name")
        where = f"entry {index} ({name or 'no name'})"
        if not isinstance(name, str) or not re.fullmatch(r"[A-Za-z0-9_.-]+", name):
            problems.append(f"{where}: name is one word of letters, digits, _, . or -")
        elif name in names:
            problems.append(f"{where}: another entry is called {name}")
        names.append(name)
        for key in ("git", "package"):
            if not isinstance(entry.get(key), str) or not entry[key]:
                problems.append(f"{where}: no {key}")
        if not re.fullmatch(r"[0-9a-f]{40}", str(entry.get("commit", ""))):
            problems.append(f"{where}: commit is a whole hash, 40 hex digits")
        shard = entry.get("shard")
        if not isinstance(shard, int) or not 1 <= shard <= 64:
            problems.append(f"{where}: shard is N for --shard=1/N, 1 to 64")
        expect = str(entry.get("expect", ""))
        if not re.fullmatch(r"score|refused:[a-z-]+", expect):
            problems.append(f"{where}: expect is score or refused:<code>")
        elif expect.startswith("refused:") and not entry.get("note"):
            problems.append(f"{where}: a refusal needs a note saying why it's right")
        if not entry.get("shapes"):
            problems.append(f"{where}: shapes says what it covers that the others don't")
    unknown = [name for name in picked if name not in names]
    if unknown:
        problems.append("no entry called " + ", ".join(unknown))
    if problems:
        print("\n".join("corpus: " + problem for problem in problems), file=sys.stderr)
        return 2
    print(json.dumps([name for name in names if not picked or name in picked]))
    return 0


def first_line(event):
    message = event.get("message") or ""
    return message.splitlines()[0] if message else ""


def read(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as file:
            return file.read()
    except OSError:
        return ""


def last_event(text):
    """The run's last run-finished or run-failed, and every run-failed code on the way."""
    last = None
    codes = set()
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if event.get("reason") == "run-failed":
            codes.add(event.get("code", "other"))
        if event.get("reason") in ("run-finished", "run-failed"):
            last = event
    return last, codes


def agent_shape(path, status, finished):
    """What's wrong with the agent run's head, or "ok". The verdict first; then reports: and the
    other lines that come before the rules; rules:, decline:, maybe tasks: and the unsafe line;
    then one fence with matching tokens, or nothing, when there's nothing of the project's to
    quote."""
    if status in STOPPED:
        return "stopped at the time limit"
    lines = read(path).rstrip("\n").split("\n")
    if not re.match(r"mutastic: (PASS|FAIL)\b", lines[0]):
        return "line 1 isn't the verdict: " + lines[0][:80]
    rules = next((at for at, line in enumerate(lines) if line.startswith("rules: ")), None)
    if rules is None:
        return "no rules: line"
    head = ("reports: ", "vs last run: ", "likely flaky: ", "note: ")
    for line in lines[1:rules]:
        if not line.startswith(head):
            return "before rules:, a line that isn't the head's: " + line[:80]
    if not any(line.startswith("reports: ") for line in lines[1:rules]):
        return "no reports: line"
    if rules + 1 >= len(lines) or not lines[rules + 1].startswith("decline: "):
        return "decline: doesn't follow rules:"
    rest = lines[rules + 2 :]
    for label in ("tasks: ", "not mutated: "):
        if rest and rest[0].startswith(label):
            rest = rest[1:]
    missed = (finished.get("survived") or 0) + (finished.get("noCoverage") or 0)
    if not rest:
        return "no fence, with mutants missed" if missed else "ok"
    opening = re.match(r"<<mutastic-data (?P<token>[0-9a-f]{8})>> ", rest[0])
    if not opening:
        return "after the head, a line outside the fence: " + rest[0][:80]
    if rest[-1] != f"<<end mutastic-data {opening.group('token')}>>":
        return "the fence doesn't close with its token on the last line"
    markers = [line for line in lines if re.match(r"<<(end )?mutastic-data ", line)]
    if len(markers) != 2:
        return f"{len(markers)} fence markers, not one fence"
    return "ok"


def judge(name, expect, red, status, seconds, events, log, agent, agent_status):
    """The result line, and whether it failed."""
    result = {
        "name": name,
        "expect": expect,
        "outcome": "fail",
        "exit": None,
        "mutants": None,
        "score": None,
        "seconds": None,
        "codes": [],
        "reason": "",
        "agent": None,
    }

    def done(outcome, reason):
        result["outcome"] = outcome
        result["reason"] = reason
        print(json.dumps(result))
        return 1 if outcome == "fail" else 0

    if red:
        return done("crate-red", red)
    status = int(status)
    result["exit"] = status
    result["seconds"] = int(seconds)
    text = read(events)
    last, codes = last_event(text)
    stopped = status in STOPPED
    if stopped:
        codes.add("time-limit")
    if last is None:
        codes.add("no-last-event")
    if status == 4:
        codes.add("internal")
    own = "without any mutant" in text + read(log)
    if own and "red-baseline" not in codes:
        codes.add("tests-fail-without-any-mutant")
    result["codes"] = sorted(codes)
    if last and last["reason"] == "run-finished":
        result["mutants"] = last.get("mutants")
        result["score"] = last.get("score")

    if expect.startswith("refused:"):
        wanted = expect.split(":", 1)[1]
        if last and last["reason"] == "run-failed" and last.get("code") == wanted:
            return done("pass", first_line(last))
        if stopped:
            return done("fail", f"expected {expect}, but it stopped at the time limit")
        if last is None:
            return done("fail", f"expected {expect}, but it ended with no last event (exit {status})")
        if last["reason"] == "run-failed":
            return done("fail", f"expected {expect}, got refused:{last.get('code')}: {first_line(last)}")
        return done("fail", f"expected {expect}, got a score")

    if stopped:
        return done("fail", f"stopped at the time limit (exit {status})")
    if last is None:
        return done("fail", f"ended with no last event (exit {status}): it crashed or was stopped")
    if "internal" in codes:
        return done("fail", "an internal error: " + first_line(last))
    if last["reason"] == "run-failed":
        return done("fail", f"refused:{last.get('code')}: {first_line(last)}")
    if own or status == 3:
        return done("fail", "the tests fail without any mutant")
    if not result["mutants"]:
        return done("fail", "tested crate with 0 mutants")
    if result["score"] is None:
        return done("fail", "run-finished has no score")
    if status != 0:
        return done("fail", f"finished, but exited {status} at threshold 0")
    result["agent"] = agent_shape(agent, int(agent_status), last)
    if result["agent"] != "ok":
        return done("fail", "agent head: " + result["agent"])
    return done("pass", "")


def cell(text):
    return str(text).replace("|", "\\|").replace("\n", " ")


def shown(result, regressed):
    outcome = result["outcome"]
    if outcome == "pass" and str(result["expect"]).startswith("refused:"):
        return "✓ refused, as expected"
    if outcome == "pass":
        return "✓ pass"
    if outcome == "crate-red":
        return "– crate red: " + cell(result["reason"])
    if regressed:
        return f"**✗ fails now: {cell(result['reason'])}**"
    return "✗ " + cell(result["reason"])


def summary(results_dir, previous, names):
    """DIR's results as one file, DIR/corpus-results.jsonl, and a Markdown table for the job
    summary, against PREV, the last nightly's file. A crate that passed then and fails now is what
    the night is for: it's named, red, and the exit is 1. A name with no result failed: its job
    stopped before it wrote one."""
    corpus = {entry["name"]: entry for entry in read_entries()}
    results = []
    for name in names:
        lines = read(os.path.join(results_dir, name + ".result.json")).strip().splitlines()
        try:
            results.append(json.loads(lines[-1]))
        except (ValueError, IndexError):
            results.append(
                {
                    "name": name,
                    "expect": corpus.get(name, {}).get("expect") or "",
                    "outcome": "fail",
                    "exit": None,
                    "mutants": None,
                    "score": None,
                    "seconds": None,
                    "codes": [],
                    "reason": "no result: its job stopped before it wrote one",
                    "agent": None,
                }
            )
    with open(os.path.join(results_dir, "corpus-results.jsonl"), "w", encoding="utf-8") as merged:
        for result in results:
            merged.write(json.dumps(result) + "\n")
    before = {}
    for line in read(previous).splitlines():
        if line.strip():
            earlier = json.loads(line)
            before[earlier["name"]] = earlier["outcome"]

    regressed = {r["name"] for r in results if r["outcome"] == "fail" and before.get(r["name"]) == "pass"}
    counts = {}
    for result in results:
        counts[result["outcome"]] = counts.get(result["outcome"], 0) + 1
    print(f"## Corpus: {len(results)} crates")
    print()
    tally = ", ".join(f"{count} {outcome}" for outcome, count in sorted(counts.items()))
    print(tally + ("" if before else "; no earlier results to compare with"))
    print()
    for result in results:
        if result["name"] in regressed:
            name, reason = result["name"], result["reason"]
            print(f"- **✗ {name} passed last time and fails now:** {cell(reason)}")
            print(f"::error title=Corpus: {name} fails now::{name} passed last time: {reason}", file=sys.stderr)
    if regressed:
        print()
    print("| Crate | Expect | Result | Mutants | Score | Took | Last time | Codes |")
    print("| --- | --- | --- | ---: | ---: | ---: | --- | --- |")
    for result in results:
        name = result["name"]
        score = "" if result.get("score") is None else f"{result['score']}%"
        mutants = "" if result.get("mutants") is None else result["mutants"]
        took = "" if result.get("seconds") is None else f"{result['seconds']}s"
        codes = " ".join(result.get("codes") or [])
        print(
            f"| {name} | {result['expect']} | {shown(result, name in regressed)} | {mutants} | {score} "
            f"| {took} | {before.get(name, '—')} | {cell(codes)} |"
        )
    return 1 if regressed else 0


def main(args):
    if args[:1] == ["matrix"] and len(args) == 2:
        return matrix(args[1])
    if args[:1] == ["judge"] and len(args) == 10:
        return judge(*args[1:])
    if args[:1] == ["summary"] and len(args) >= 3:
        return summary(args[1], args[2], args[3:])
    print("corpus.py matrix ONLY | judge … | summary DIR PREV NAME…", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
