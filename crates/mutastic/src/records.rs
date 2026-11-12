//! The run records of a package's `target/mutastic`, one for each set of operators, as the Java
//! edition keeps them, so that a run under another set doesn't leave the next run under the first
//! with nothing to compare with.
//!
//! `last-run.tsv` is the latest run, under whatever operators. When a run replaces it, it moves to
//! `last-runs/<hash of its operators>.tsv` unless the new run is under the same ones. A file's name
//! is only where to look: what's read is compared by what the record inside says it's of
//! ([`RunRecord::compares_with`]), so a stale, renamed or unreadable file is never taken for a run
//! it isn't.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::record::RunRecord;

/// Records kept beside the latest: sets of operators a package goes back to are few, and each
/// record is a line per mutant, so a run with one-off sets can't fill the disk. The longest unused
/// go first.
pub const KEPT_BESIDE_THE_LATEST: usize = 7;

/// The records in one folder.
pub struct RunRecords {
    latest: PathBuf,
    others: PathBuf,
}

impl RunRecords {
    /// The records kept in `folder`: `target/mutastic`, or a share's own folder.
    pub fn new(folder: &Path) -> RunRecords {
        RunRecords {
            latest: folder.join("last-run.tsv"),
            others: folder.join("last-runs"),
        }
    }

    /// The last run that `record` can be compared with, if there's one.
    pub fn comparable_to(&self, record: &RunRecord) -> Option<RunRecord> {
        [self.latest.clone(), self.file_for(&record.operators)]
            .iter()
            .filter_map(|file| RunRecord::read(file))
            .find(|previous| record.compares_with(previous))
    }

    /// Makes `record` the latest, keeping the one it replaces if that was under other operators.
    pub fn keep(&self, record: &RunRecord) -> std::io::Result<()> {
        if let Some(previous) = RunRecord::read(&self.latest) {
            if record.same_project_as(&previous)
                && !previous.operators.is_empty()
                && previous.operators != record.operators
            {
                // Moved rather than written, so the file keeps the time of the run it's of.
                std::fs::create_dir_all(&self.others)?;
                std::fs::rename(&self.latest, self.file_for(&previous.operators))?;
            }
        }
        record.write(&self.latest)?;
        match std::fs::remove_file(self.file_for(&record.operators)) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
        self.drop_the_oldest()
    }

    fn drop_the_oldest(&self) -> std::io::Result<()> {
        let files = self.others_oldest_first();
        let extra = files.len().saturating_sub(KEPT_BESIDE_THE_LATEST);
        for file in &files[..extra] {
            std::fs::remove_file(file)?;
        }
        Ok(())
    }

    /// The records set aside, the longest unused first, by when each was last written.
    fn others_oldest_first(&self) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(&self.others) else {
            return Vec::new();
        };
        let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "tsv"))
            .map(|path| {
                let modified = std::fs::metadata(&path)
                    .and_then(|metadata| metadata.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                (modified, path)
            })
            .collect();
        files.sort();
        files.into_iter().map(|(_, path)| path).collect()
    }

    /// A set's ids can be as long as there are operators, and a file name can't.
    fn file_for(&self, operators: &str) -> PathBuf {
        let hash: String = Sha256::digest(operators.as_bytes())[..8]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        self.others.join(format!("{hash}.tsv"))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn record(operators: &str, score: f64) -> RunRecord {
        RunRecord {
            project: "/code/shop".into(),
            operators: operators.into(),
            code: "c".into(),
            tool: "t".into(),
            score: Some(score),
            results: BTreeMap::new(),
        }
    }

    fn folder(label: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("mutastic-records-{label}-{}", std::process::id()));
        std::fs::remove_dir_all(&folder).ok();
        folder
    }

    #[test]
    fn a_run_under_other_operators_leaves_the_first_sets_record() {
        let folder = folder("sets");
        let records = RunRecords::new(&folder);
        records.keep(&record("defaults", 80.0)).unwrap();
        records.keep(&record("stronger", 60.0)).unwrap();

        let defaults = records.comparable_to(&record("defaults", 0.0));
        let stronger = records.comparable_to(&record("stronger", 0.0));
        let other = records.comparable_to(&record("all", 0.0));
        std::fs::remove_dir_all(&folder).ok();
        assert_eq!(defaults.map(|r| r.score), Some(Some(80.0)));
        assert_eq!(stronger.map(|r| r.score), Some(Some(60.0)));
        assert_eq!(other, None);
    }

    #[test]
    fn only_so_many_sets_are_kept_beside_the_latest() {
        let folder = folder("kept");
        let records = RunRecords::new(&folder);
        for set in 0..KEPT_BESIDE_THE_LATEST + 3 {
            records.keep(&record(&format!("set{set}"), 1.0)).unwrap();
            // Runs this close together can share a modified time: each set aside gets its own.
            if set > 0 {
                let aside = records.file_for(&format!("set{}", set - 1));
                let when = std::time::UNIX_EPOCH + std::time::Duration::from_secs(set as u64);
                if let Ok(file) = std::fs::File::options().write(true).open(&aside) {
                    file.set_modified(when).unwrap();
                }
            }
        }
        let set_aside = std::fs::read_dir(folder.join("last-runs")).unwrap().count();
        let first = records.comparable_to(&record("set0", 0.0));
        let last =
            records.comparable_to(&record(&format!("set{}", KEPT_BESIDE_THE_LATEST + 2), 0.0));
        std::fs::remove_dir_all(&folder).ok();
        assert_eq!(set_aside, KEPT_BESIDE_THE_LATEST);
        assert_eq!(first, None, "the longest unused set went first");
        assert!(last.is_some(), "the latest is kept");
    }
}
