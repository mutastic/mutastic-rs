//! `.mutastic/ignores.yaml`: the mutants a project has decided not to test, written by `mutastic
//! ignore` and committed, so each decision is a diff someone reviews; and
//! `.mutastic/proposals.yaml`, the ignores proposed, by a coding agent mostly, waiting for a person.
//! The same files, in the same shape, as the Java edition's:
//!
//! ```yaml
//! ignores:
//!   - id: m855c8e3c
//!     at: 'pricing::Pricing::discount, line 26'
//!     change: '`>=` becomes `>`'
//!     code: 'if total >= 100 {'
//!     reason: 'totals are rounded to tens here'
//!     kind: unreachable
//! ```
//!
//! `at` and `change` are for the reader; `code` is the line the decision was made on, and once that
//! line changes, the decision no longer holds and the mutant runs again.

use std::path::{Path, PathBuf};

/// The sorts of decision, as the Java edition has them: the change can't be told apart, its case
/// can't happen in practice, it only feeds logs or metrics, the code is on its way out, or the risk
/// is accepted.
pub const KINDS: &[&str] = &[
    "equivalent",
    "unreachable",
    "telemetry",
    "deprecated",
    "accepted-risk",
];

pub const FILE: &str = ".mutastic/ignores.yaml";
pub const PROPOSALS: &str = ".mutastic/proposals.yaml";

const HEADER: &[&str] = &[
    "# Mutants this project has decided not to test, written by `mutastic ignore`. Commit it:",
    "# each entry is a decision to review. One whose code has changed no longer holds; remove",
    "# those with `mutastic ignore --prune`.",
    "ignores:",
];

const PROPOSALS_HEADER: &[&str] = &[
    "# Ignores proposed and waiting for a person, written by `mutastic ignore --propose`: until",
    "# one is accepted (`mutastic ignore --accept ID`), its mutant still counts as missed.",
    "proposals:",
];

/// One decision, with what a reader needs to review it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub at: String,
    pub change: String,
    pub code: String,
    pub reason: String,
    pub kind: String,
}

impl Entry {
    /// `user: unreachable: totals are rounded (ignored)`: the skip reason its mutant gets.
    pub fn skip_reason(&self) -> String {
        let kind = if self.kind.is_empty() {
            String::new()
        } else {
            format!("{}: ", self.kind)
        };
        format!("user: {kind}{} (ignored)", self.reason)
    }

    /// Whether the decision still holds for the line as it is now.
    pub fn holds_for(&self, line: &str) -> bool {
        self.code == line.trim()
    }
}

pub fn read(dir: &Path) -> Result<Vec<Entry>, String> {
    read_file(&dir.join(FILE), FILE)
}

pub fn proposed(dir: &Path) -> Result<Vec<Entry>, String> {
    read_file(&dir.join(PROPOSALS), PROPOSALS)
}

pub fn write(dir: &Path, entries: &[Entry]) -> Result<(), String> {
    write_file(&dir.join(FILE), entries, HEADER)
}

/// Writes the proposals; with none waiting, there's no file, and nothing to commit.
pub fn propose(dir: &Path, entries: &[Entry]) -> Result<(), String> {
    let file = dir.join(PROPOSALS);
    if entries.is_empty() {
        if file.exists() {
            std::fs::remove_file(&file).map_err(|error| format!("{PROPOSALS}: {error}"))?;
        }
        return Ok(());
    }
    write_file(&file, entries, PROPOSALS_HEADER)
}

fn read_file(file: &PathBuf, name: &str) -> Result<Vec<Entry>, String> {
    let Ok(text) = std::fs::read_to_string(file) else {
        return Ok(Vec::new());
    };
    let mut entries = Vec::new();
    let mut fields: Vec<(String, String)> = Vec::new();
    for raw in text.lines() {
        let mut line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line == "ignores:" || line == "proposals:" {
            continue;
        }
        if let Some(rest) = line.strip_prefix("- ") {
            add(&mut entries, std::mem::take(&mut fields), name)?;
            line = rest;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(format!("{name}: can't read the line `{raw}`"));
        };
        fields.push((key.trim().to_string(), unquoted(value.trim())));
    }
    add(&mut entries, fields, name)?;
    Ok(entries)
}

fn add(entries: &mut Vec<Entry>, fields: Vec<(String, String)>, name: &str) -> Result<(), String> {
    if fields.is_empty() {
        return Ok(());
    }
    let get = |key: &str| {
        fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, value)| value.clone())
    };
    let (Some(id), Some(reason)) = (get("id"), get("reason")) else {
        return Err(format!("{name}: every entry needs an id and a reason"));
    };
    entries.push(Entry {
        id,
        at: get("at").unwrap_or_default(),
        change: get("change").unwrap_or_default(),
        code: get("code").unwrap_or_default(),
        reason,
        kind: get("kind").unwrap_or_default(),
    });
    Ok(())
}

fn write_file(file: &Path, entries: &[Entry], header: &[&str]) -> Result<(), String> {
    let mut lines: Vec<String> = header.iter().map(|line| line.to_string()).collect();
    for entry in entries {
        lines.push(format!("  - id: {}", entry.id));
        lines.push(format!("    at: {}", quoted(&entry.at)));
        lines.push(format!("    change: {}", quoted(&entry.change)));
        lines.push(format!("    code: {}", quoted(&entry.code)));
        lines.push(format!("    reason: {}", quoted(&entry.reason)));
        if !entry.kind.is_empty() {
            lines.push(format!("    kind: {}", entry.kind));
        }
    }
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(file, lines.join("\n") + "\n")
        .map_err(|error| format!("couldn't write {}: {error}", file.display()))
}

/// In single quotes, as YAML has them: a quote inside is doubled.
fn quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn unquoted(value: &str) -> String {
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        value[1..value.len() - 1].replace("''", "'")
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_written_and_read_back_as_the_java_edition_writes_them() {
        let dir = std::env::temp_dir().join(format!("mutastic-ignores-{}", std::process::id()));
        let entry = Entry {
            id: "m855c8e3c".into(),
            at: "pricing::Pricing::discount, line 26".into(),
            change: "`>=` becomes `>`".into(),
            code: "if total >= 100 {".into(),
            reason: "totals can't be 100: it's the shop's".into(),
            kind: "unreachable".into(),
        };
        write(&dir, std::slice::from_ref(&entry)).unwrap();
        let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(
            text.ends_with(
                "ignores:\n  - id: m855c8e3c\n    at: 'pricing::Pricing::discount, line 26'\n    \
                 change: '`>=` becomes `>`'\n    code: 'if total >= 100 {'\n    \
                 reason: 'totals can''t be 100: it''s the shop''s'\n    kind: unreachable\n"
            ),
            "{text}"
        );
        assert_eq!(read(&dir).unwrap(), vec![entry.clone()]);
        assert_eq!(
            entry.skip_reason(),
            "user: unreachable: totals can't be 100: it's the shop's (ignored)"
        );
        assert!(entry.holds_for("    if total >= 100 {  "));
        assert!(!entry.holds_for("if total > 100 {"));
        propose(&dir, &[]).unwrap();
        assert!(proposed(&dir).unwrap().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
