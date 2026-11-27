//! `.mutastic/baseline.yaml`: the mutants a project missed when its baseline was taken, by id,
//! written by `mutastic baseline` and committed. A miss that isn't in it is new: `gate` holds only
//! new misses, `next` offers them first, and `run --fail-on-new` fails only on them. It shrinks
//! with `baseline --prune` as tests catch its mutants, and is never rewritten to take in new
//! misses, or it would hide them. The same file, in the same shape, as the Java edition's:
//!
//! ```yaml
//! operators: defaults
//! baseline:
//!   - id: mb2f8ae57
//!     at: 'pricing::Pricing::discount, line 26'
//!     change: '`>=` becomes `>`'
//! ```
//!
//! Ids alone decide: a mutant's id is a hash of what it is, without its line, so it stays the same
//! while code around it moves. `at` and `change` are for whoever reads a diff of it. One entry an
//! id, in order of function and then id, so a line moving doesn't reorder it.
//!
//! `operators:` is the set the run that took it chose, as `--operators` or `mutastic.yaml` chose
//! it: a mutant only another set makes isn't in a baseline taken without it, so `--prune` under
//! another set would take it for gone. A baseline from before it was written has none.

use std::collections::BTreeSet;
use std::path::Path;

/// The file, in the project's directory.
pub const FILE: &str = ".mutastic/baseline.yaml";

const HEADER: &[&str] = &[
    "# The mutants this project missed when its baseline was taken, written by `mutastic baseline`.",
    "# Commit it. A miss that isn't here is new: the gate holds only new misses. As tests",
    "# catch these, `mutastic baseline --prune` drops them; it never takes in new misses.",
];

/// One mutant, with where it is and what it changes, for the reader.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub at: String,
    pub change: String,
}

/// The id an entry's first line names: `  - id: mb2f8ae57`.
fn id_on(line: &str) -> Option<&str> {
    let id = line.trim().strip_prefix("- id:")?.trim();
    let hex = id.strip_prefix('m')?;
    (hex.len() == 8
        && hex
            .bytes()
            .all(|digit| matches!(digit, b'0'..=b'9' | b'a'..=b'f')))
    .then_some(id)
}

/// The ids in a baseline's text.
pub fn ids_in(text: &str) -> BTreeSet<String> {
    text.lines().filter_map(id_on).map(str::to_string).collect()
}

/// The ids in the project's baseline; none when it has none.
pub fn ids(dir: &Path) -> BTreeSet<String> {
    std::fs::read_to_string(dir.join(FILE))
        .map(|text| ids_in(&text))
        .unwrap_or_default()
}

/// The baseline as it was at a commit, as git has it: what a gated task started from. None when
/// it had none.
pub fn ids_at(dir: &Path, commit: &str) -> BTreeSet<String> {
    crate::since::git(dir, &["show", &format!("{commit}:./{FILE}")])
        .map(|text| ids_in(&text))
        .unwrap_or_default()
}

/// The operators the baseline was taken under, `defaults` or `stronger,-stream`: none for a
/// baseline from before they were written, or none at all.
pub fn operators(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(FILE)).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("operators: "))
        .map(|operators| operators.trim().to_string())
        .filter(|operators| !operators.is_empty())
}

pub fn exists(dir: &Path) -> bool {
    dir.join(FILE).is_file()
}

/// Writes the entries, taken under `operators`, replacing the file.
pub fn write(dir: &Path, operators: &str, entries: &[Entry]) -> Result<(), String> {
    let mut lines: Vec<String> = HEADER.iter().map(|line| line.to_string()).collect();
    lines.push(format!("operators: {operators}"));
    lines.push("baseline:".to_string());
    for entry in entries {
        lines.push(format!("  - id: {}", entry.id));
        lines.push(format!("    at: {}", quoted(&entry.at)));
        lines.push(format!("    change: {}", quoted(&entry.change)));
    }
    write_lines(&dir.join(FILE), &lines)
}

/// Drops the entries whose ids are in `caught`, each whole, and keeps the rest as they were: how
/// many it dropped.
pub fn prune(dir: &Path, caught: &BTreeSet<String>) -> Result<usize, String> {
    let file = dir.join(FILE);
    let text = std::fs::read_to_string(&file)
        .map_err(|error| format!("couldn't read {}: {error}", file.display()))?;
    let mut kept = Vec::new();
    let mut dropped = 0;
    let mut dropping = false;
    for line in text.lines() {
        if let Some(id) = id_on(line) {
            dropping = caught.contains(id);
            dropped += usize::from(dropping);
        } else if !line.starts_with("    ") {
            dropping = false;
        }
        if !dropping {
            kept.push(line.to_string());
        }
    }
    write_lines(&file, &kept)?;
    Ok(dropped)
}

/// In single quotes, as YAML has them: a quote inside is doubled.
fn quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn write_lines(file: &Path, lines: &[String]) -> Result<(), String> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(file, lines.join("\n") + "\n")
        .map_err(|error| format!("couldn't write {}: {error}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(label: &str) -> std::path::PathBuf {
        let folder =
            std::env::temp_dir().join(format!("mutastic-baseline-{label}-{}", std::process::id()));
        std::fs::remove_dir_all(&folder).ok();
        folder
    }

    #[test]
    fn entries_are_written_as_the_java_edition_writes_them_and_pruned_whole() {
        let dir = folder("write");
        let entry = |id: &str, at: &str| Entry {
            id: id.into(),
            at: at.into(),
            change: "`>=` becomes `>`".into(),
        };
        write(
            &dir,
            "stronger",
            &[
                entry("mb2f8ae57", "pricing::Pricing::discount, line 26"),
                entry("m7d92063c", "pricing::Pricing::shipping, line 34"),
            ],
        )
        .unwrap();
        let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert_eq!(
            text,
            "# The mutants this project missed when its baseline was taken, written by `mutastic baseline`.\n\
             # Commit it. A miss that isn't here is new: the gate holds only new misses. As tests\n\
             # catch these, `mutastic baseline --prune` drops them; it never takes in new misses.\n\
             operators: stronger\n\
             baseline:\n  \
             - id: mb2f8ae57\n    at: 'pricing::Pricing::discount, line 26'\n    change: '`>=` becomes `>`'\n  \
             - id: m7d92063c\n    at: 'pricing::Pricing::shipping, line 34'\n    change: '`>=` becomes `>`'\n"
        );
        assert_eq!(
            ids(&dir),
            BTreeSet::from(["mb2f8ae57".to_string(), "m7d92063c".to_string()])
        );
        assert_eq!(operators(&dir).as_deref(), Some("stronger"));
        let dropped = prune(&dir, &BTreeSet::from(["mb2f8ae57".to_string()])).unwrap();
        assert_eq!(dropped, 1);
        let pruned = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(
            pruned.ends_with("operators: stronger\nbaseline:\n  - id: m7d92063c\n    at: 'pricing::Pricing::shipping, line 34'\n    change: '`>=` becomes `>`'\n"),
            "{pruned}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn only_an_id_line_names_an_id() {
        assert_eq!(
            ids_in("baseline:\n  - id: mb2f8ae57\n    at: '- id: m00000000'\n  - id: nothex\n"),
            BTreeSet::from(["mb2f8ae57".to_string()])
        );
        assert!(ids(&folder("none")).is_empty());
    }
}
