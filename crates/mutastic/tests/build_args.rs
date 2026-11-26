//! Arguments after `--`, in `MUTASTIC_BUILD_ARGS` and in mutastic.yaml's `build:` `args:` reach Cargo,
//! choose the code mutated with its features, and are told apart in the run record; only those typed
//! are repeated.

#![expect(clippy::disallowed_methods, reason = "runs mutastic as a user does")]
#![allow(
    clippy::unwrap_used,
    reason = "a helper here asserts as the tests it serves do: a value that isn't there fails the test"
)]

use std::path::Path;
use std::process::Command;

mod common;

fn mutastic(dir: &Path, variable: &str, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args(args)
        .current_dir(dir)
        .env_remove("MUTASTIC_FORMAT")
        .env_remove("MUTASTIC_AGENT")
        .env("MUTASTIC_BUILD_ARGS", variable)
        .env("MUTASTIC_PROGRESS", "none")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).into_owned() + &String::from_utf8_lossy(&output.stderr)
}

/// A package with code and a test behind a feature, on one copy of the pricing fixture.
#[test]
fn arguments_reach_cargo_and_only_typed_ones_are_repeated() {
    let dir = common::own_copy("pricing", "build-args");
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"pricing\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\
         [features]\ngift = []\n[workspace]\n",
    )
    .unwrap();
    std::fs::remove_dir_all(dir.join("tests")).ok();
    std::fs::write(
        dir.join("src/lib.rs"),
        "pub fn plain(total: u32) -> bool { total >= 100 }\n\
         #[cfg(feature = \"gift\")]\npub fn gift(total: u32) -> bool { total >= 50 }\n\
         #[cfg(test)]\nmod tests {\n    use super::*;\n\
         #[test] fn p() { assert!(plain(100)); assert!(!plain(99)); }\n\
         #[cfg(feature = \"gift\")] #[test] fn g() { assert!(gift(60)); }\n}\n",
    )
    .unwrap();
    let typed = ["run", "--", "--features", "gift"];
    let offered = "mutastic verify <id> . -- --features gift ";

    let plain = mutastic(&dir, "", &["run"]);
    assert!(plain.contains("Score  100.0%"), "{plain}");
    let first = mutastic(&dir, "", &typed);
    assert!(
        first.contains("2 missed") && first.contains("(code changed)"),
        "{first}"
    );
    assert!(first.contains(offered), "{first}");
    let again = mutastic(&dir, "", &typed);
    assert!(again.contains("(same code)"), "{again}");

    let variable = mutastic(&dir, "--features gift", &["run"]);
    assert!(
        variable.contains("2 missed") && variable.contains("(same code)"),
        "{variable}"
    );
    assert!(!variable.contains("--features"), "{variable}");
    let none = mutastic(&dir, "--features gift", &["run", "--"]);
    assert!(none.contains("Score  100.0%"), "{none}");

    std::fs::write(
        dir.join("mutastic.yaml"),
        "build:\n  args:\n    - --features=gift\n",
    )
    .unwrap();
    let file = mutastic(&dir, "", &["run"]);
    assert!(
        file.contains("2 missed") && !file.contains("--features"),
        "{file}"
    );
    std::fs::write(
        dir.join("mutastic.yaml"),
        "build:\n  args:\n    - --release\n",
    )
    .unwrap();
    let refused = mutastic(&dir, "", &["run"]);
    assert!(
        refused.contains("mutastic.yaml line 3: --release can't be passed to Cargo: "),
        "{refused}"
    );
}

/// A package whose feature only its tests see, so the mutants are the same with it and without.
fn package_at(dir: &Path) {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"shop\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\
         [features]\ngift = []\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/lib.rs"),
        "pub fn plain(t: u32) -> bool { t >= 100 }\n#[cfg(test)]\nmod tests {\n\
         use super::*;\n#[test] fn p() { assert!(plain(101)); }\n\
         #[cfg(feature = \"gift\")]\n#[test] fn g() { assert!(plain(100)); \
         assert!(!plain(99)); }\n}\n",
    )
    .unwrap();
}

/// Shares made in work directories of their own, each with a key of its own, merge when they were
/// built with the same arguments, and are refused when their features differ, even where only the
/// tests can tell.
#[test]
fn shares_of_separate_jobs_merge_only_with_the_same_features() {
    let root = common::own_copy("pricing", "build-args-shares");
    let job = |label: &str| {
        let dir = root.join(label).join("shop");
        package_at(&dir);
        dir
    };
    let share = |label: &str, number: &str, build: &[&str]| {
        let dir = job(label);
        let mut args = vec!["run", "--threshold=0", number, "--"];
        args.extend(build);
        mutastic(&dir, "", &args);
        dir.join("target/mutastic/shares")
    };
    let together = |label: &str, shares: [&Path; 2]| {
        let into = root.join(label);
        for (from, named) in shares.iter().zip(["1-of-2", "2-of-2"]) {
            std::fs::create_dir_all(into.join(named)).unwrap();
            let events = format!("{named}/events.jsonl");
            std::fs::copy(from.join(&events), into.join(&events)).unwrap();
        }
        format!("--from-shards={}", into.display())
    };
    let first = share("a", "--shard=1/2", &["-Fgift"]);
    let same = share("b", "--shard=2/2", &["--features", "gift"]);
    let other = share("c", "--shard=2/2", &[]);
    let key = |shares: &Path| std::fs::read(shares.with_file_name("build-args.key")).unwrap();
    assert_ne!(key(&first), key(&same));

    let whole = job("whole");
    let merged = together("same", [&first, &same]);
    let run = mutastic(
        &whole,
        "",
        &["run", "--threshold=0", &merged, "--", "-Fgift"],
    );
    assert!(run.contains("Score  100.0%"), "{run}");
    let bare = mutastic(&whole, "", &["run", "--threshold=0", &merged]);
    assert!(bare.contains("ran on other code than this"), "{bare}");
    let mixed = together("mixed", [&first, &other]);
    let refused = mutastic(
        &whole,
        "",
        &["run", "--threshold=0", &mixed, "--", "-Fgift"],
    );
    assert!(refused.contains("ran on different code"), "{refused}");
}
