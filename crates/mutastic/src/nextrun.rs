//! What the last `mutastic next` left uncaught, kept in `target/mutastic/next.tsv`, so the next one
//! runs only those mutants while the code under test is the same: a test written since can only
//! catch some of them, never make a new one. A change to the code under test, the operators, the
//! unsafe code mutated, the lines in scope or mutastic itself runs every mutant again. As the Java
//! edition's `NextRun`.
//!
//! The file is one entry per line, so it diffs well: `key <key>`, then an uncaught id a line.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::engine::Status;
use crate::sites::Crate;

/// The file in the package's `target/mutastic`.
pub const FILE: &str = "next.tsv";

/// What the last `next` left uncaught, and what it ran under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NextRun {
    /// What the mutants were found under, as [`key`] makes it.
    pub key: String,
    /// The ids no test caught then: missed, not covered, or crashed where a crash proves nothing.
    pub uncaught: BTreeSet<String>,
}

/// What a queue was found under: the code under test, the operators, how unsafe code was treated,
/// the lines in scope and the mutastic that ran, each as a word, joined by `|`.
pub fn key(code: &str, operators: &[&str], unsafe_mode: &str, since: &str, tool: &str) -> String {
    let mut operators = operators.to_vec();
    operators.sort_unstable();
    [code, &operators.join(","), unsafe_mode, since, tool].join("|")
}

/// A hash of the code under test: each file a run mutates, by its path, with its test code taken
/// out ([`crate::sites::without_test_code`]). A test written in `tests/` or in a `mod tests` leaves
/// it as it was, so the next `next` runs only what was left uncaught; a change to the code it
/// tests changes it. The run record's hash, of every file, tests included, would change with each
/// test written.
pub fn code_under_test(found: &Crate) -> String {
    let mut hasher = Sha256::new();
    let mut files: Vec<(String, &str)> = found
        .files
        .iter()
        .map(|file| (crate::workspace::slashed(&file.path), file.text.as_str()))
        .collect();
    files.sort();
    for (path, text) in files {
        let code = crate::sites::without_test_code(text);
        hasher.update(path.as_bytes());
        hasher.update([0u8]);
        hasher.update((code.len() as u64).to_le_bytes());
        hasher.update(code.as_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// What a `next` leaves uncaught: each of the ids it `asked` to run (every one it found, on a
/// first pass) but those a test caught and those skipped. One with no result, as when its run was
/// cut short, stays uncaught: a false catch is worse than a miss, and dropping it would take it
/// out of every queue until the code changes.
pub fn left_uncaught<'a>(
    asked: &BTreeSet<String>,
    results: impl IntoIterator<Item = (&'a str, Status)>,
) -> BTreeSet<String> {
    let settled: BTreeSet<&str> = results
        .into_iter()
        .filter(|(_, status)| status.detected() || *status == Status::Ignored)
        .map(|(id, _)| id)
        .collect();
    asked
        .iter()
        .filter(|id| !settled.contains(id.as_str()))
        .cloned()
        .collect()
}

impl NextRun {
    fn file(folder: &Path) -> PathBuf {
        folder.join(FILE)
    }

    /// What the last `next` left, if it ran under `key`.
    pub fn read(folder: &Path, key: &str) -> Option<NextRun> {
        let text = std::fs::read_to_string(NextRun::file(folder)).ok()?;
        let mut lines = text.lines();
        let kept = lines.next()?.strip_prefix("key\t")?;
        if kept != key {
            return None;
        }
        Some(NextRun {
            key: kept.to_string(),
            uncaught: lines
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(str::to_string)
                .collect(),
        })
    }

    pub fn write(&self, folder: &Path) -> Result<(), String> {
        let mut text = format!("key\t{}\n", self.key);
        for id in &self.uncaught {
            text.push_str(id);
            text.push('\n');
        }
        std::fs::create_dir_all(folder)
            .and_then(|()| std::fs::write(NextRun::file(folder), text))
            .map_err(|error| {
                format!(
                    "couldn't keep what next left in {}: {error}",
                    folder.display()
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(label: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("mutastic-nextrun-{label}-{}", std::process::id()));
        std::fs::remove_dir_all(&folder).ok();
        folder
    }

    #[test]
    fn what_next_left_is_read_back_only_under_the_same_key() {
        let folder = folder("key");
        let run = NextRun {
            key: key("c0de", &["b", "a"], "skip", "-", "t"),
            uncaught: BTreeSet::from(["m00000002".to_string(), "m00000001".to_string()]),
        };
        assert_eq!(run.key, "c0de|a,b|skip|-|t", "operators sorted");
        run.write(&folder).unwrap();
        assert_eq!(NextRun::read(&folder, &run.key), Some(run.clone()));
        assert_eq!(NextRun::read(&folder, "c0de|a,b|all|-|t"), None);
        std::fs::remove_dir_all(&folder).ok();
    }

    #[test]
    fn nothing_left_reads_back_as_nothing_left() {
        let folder = folder("empty");
        let run = NextRun {
            key: "k".into(),
            uncaught: BTreeSet::new(),
        };
        run.write(&folder).unwrap();
        assert_eq!(NextRun::read(&folder, "k"), Some(run));
        std::fs::remove_dir_all(&folder).ok();
    }

    #[test]
    fn only_a_catch_or_a_skip_takes_a_mutant_out_of_the_queue() {
        let asked: BTreeSet<String> = ["m1", "m2", "m3", "m4", "m5", "m6", "m7"]
            .into_iter()
            .map(str::to_string)
            .collect();
        let left = left_uncaught(
            &asked,
            [
                ("m1", Status::Killed),
                ("m2", Status::Timeout),
                ("m3", Status::Ignored),
                ("m4", Status::Survived),
                ("m5", Status::NoCoverage),
                ("m6", Status::RuntimeError),
            ],
        );
        let left: Vec<&str> = left.iter().map(String::as_str).collect();
        assert_eq!(left, ["m4", "m5", "m6", "m7"], "m7 has no result");
    }

    /// A test written into a file's `mod tests` leaves the code under test as it was; a change to
    /// the code it tests doesn't.
    #[test]
    fn a_new_test_leaves_the_code_under_test_as_it_was() {
        let found = |text: &str| Crate {
            files: vec![crate::sites::File {
                path: PathBuf::from("src/lib.rs"),
                text: text.to_string(),
                unparsed: Vec::new(),
            }],
            ..Crate::default()
        };
        let code = "pub fn big(n: u32) -> bool {\n    n >= 100\n}\n";
        let tested = format!(
            "{code}\n#[cfg(test)]\nmod tests {{\n    #[test]\n    fn t() {{ assert!(super::big(100)); }}\n}}\n"
        );
        let more = tested.replace("}\n}\n", "}\n    #[test]\n    fn u() {}\n}\n");
        assert_ne!(tested, more);
        let hash = code_under_test(&found(&tested));
        assert_eq!(hash, code_under_test(&found(&more)));
        assert_ne!(
            hash,
            code_under_test(&found(&tested.replace(">= 100", "> 100")))
        );
    }
}
