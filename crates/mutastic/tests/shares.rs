//! A run split across machines with `--shard`, and put back together with `--from-shards`: the
//! shares together are the whole run, and shares that aren't one run's are refused.

#![allow(
    clippy::unwrap_used,
    reason = "a helper here asserts as the tests it serves do: a value that isn't there fails the test"
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

mod common;
use common::schema;

/// A copy of the pricing fixture in a directory of its own, without its `target`.
fn pricing(label: &str) -> PathBuf {
    let to = std::env::temp_dir().join(format!("mutastic-shares-{label}-{}", std::process::id()));
    std::fs::remove_dir_all(&to).ok();
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pricing");
    copy(&from, &to, &["target"]);
    to
}

/// `from` into `to`, leaving out entries named in `skip`.
fn copy(from: &Path, to: &Path, skip: &[&str]) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if skip.iter().any(|name| entry.file_name() == *name) {
            continue;
        }
        let path = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &path, &[]);
        } else {
            std::fs::copy(entry.path(), path).unwrap();
        }
    }
}

/// `mutastic` run in `dir`: its exit status, stdout and stderr.
fn mutastic(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args(args)
        .current_dir(dir)
        .env_remove("MUTASTIC_FORMAT")
        .env_remove("MUTASTIC_AGENT")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn events(text: &str) -> Vec<Value> {
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// The events without what differs from one run to the next: the timings, and how many tests
/// ran before one caught a mutant, as the fastest tests run first.
fn untimed(text: &str) -> Vec<Value> {
    let mut events = events(text);
    for event in &mut events {
        let fields = event.as_object_mut().unwrap();
        fields.remove("durationMs");
        fields.remove("coverageMs");
        fields.remove("testsRun");
        if let Some(build) = fields.get_mut("build").and_then(Value::as_object_mut) {
            build.remove("setupMs");
        }
    }
    events
}

/// Takes away the package's kept runs, so its next run is a first run, with nothing to compare
/// with: what's compared here is the results, which a comparison with the last run would add to.
fn forget_last_runs(dir: &Path) {
    let kept = dir.join("target/mutastic");
    std::fs::remove_file(kept.join("last-run.tsv")).ok();
    std::fs::remove_dir_all(kept.join("last-runs")).ok();
}

/// A human format without the rows that tell the time, or how many tests ran.
fn untimed_text(text: &str) -> String {
    let varies = ["  Start at", "  Duration", " Test runs"];
    text.lines()
        .filter(|line| !varies.iter().any(|row| line.starts_with(row)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `mutation.json` without its timings, or how many tests ran.
fn untimed_report(file: &Path) -> Value {
    let mut report: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    report.as_object_mut().unwrap().remove("performance");
    for file in report["files"].as_object_mut().unwrap().values_mut() {
        for mutant in file["mutants"].as_array_mut().unwrap() {
            let fields = mutant.as_object_mut().unwrap();
            fields.remove("duration");
            fields.remove("testsCompleted");
        }
    }
    report
}

fn ids(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .filter(|event| event["reason"] == "mutant-finished")
        .map(|event| event["id"].as_str().unwrap().to_string())
        .collect()
}

const FORMATS: [&str; 4] = ["jsonl", "full", "concise", "agent"];

/// Two shares, their folders downloaded under other names, put together: the same summary,
/// cards, events and report as the whole run, in every format, the threshold judged on the whole,
/// and `explain` reading the whole run's record for a mutant of either share.
#[test]
fn two_shares_put_together_are_the_whole_run() {
    let dir = pricing("whole");
    let report = dir.join("target/mutastic/mutation.json");
    let whole: Vec<String> = FORMATS
        .iter()
        .map(|format| {
            forget_last_runs(&dir);
            let (code, out, err) = mutastic(&dir, &["run", &format!("--format={format}")]);
            assert_eq!(code, 0, "{out}{err}");
            out
        })
        .collect();
    let whole_report = untimed_report(&report);

    let shares = dir.join("target/mutastic/shares");
    let mut ran = Vec::new();
    let mut kept_results = Vec::new();
    for (number, format) in [(1, "concise"), (2, "full")] {
        let shard = format!("--shard={number}/2");
        let (code, out, err) = mutastic(
            &dir,
            &[
                "run",
                &shard,
                "--threshold=0",
                &format!("--format={format}"),
            ],
        );
        assert_eq!(code, 0, "{out}{err}");
        let folder = shares.join(format!("{number}-of-2"));
        assert!(
            out.contains(&format!(
                "target/mutastic/shares/{number}-of-2/events.jsonl"
            )),
            "{out}"
        );
        assert!(folder.join("mutation.json").is_file());
        let kept = events(&std::fs::read_to_string(folder.join("events.jsonl")).unwrap());
        for event in &kept {
            let errors: Vec<String> = schema().iter_errors(event).map(|e| e.to_string()).collect();
            assert!(errors.is_empty(), "{event}\n{}", errors.join("\n"));
        }
        assert_eq!(kept[0]["share"]["number"], number);
        assert_eq!(kept[0]["share"]["of"], 2);
        assert_eq!(kept[0]["share"]["version"], env!("CARGO_PKG_VERSION"));
        ran.push(ids(&kept));
        kept_results.extend(
            kept.into_iter()
                .filter(|e| e["reason"] == "mutant-finished"),
        );
    }
    assert!(!ran[0].is_empty() && !ran[1].is_empty(), "{ran:?}");
    assert!(ran[0].iter().all(|id| !ran[1].contains(id)), "{ran:?}");
    let mut both: Vec<String> = ran.concat();
    both.sort();
    let mut all = ids(&events(&whole[0]));
    all.sort();
    assert_eq!(both, all);
    assert_eq!(
        untimed_report(&report),
        whole_report,
        "a share's report is its own, not the whole run's"
    );

    // Each job uploads its share's folder, and the last downloads them all, named its own way.
    let downloaded = dir.join("downloaded");
    copy(&shares.join("1-of-2"), &downloaded.join("share-1"), &[]);
    copy(
        &shares.join("2-of-2"),
        &downloaded.join("deeper/share-2"),
        &[],
    );
    std::fs::remove_file(&report).unwrap();
    for (format, whole) in FORMATS.iter().zip(&whole) {
        forget_last_runs(&dir);
        let (code, out, err) = mutastic(
            &dir,
            &[
                "run",
                "--from-shards=downloaded",
                &format!("--format={format}"),
            ],
        );
        assert_eq!(code, 0, "{out}{err}");
        if *format == "jsonl" {
            assert_eq!(untimed(&out), untimed(whole));
            // Each result is its share's, to the millisecond.
            for event in events(&out)
                .iter()
                .filter(|e| e["reason"] == "mutant-finished")
            {
                assert!(kept_results.contains(event), "{event}");
            }
            for event in events(&out) {
                assert!(schema().iter_errors(&event).next().is_none(), "{event}");
            }
        } else {
            assert_eq!(untimed_text(&out), untimed_text(whole), "{format}");
        }
    }
    assert_eq!(untimed_report(&report), whole_report);
    // The shares put together are a whole run, compared with the last whole run, as in the Java
    // edition.
    let (_, out, _) = mutastic(&dir, &["run", "--from-shards=downloaded", "--format=jsonl"]);
    let finished = events(&out).pop().unwrap();
    assert_eq!(finished["previousScore"], finished["score"], "{finished}");
    assert_eq!(finished["sameCodeAsLastRun"], true, "{finished}");

    // explain reads the whole run's record, for a mutant of either share.
    let missed = "mb2f8ae57";
    let caught = "mc9e13290";
    assert!(ran[1].contains(&missed.to_string()) && ran[0].contains(&caught.to_string()));
    let (_, out, _) = mutastic(&dir, &["explain", missed]);
    assert!(
        out.contains("last run  missed: every covering test still passed"),
        "{out}"
    );
    let (_, out, _) = mutastic(&dir, &["explain", caught]);
    assert!(
        out.contains("last run  caught by tests::small_orders_pay_full_price"),
        "{out}"
    );

    // The threshold is the whole run's: share 2 alone scores 83.3, the whole 90.9.
    let second = events(&std::fs::read_to_string(shares.join("2-of-2/events.jsonl")).unwrap());
    assert_eq!(second.last().unwrap()["score"], 83.3);
    let at = |threshold: &str| {
        let (code, out, _) = mutastic(
            &dir,
            &[
                "run",
                "--from-shards=downloaded",
                "--format=jsonl",
                threshold,
            ],
        );
        (code, events(&out).pop().unwrap())
    };
    let (code, finished) = at("--threshold=90");
    assert_eq!((code, &finished["passed"]), (0, &Value::Bool(true)));
    let (code, finished) = at("--threshold=91");
    assert_eq!((code, &finished["passed"]), (1, &Value::Bool(false)));
    assert_eq!(finished["score"], 90.9);

    // One share of one is a share too, and is the whole run.
    let (code, out, err) = mutastic(&dir, &["run", "--shard=1/1", "--format=jsonl"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(events(&out)[0]["share"]["of"], 1);
    forget_last_runs(&dir);
    let (code, out, err) = mutastic(
        &dir,
        &[
            "run",
            "--from-shards=target/mutastic/shares/1-of-1",
            "--format=jsonl",
        ],
    );
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(untimed(&out), untimed(&whole[0]));
    std::fs::remove_dir_all(&dir).ok();
}

/// A share's events, read to change.
fn read_share(file: &Path) -> Vec<Value> {
    events(&std::fs::read_to_string(file).unwrap())
}

fn write_share(file: &Path, events: &[Value]) {
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let text: String = events.iter().map(|event| format!("{event}\n")).collect();
    std::fs::write(file, text).unwrap();
}

/// `run --from-shards` on `shares`, which must refuse them with exit status 2, saying `why`.
fn refuses(dir: &Path, shares: &Path, extra: &[&str], why: &str) {
    let from = format!("--from-shards={}", shares.display());
    let args: Vec<&str> = ["run", from.as_str()]
        .iter()
        .copied()
        .chain(extra.iter().copied())
        .collect();
    let (code, out, err) = mutastic(dir, &args);
    assert_eq!(code, 2, "{why}\n{out}{err}");
    assert!(err.contains(why), "expected {why:?} in:\n{err}");
}

/// Shares that aren't every share of one run are refused, with exit status 2 and what's wrong,
/// and `--from-shards` with what a share runs is a usage error.
#[test]
fn shares_that_arent_one_run_are_refused() {
    let dir = pricing("refused");
    for number in [1, 2] {
        let shard = format!("--shard={number}/2");
        let (code, out, err) = mutastic(&dir, &["run", &shard, "--threshold=0", "--format=jsonl"]);
        assert_eq!(code, 0, "{out}{err}");
    }
    let kept = |number: usize| {
        read_share(&dir.join(format!("target/mutastic/shares/{number}-of-2/events.jsonl")))
    };
    let (first, second) = (kept(1), kept(2));
    let case = |name: &str, shares: &[&[Value]]| -> PathBuf {
        let folder = dir.join("cases").join(name);
        std::fs::create_dir_all(&folder).unwrap();
        for (index, events) in shares.iter().enumerate() {
            write_share(&folder.join(format!("{index}/events.jsonl")), events);
        }
        folder
    };
    let changed = |events: &[Value], change: &dyn Fn(&mut Vec<Value>)| {
        let mut events = events.to_vec();
        change(&mut events);
        events
    };

    refuses(&dir, &dir.join("nowhere"), &[], "there's no directory");
    refuses(
        &dir,
        &case("none", &[]),
        &[],
        "holds no share's events.jsonl",
    );
    let not_json = case("not-json", &[&first]);
    std::fs::write(not_json.join("0/events.jsonl"), "not json\n").unwrap();
    refuses(
        &dir,
        &not_json,
        &[],
        "isn't a share's events: line 1 isn't JSON",
    );
    let unshared = changed(&first, &|events| {
        events[0].as_object_mut().unwrap().remove("share");
    });
    refuses(
        &dir,
        &case("unshared", &[&unshared, &second]),
        &[],
        "its run-started names no share",
    );
    refuses(
        &dir,
        &case("missing", &[&first]),
        &[],
        "share 2 of 2 is missing",
    );
    refuses(
        &dir,
        &case("twice", &[&first, &first, &second]),
        &[],
        "share 1 of 2 is there twice",
    );
    let unfinished = changed(&second, &|events| {
        events.pop();
    });
    refuses(
        &dir,
        &case("unfinished", &[&first, &unfinished]),
        &[],
        "didn't finish",
    );
    let of_three = changed(&second, &|events| events[0]["share"]["of"] = 3.into());
    refuses(
        &dir,
        &case("of-three", &[&first, &of_three]),
        &[],
        "split differently",
    );
    let doubled = changed(&second, &|events| {
        let mutant = first
            .iter()
            .find(|e| e["reason"] == "mutant-finished")
            .unwrap();
        events.insert(1, mutant.clone());
    });
    refuses(
        &dir,
        &case("doubled", &[&first, &doubled]),
        &[],
        "is in share 1 of 2",
    );
    let older = |events: &[Value]| {
        changed(events, &|events| {
            events[0]["share"]["version"] = "0.0.1".into()
        })
    };
    refuses(
        &dir,
        &case("older", &[&older(&first), &older(&second)]),
        &[],
        "was run by mutastic 0.0.1",
    );
    let other_code = changed(&second, &|events| {
        events[0]["share"]["code"] = "0".repeat(64).into()
    });
    refuses(
        &dir,
        &case("other-code", &[&first, &other_code]),
        &[],
        "ran on different code",
    );
    let other = |events: &[Value]| changed(events, &|events| events[0]["project"] = "shop".into());
    refuses(
        &dir,
        &case("other-project", &[&other(&first), &other(&second)]),
        &[],
        "are of shop, not pricing",
    );
    let since = changed(&second, &|events| {
        events.last_mut().unwrap()["scope"] = serde_json::json!({ "since": "main" });
    });
    refuses(
        &dir,
        &case("since", &[&first, &since]),
        &[],
        "ran only a change's lines (--since)",
    );
    let both = case("both", &[&first, &second]);
    refuses(
        &dir,
        &both,
        &["--operators=stronger"],
        "didn't run the mutants this run finds",
    );
    let source = dir.join("src/lib.rs");
    let original = std::fs::read_to_string(&source).unwrap();
    std::fs::write(&source, format!("{original}// changed since\n")).unwrap();
    refuses(&dir, &both, &[], "ran on other code than this");
    std::fs::write(&source, original).unwrap();

    for (args, why) in [
        (
            vec!["run", "--from-shards=cases/both", "--shard=1/2"],
            "not with --shard",
        ),
        (
            vec!["run", "--from-shards=cases/both", "--only=mb2f8ae57"],
            "not with --only",
        ),
        (
            vec!["run", "--from-shards=cases/both", "--since"],
            "not with --since",
        ),
        (
            vec!["run", "--from-shards"],
            "--from-shards needs the shares' directory",
        ),
        (
            vec!["verify", "mb2f8ae57", "--from-shards=cases/both"],
            "verify doesn't take --from-shards",
        ),
        (
            vec!["explain", "mb2f8ae57", "--from-shards=cases/both"],
            "explain doesn't take --from-shards",
        ),
        (
            vec!["mutants", "--from-shards=cases/both"],
            "mutants doesn't take --from-shards",
        ),
        (
            vec!["run", "--shard=3/2"],
            "--shard: 3 isn't a number from 1 to 2",
        ),
        (
            vec!["run", "--shard=2"],
            "--shard needs a share and how many, as 2/4",
        ),
    ] {
        let (code, out, err) = mutastic(&dir, &args);
        assert_eq!(code, 2, "{args:?}\n{out}{err}");
        assert!(err.contains(why), "expected {why:?} in:\n{err}");
    }
    let (code, out, err) = mutastic(&dir, &["run", "--from-shards=cases/both", "--format=jsonl"]);
    assert_eq!(code, 0, "the cases' own shares put together: {out}{err}");
    std::fs::remove_dir_all(&dir).ok();
}
