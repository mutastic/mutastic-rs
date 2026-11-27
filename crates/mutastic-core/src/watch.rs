//! The tests watched since a run named them likely flaky (decision 27), as the Java edition's
//! `FlakyWatch` keeps them: beside the run records, in `last-runs/flaky-watch.tsv`, so whatever
//! keeps those between runs (a CI cache) keeps it too. One test per line, `<test>\t<runs held>`,
//! sorted by test.
//!
//! Each catch a watched test makes is confirmed [`CONFIRMATIONS`] more times, and counts only if it
//! fails every time. Once its catches have held for [`NEEDED`] full runs in a row, it's forgotten.
//! Only a full run, or the `--from-shards` merge, changes the watch: a share reads it and confirms
//! against it, but shares may run side by side on one work directory, and each sees only part of a
//! test's catches, so none writes it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::record::Entry;

/// How many more times each catch a watched test makes is run against its mutant. A test that fails
/// 1 time in 3 on its own, as one did on a real project, makes a false catch that counts 1 time in
/// 3 unchecked, 1 in 9 confirmed once, and 1 in 81 confirmed 3 times.
pub const CONFIRMATIONS: usize = 3;

/// How many full runs in a row a watched test's catches must hold before it's forgotten: a false
/// catch holds [`CONFIRMATIONS`] confirmations 1 time in 27, so 3 such runs by luck alone is 1 in
/// about 20,000.
pub const NEEDED: usize = 3;

/// Its name in `last-runs/`, which the run records pass over: it isn't a run's record.
pub const FILE: &str = "flaky-watch.tsv";

/// What earlier runs say about the tests' catches: the last comparable run's results, when the
/// code, the tests and mutastic are what it had, so a mutant whose result changed points at the
/// test that caught it; and the tests watched.
#[derive(Clone, Debug, Default)]
pub struct History {
    /// The last comparable run's results by mutant id, only when nothing it ran has changed since.
    pub same_code: BTreeMap<String, Entry>,
    /// The tests in `last-runs/flaky-watch.tsv`.
    pub watched: BTreeSet<String>,
}

impl History {
    /// How many more times a catch by `test` is run against its mutant before it counts:
    /// [`CONFIRMATIONS`] for a watched test, once with `--confirm-catches`, else none.
    pub fn confirmations_of(&self, test: &str, confirm: bool) -> usize {
        if self.watched.contains(test) {
            CONFIRMATIONS
        } else {
            usize::from(confirm)
        }
    }
}

/// A test watched since an earlier run named it, as this run left it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Watched {
    pub test: String,
    /// Full runs in a row, this one included when it's one, in which it caught mutants and every
    /// catch held.
    pub held: usize,
    /// Whether this run made it [`NEEDED`], so it's forgotten after it.
    pub released: bool,
}

/// The watch file of the project whose work directory is `folder`.
pub fn file(folder: &Path) -> PathBuf {
    folder.join("last-runs").join(FILE)
}

/// Each test watched, with its runs held; none when there's no file, or it can't be read. A line
/// that isn't a test and a count is passed over.
pub fn read(folder: &Path) -> BTreeMap<String, usize> {
    let Ok(text) = std::fs::read_to_string(file(folder)) else {
        return BTreeMap::new();
    };
    text.lines()
        .filter_map(|line| {
            let (test, held) = line.rsplit_once('\t')?;
            (!test.is_empty()).then_some(())?;
            Some((test.to_string(), held.parse().ok()?))
        })
        .collect()
}

/// Keeps `watched`, or removes the file when there's none to watch.
pub fn write(folder: &Path, watched: &BTreeMap<String, usize>) -> std::io::Result<()> {
    let file = file(folder);
    if watched.is_empty() {
        return match std::fs::remove_file(&file) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lines: String = watched
        .iter()
        .map(|(test, held)| format!("{test}\t{held}\n"))
        .collect();
    std::fs::write(file, lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(label: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("mutastic-watch-{label}-{}", std::process::id()));
        std::fs::remove_dir_all(&folder).ok();
        folder
    }

    #[test]
    fn it_is_kept_sorted_one_test_a_line_and_removed_when_empty() {
        let folder = folder("kept");
        let watched = BTreeMap::from([("b::b".to_string(), 2), ("a::a".to_string(), 0)]);
        write(&folder, &watched).unwrap();

        assert_eq!(
            std::fs::read_to_string(file(&folder)).unwrap(),
            "a::a\t0\nb::b\t2\n"
        );
        assert_eq!(read(&folder), watched);

        write(&folder, &BTreeMap::new()).unwrap();

        assert!(!file(&folder).exists());
        assert!(read(&folder).is_empty());
        std::fs::remove_dir_all(&folder).ok();
    }

    #[test]
    fn a_line_that_isnt_a_test_and_a_count_is_passed_over() {
        let folder = folder("lines");
        std::fs::create_dir_all(folder.join("last-runs")).unwrap();
        std::fs::write(file(&folder), "a::a\t1\nnot a line\nb::b\tmany\n").unwrap();

        assert_eq!(read(&folder), BTreeMap::from([("a::a".to_string(), 1)]));
        std::fs::remove_dir_all(&folder).ok();
    }

    #[test]
    fn a_watched_tests_catches_are_confirmed_three_times() {
        let history = History {
            same_code: BTreeMap::new(),
            watched: BTreeSet::from(["a::a".to_string()]),
        };

        assert_eq!(history.confirmations_of("a::a", false), 3);
        assert_eq!(history.confirmations_of("b::b", true), 1);
        assert_eq!(history.confirmations_of("b::b", false), 0);
    }
}
