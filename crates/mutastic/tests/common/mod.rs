//! What the integration tests share.

#![allow(
    clippy::disallowed_methods,
    reason = "where the Java edition is kept; an allow, as not every test crate uses this module"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use serde_json::Value;

/// `mutastic.events/v1`, pinned from the Java repository (`docs/public/schemas/events/v1.json` on
/// its `progress-p2b` branch, mutastic #44: with `mutant-explained`, the optional `docs` the mutant
/// events gained, `run-finished`'s `failOnNew`, `newMisses` and `inBaseline`, and the live
/// `phase-started`, `phase-finished` and `progress` events). It asks `jdk` only of the Java
/// edition's events, and every Rust event says `"edition": "rust"`, so it's taken as it is.
#[allow(
    dead_code,
    reason = "only some test crates check events against the schema"
)]
pub fn schema() -> jsonschema::Validator {
    jsonschema::validator_for(&pinned()).unwrap()
}

/// What the schema says a mutation's `applied` is. The schema leaves objects open, so an event
/// with a field it doesn't name still passes [`schema`]: this checks `applied` is one it names, and
/// as it describes it.
#[allow(
    dead_code,
    reason = "only some test crates check `applied`, and an `expect` would fail in the others"
)]
pub fn applied_schema() -> jsonschema::Validator {
    let applied = pinned()["$defs"]["mutantFinished"]["properties"]["mutation"]["properties"]
        ["applied"]
        .clone();
    assert!(applied.is_object(), "the pinned schema names no `applied`");
    jsonschema::validator_for(&applied).unwrap()
}

#[allow(
    dead_code,
    reason = "only some test crates check events against the schema"
)]
fn pinned() -> Value {
    serde_json::from_str(include_str!("../schema/events-v1.json")).unwrap()
}

/// Where the tests keep the directories they run mutastic in: outside the repository, so Cargo
/// doesn't take a fixture's copy for a member of mutastic's own workspace, and git doesn't take it
/// for part of mutastic's repository.
#[allow(
    dead_code,
    reason = "only some test crates run mutastic in a directory of their own"
)]
pub fn slots() -> PathBuf {
    scratch().join("mutastic-rs-tests")
}

/// Where the tests keep what should last from one test process to the next, a fixture's copy with
/// its build: the directory `MUTASTIC_RS_TEST_SCRATCH` names, or the temporary directory without
/// it. mutastic gives each test process an empty temporary directory of its own and deletes it
/// when the process ends, so in a run of mutastic on itself nothing kept there is found again, and
/// every fixture builds cold for every mutant: such a run names a directory that lasts.
#[allow(
    dead_code,
    reason = "only some test crates run mutastic in a directory of their own"
)]
pub fn scratch() -> PathBuf {
    scratch_from(std::env::var_os("MUTASTIC_RS_TEST_SCRATCH"))
}

/// [`scratch`], given what the variable holds: unset or empty, the temporary directory.
#[allow(
    dead_code,
    reason = "only some test crates run mutastic in a directory of their own"
)]
pub fn scratch_from(named: Option<OsString>) -> PathBuf {
    named
        .filter(|named| !named.is_empty())
        .map_or_else(std::env::temp_dir, PathBuf::from)
}

/// A directory under `base` this test process has to itself, `name` or the first of `name-1`,
/// `name-2`, … no other process holds, and whether it was taken just now. A run on a mutant of
/// mutastic runs these tests in each of its workers at once: in one directory, each run would
/// wait for the others' lock, or rewrite their copy while they ran. Kept between processes, so its
/// build is warm for the next. The process holds it until it exits; its threads share it.
///
/// There are as many of each as the machine has cores, at most: past that, it waits for one to be
/// let go, as more at once would only slow each other down. A copy no process has taken for a day
/// is removed, and its build with it.
#[allow(
    dead_code,
    reason = "only some test crates run mutastic in a directory of their own"
)]
pub fn slot(base: &Path, name: &str) -> (PathBuf, bool) {
    let most = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let mut waiting = false;
    loop {
        let mut held = held().lock().unwrap();
        if let Some((dir, _)) = held.taken.get(&(base.to_path_buf(), name.to_string())) {
            return (dir.clone(), false);
        }
        std::fs::create_dir_all(base).unwrap();
        if held.pruned.insert(base.to_path_buf()) {
            prune(base);
        }
        for n in 0..most {
            let named = if n == 0 {
                name.to_string()
            } else {
                format!("{name}-{n}")
            };
            let dir = base.join(&named);
            let lock = File::create(base.join(format!("{named}.lock"))).unwrap();
            if lock.try_lock().is_ok() {
                // When it was last taken, for `prune`.
                lock.set_modified(SystemTime::now()).unwrap();
                std::fs::create_dir_all(&dir).unwrap();
                held.taken
                    .insert((base.to_path_buf(), name.to_string()), (dir.clone(), lock));
                return (dir, true);
            }
        }
        drop(held);
        if !waiting {
            eprintln!(
                "waiting for one of the {most} copies of {name} in {} other test processes hold",
                base.display()
            );
            waiting = true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// The slots this process holds, by base and name, and the bases it has pruned.
#[derive(Default)]
struct Held {
    taken: BTreeMap<(PathBuf, String), (PathBuf, File)>,
    pruned: BTreeSet<PathBuf>,
}

fn held() -> &'static Mutex<Held> {
    static HELD: OnceLock<Mutex<Held>> = OnceLock::new();
    HELD.get_or_init(Mutex::default)
}

/// Removes each directory under `base` whose lock nobody has taken for a day and nobody holds now.
/// The lock stays, empty, so a process that opens it as this one removes the directory still
/// waits for the lock, and finds no other process's directory gone.
fn prune(base: &Path) {
    const DAY: Duration = Duration::from_secs(24 * 60 * 60);
    for entry in std::fs::read_dir(base).into_iter().flatten().flatten() {
        let path = entry.path();
        let Some(named) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".lock"))
        else {
            continue;
        };
        let dir = base.join(named);
        let unused = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .is_ok_and(|taken| taken.elapsed().is_ok_and(|age| age > DAY));
        if !unused || !dir.exists() {
            continue;
        }
        let Ok(lock) = std::fs::OpenOptions::new().write(true).open(&path) else {
            continue;
        };
        if lock.try_lock().is_ok() {
            remove(&dir);
        }
    }
}

/// A copy of the fixture `name` this test process has to itself, under `label`: on the first ask
/// in the process, the fixture as it is, with no runs kept from before but its build still warm.
#[allow(
    dead_code,
    reason = "only some test crates run mutastic in a directory of their own"
)]
pub fn own_copy(name: &str, label: &str) -> PathBuf {
    let (dir, taken) = slot(&slots(), label);
    if taken {
        fresh(&fixture(name), &dir);
    }
    dir
}

/// A fixture in the repository, as it is checked in.
#[allow(dead_code, reason = "only some test crates read the fixtures")]
pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

/// Makes `to` the tree at `from`, writing only what changed and removing what `from` doesn't have,
/// and forgets what mutastic kept from `to`'s last runs, keeping its builds: its next run is a
/// first run, as a fresh copy's is, only faster.
#[allow(
    dead_code,
    reason = "only some test crates run mutastic in a directory of their own"
)]
pub fn fresh(from: &Path, to: &Path) {
    mirror(from, to);
    let kept = to.join("target/mutastic");
    for entry in std::fs::read_dir(&kept).into_iter().flatten() {
        let entry = entry.unwrap();
        if !matches!(entry.file_name().to_str(), Some("copy" | "target" | "lock")) {
            remove(&entry.path());
        }
    }
}

fn mirror(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    let mut wanted = BTreeSet::new();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if name == "target" {
            continue;
        }
        let (source, destination) = (from.join(&name), to.join(&name));
        if entry.file_type().unwrap().is_dir() {
            if destination.is_file() {
                remove(&destination);
            }
            mirror(&source, &destination);
        } else {
            let bytes = std::fs::read(&source).unwrap();
            if std::fs::read(&destination).ok() != Some(bytes.clone()) {
                remove(&destination);
                std::fs::write(&destination, bytes).unwrap();
            }
        }
        wanted.insert(name);
    }
    for entry in std::fs::read_dir(to).unwrap() {
        let name = entry.unwrap().file_name();
        if name != "target" && !wanted.contains(&name) {
            remove(&to.join(name));
        }
    }
}

fn remove(path: &Path) {
    if path.is_dir() && !path.is_symlink() {
        std::fs::remove_dir_all(path).ok();
    } else {
        std::fs::remove_file(path).ok();
    }
}
