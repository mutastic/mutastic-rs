//! The directories the tests run mutastic in, from `common::slot`: no more of each than the machine
//! has cores, and none kept once it's gone unused for a day, so repeated and parallel runs don't
//! fill the disk.

#![allow(
    clippy::unwrap_used,
    reason = "a helper here asserts as the tests it serves do: a value that isn't there fails the test"
)]

use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

mod common;

/// A new, empty base of the test's own.
fn base(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("mutastic-slots-{}", std::process::id()))
        .join(name);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A slot `named` under `base`, with a file in it, last taken `ago`.
fn kept(base: &Path, named: &str, ago: Duration) -> File {
    std::fs::create_dir_all(base.join(named)).unwrap();
    std::fs::write(base.join(named).join("built"), "").unwrap();
    let lock = File::create(base.join(format!("{named}.lock"))).unwrap();
    lock.set_modified(SystemTime::now() - ago).unwrap();
    lock
}

/// Another process's hold on the slot `named`: a lock of its own on the file.
fn hold(base: &Path, named: &str) -> File {
    let lock = File::create(base.join(format!("{named}.lock"))).unwrap();
    lock.try_lock().unwrap();
    lock
}

#[test]
fn a_slot_unused_for_a_day_is_removed_and_one_held_or_used_is_kept() {
    const DAY: Duration = Duration::from_secs(24 * 60 * 60);
    let base = base("pruned");
    drop(kept(&base, "old", DAY * 2));
    // Held by another process, as a long run would, since a day ago.
    let held = kept(&base, "old-held", DAY * 2);
    held.try_lock().unwrap();
    drop(kept(&base, "recent", DAY / 2));
    let (dir, taken) = common::slot(&base, "mine");
    assert!(taken && dir == base.join("mine") && dir.is_dir());
    assert!(!base.join("old").exists(), "unused for a day, removed");
    assert!(base.join("old.lock").exists(), "its lock stays");
    assert!(base.join("old-held/built").exists(), "held, kept");
    assert!(base.join("recent/built").exists(), "used today, kept");
}

#[test]
fn a_process_keeps_the_slot_it_took() {
    let base = base("kept");
    let first = hold(&base, "same");
    let (dir, taken) = common::slot(&base, "same");
    assert!(taken && dir == base.join("same-1"));
    drop(first);
    // The first is free now, but this process has its own already.
    assert_eq!(common::slot(&base, "same"), (base.join("same-1"), false));
}

#[test]
fn past_one_a_core_a_slot_waits_for_one_to_be_let_go() {
    let base = base("capped");
    let cores = std::thread::available_parallelism().unwrap().get();
    let mut held: Vec<File> = (0..cores)
        .map(|n| {
            let named = if n == 0 {
                "busy".to_string()
            } else {
                format!("busy-{n}")
            };
            hold(&base, &named)
        })
        .collect();
    let waiting = {
        let base = base.clone();
        std::thread::spawn(move || common::slot(&base, "busy"))
    };
    std::thread::sleep(Duration::from_millis(600));
    assert!(!waiting.is_finished(), "it took one past the cores");
    assert!(!base.join(format!("busy-{cores}")).exists());
    drop(held.pop());
    let last = if cores == 1 {
        "busy".to_string()
    } else {
        format!("busy-{}", cores - 1)
    };
    assert_eq!(waiting.join().unwrap(), (base.join(last), true));
}
