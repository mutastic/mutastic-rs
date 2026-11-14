//! The lines a change touched, from git, for `--since`: since the merge-base of `HEAD` and a
//! commit, so `--since=main` on a branch is the branch's own change, uncommitted edits and new
//! files included, as the Java edition reads it. Lines a change only deleted aren't there: nothing
//! is left on them to mutate.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::failure::{Code, Failure};

/// The lines changed since a commit, by file (whole paths), and what they're since, as given.
#[derive(Clone, Debug, Default)]
pub struct ChangedLines {
    pub since: String,
    pub lines: BTreeMap<PathBuf, BTreeSet<usize>>,
}

impl ChangedLines {
    /// Whether `line` of `file` (a whole path) changed.
    pub fn touches(&self, file: &Path, line: usize) -> bool {
        let file = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
        self.lines
            .get(&file)
            .is_some_and(|lines| lines.contains(&line))
    }
}

/// The Rust lines changed in the repository holding `dir` since `since`, or since its default
/// branch for `""`.
pub fn changed_since(dir: &Path, since: &str) -> Result<ChangedLines, Failure> {
    let root = PathBuf::from(
        git(dir, &["rev-parse", "--show-toplevel"])
            .ok_or_else(|| Code::Git.with("--since needs a git repository"))?
            .trim(),
    );
    let reference = if since.is_empty() {
        default_branch(dir)
    } else {
        since.to_string()
    };
    let base = git(dir, &["merge-base", &reference, "HEAD"])
        .ok_or_else(|| Code::Git.with(format!("git knows no commit {reference}")))?
        .trim()
        .to_string();
    let diff = git(
        dir,
        &[
            "diff",
            "--unified=0",
            "--no-color",
            "--no-ext-diff",
            "--no-renames",
            &base,
            "--",
            "*.rs",
        ],
    )
    .ok_or_else(|| Code::Git.with("git couldn't diff"))?;
    let mut lines = BTreeMap::new();
    let mut current: Option<PathBuf> = None;
    for line in diff.lines() {
        if let Some(name) = line.strip_prefix("+++ ") {
            current = name.strip_prefix("b/").map(|name| real(&root.join(name)));
        } else if let (Some(file), Some(added)) = (&current, added(line)) {
            lines
                .entry(file.clone())
                .or_insert_with(BTreeSet::new)
                .extend(added);
        }
    }
    let untracked = git(
        dir,
        &[
            "ls-files",
            "--others",
            "--exclude-standard",
            "--full-name",
            "--",
            "*.rs",
        ],
    )
    .ok_or_else(|| Code::Git.with("git couldn't list new files"))?;
    for name in untracked.lines().filter(|name| !name.trim().is_empty()) {
        let file = real(&root.join(name));
        let count = std::fs::read_to_string(&file)
            .map(|text| text.lines().count())
            .unwrap_or(0);
        lines.insert(file, (1..=count).collect());
    }
    lines.retain(|_, changed: &mut BTreeSet<usize>| !changed.is_empty());
    Ok(ChangedLines {
        since: reference,
        lines,
    })
}

/// The lines a hunk header says it added: `@@ -12,3 +14,5 @@` adds 14 to 18.
fn added(line: &str) -> Option<std::ops::Range<usize>> {
    let rest = line.strip_prefix("@@ ")?;
    let new = rest.split_whitespace().find(|part| part.starts_with('+'))?;
    let new = &new[1..];
    let (start, count) = match new.split_once(',') {
        Some((start, count)) => (start.parse().ok()?, count.parse().ok()?),
        None => (new.parse().ok()?, 1),
    };
    Some(start..start + count)
}

/// `origin/HEAD`, as the remote names its default branch, else `main`, else `master`.
fn default_branch(dir: &Path) -> String {
    ["origin/HEAD", "main", "master"]
        .into_iter()
        .find(|candidate| git(dir, &["rev-parse", "--verify", "--quiet", candidate]).is_some())
        .unwrap_or("main")
        .to_string()
}

fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// What git prints, or `None` when it fails.
pub fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hunk_says_which_lines_it_added() {
        assert_eq!(added("@@ -12,3 +14,5 @@ fn f() {"), Some(14..19));
        assert_eq!(added("@@ -12 +14 @@"), Some(14..15));
        assert_eq!(added("@@ -12,3 +11,0 @@"), Some(11..11));
        assert_eq!(added("+    let x = 1;"), None);
    }
}
