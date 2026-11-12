//! The project under test, read from `cargo metadata`, and mutastic's copy of it.
//!
//! The copy lives in `target/mutastic/copy`, beside its own target directory, so the user's
//! `target/` and its lock are never used and the copy stays warm from run to run. A file in the
//! copy is written only when its bytes change, because Cargo judges freshness by mtime.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Project {
    /// The directory mutastic was pointed at, as given, for the commands it prints.
    pub given: String,
    pub workspace: PathBuf,
    pub target: PathBuf,
    pub package: String,
    /// The package's directory, relative to the workspace.
    pub package_dir: PathBuf,
    /// Crate roots of the package's library and binaries, relative to the workspace.
    pub roots: Vec<PathBuf>,
    /// The package's library's crate name (`semver`), or its first binary's.
    pub crate_name: String,
    /// The package's Rust edition, `2021`.
    pub edition: String,
    /// The features a plain build turns on: the default ones and what they enable.
    pub features: std::collections::BTreeSet<String>,
    /// The directories of the workspace's packages, relative to the workspace.
    pub members: std::collections::BTreeSet<PathBuf>,
}

impl Project {
    pub fn mutastic_dir(&self) -> PathBuf {
        self.target.join("mutastic")
    }

    pub fn copy_dir(&self) -> PathBuf {
        self.mutastic_dir().join("copy")
    }

    pub fn build_dir(&self) -> PathBuf {
        self.mutastic_dir().join("target")
    }
}

/// Reads the package in `dir` (or the only package of the workspace there).
pub fn read(dir: &str) -> Result<Project, String> {
    let output = Command::new(cargo())
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(dir)
        .output()
        .map_err(|error| format!("couldn't run cargo: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{dir} isn't a Cargo project: {}",
            stderr.lines().last().unwrap_or_default()
        ));
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("couldn't read cargo metadata: {error}"))?;
    let workspace = PathBuf::from(metadata["workspace_root"].as_str().unwrap_or(dir));
    let target = PathBuf::from(metadata["target_directory"].as_str().unwrap_or("target"));
    let here = std::fs::canonicalize(dir).map_err(|error| format!("{dir}: {error}"))?;
    let packages = metadata["packages"].as_array().cloned().unwrap_or_default();
    let manifest_dir = |package: &Value| {
        PathBuf::from(package["manifest_path"].as_str().unwrap_or_default())
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default()
    };
    let package = packages
        .iter()
        .find(|package| {
            std::fs::canonicalize(manifest_dir(package)).is_ok_and(|found| found == here)
        })
        .or_else(|| (packages.len() == 1).then(|| &packages[0]))
        .ok_or_else(|| {
            let names: Vec<&str> = packages
                .iter()
                .filter_map(|package| package["name"].as_str())
                .collect();
            format!(
                "{dir} is a workspace: run mutastic in one of its packages ({})",
                names.join(", ")
            )
        })?;
    let workspace_real = std::fs::canonicalize(&workspace).unwrap_or(workspace.clone());
    let relative = |path: &Path| {
        let real = std::fs::canonicalize(path).unwrap_or(path.to_path_buf());
        real.strip_prefix(&workspace_real)
            .map(Path::to_path_buf)
            .unwrap_or(real)
    };
    let mut roots = Vec::new();
    let mut crate_name = String::new();
    for target in package["targets"].as_array().cloned().unwrap_or_default() {
        let kinds: Vec<&str> = target["kind"]
            .as_array()
            .map(|kinds| kinds.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if kinds.contains(&"proc-macro") {
            return Err(format!(
                "{} is a proc-macro crate, which mutastic can't mutate yet",
                package["name"].as_str().unwrap_or_default()
            ));
        }
        let is_lib = kinds.iter().any(|kind| kind.ends_with("lib"));
        if is_lib || kinds.contains(&"bin") {
            let src = PathBuf::from(target["src_path"].as_str().unwrap_or_default());
            roots.push(relative(&src));
            if is_lib || crate_name.is_empty() {
                crate_name = target["name"]
                    .as_str()
                    .unwrap_or_default()
                    .replace('-', "_");
            }
        }
    }
    let members = packages
        .iter()
        .map(|member| relative(&manifest_dir(member)))
        .collect();
    Ok(Project {
        given: dir.to_string(),
        package_dir: relative(&manifest_dir(package)),
        workspace: workspace_real,
        target,
        package: package["name"].as_str().unwrap_or_default().to_string(),
        roots,
        crate_name,
        edition: package["edition"].as_str().unwrap_or("2021").to_string(),
        features: crate::cfg::default_features(&package["features"]),
        members,
    })
}

/// Holds `dir`, mutastic's work directory for a package, for this process from its first write to
/// the copy until it exits. Two runs on one package at once would rewrite each other's copy and
/// build in the middle of a run, so a mutant could be judged by the other run's build: a false
/// catch. A second run waits for the first, and says so. The lock is the process's, taken once
/// however many times it builds there, since a lock asked for again from the same process would
/// wait on itself.
fn hold(dir: &Path) -> Result<(), String> {
    static HELD: std::sync::OnceLock<
        std::sync::Mutex<std::collections::BTreeMap<PathBuf, std::fs::File>>,
    > = std::sync::OnceLock::new();
    let mut held = HELD
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| "a run's lock was poisoned".to_string())?;
    if held.contains_key(dir) {
        return Ok(());
    }
    std::fs::create_dir_all(dir)
        .map_err(|error| format!("couldn't make {}: {error}", dir.display()))?;
    let path = dir.join("lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|error| format!("couldn't open {}: {error}", path.display()))?;
    match file.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            stderr_line!(
                "mutastic: waiting for another mutastic run on this package to finish ({})",
                path.display()
            );
            file.lock()
                .map_err(|error| format!("couldn't lock {}: {error}", path.display()))?;
        }
        Err(std::fs::TryLockError::Error(error)) => {
            return Err(format!("couldn't lock {}: {error}", path.display()));
        }
    }
    held.insert(dir.to_path_buf(), file);
    Ok(())
}

pub fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())
}

/// Files whose copy has other text than the original: the rewritten sources.
pub type Overrides = std::collections::BTreeMap<PathBuf, String>;

/// Brings the copy up to date with the workspace, writing only what changed, with `overrides` in
/// place of the originals.
pub fn sync(project: &Project, overrides: &Overrides) -> Result<(), String> {
    hold(&project.mutastic_dir())?;
    let copy = project.copy_dir();
    let mut kept = std::collections::BTreeSet::new();
    copy_tree(
        &project.workspace,
        &copy,
        Path::new(""),
        project,
        overrides,
        &mut kept,
    )
    .map_err(|error| format!("couldn't copy the workspace: {error}"))?;
    remove_stale(&copy, Path::new(""), &kept).ok();
    // The copy is its own workspace root, so Cargo doesn't look above it.
    let manifest = copy.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).unwrap_or_default();
    if !text.lines().any(|line| line.trim() == "[workspace]") {
        write_if_changed(&manifest, &format!("{text}\n[workspace]\n"))
            .map_err(|error| error.to_string())?;
    }
    stand_alone(&copy, project, &kept).map_err(|error| error.to_string())
}

/// Gives each package in the copy that the workspace excludes an empty `[workspace]`, as the copy's
/// root has: in the workspace it stands alone, but Cargo, finding a workspace that excludes a
/// package, looks further up, and above the copy is the workspace itself, which doesn't exclude
/// its copy. A crate's tests that run Cargo on a fixture of its own then fail in the copy, and only
/// there. A package of a workspace nested in it is left alone: Cargo finds that workspace first.
fn stand_alone(
    copy: &Path,
    project: &Project,
    kept: &std::collections::BTreeSet<PathBuf>,
) -> std::io::Result<()> {
    let mut manifests = Vec::new();
    for manifest in kept {
        let dir = match manifest.parent() {
            Some(dir) if manifest.ends_with("Cargo.toml") && !dir.as_os_str().is_empty() => dir,
            _ => continue,
        };
        let text = std::fs::read_to_string(copy.join(manifest))?;
        manifests.push((dir.to_path_buf(), text));
    }
    let has = |text: &str, table: &str| text.lines().any(|line| line.trim() == table);
    let workspaces: std::collections::BTreeSet<&Path> = manifests
        .iter()
        .filter(|(_, text)| has(text, "[workspace]"))
        .map(|(dir, _)| dir.as_path())
        .collect();
    for (dir, text) in &manifests {
        let nested = dir
            .ancestors()
            .skip(1)
            .any(|above| workspaces.contains(above));
        let excluded = !project.members.contains(dir) && !nested;
        if excluded && has(text, "[package]") && !has(text, "[workspace]") {
            let file = copy.join(dir).join("Cargo.toml");
            write_if_changed(&file, &format!("{text}\n[workspace]\n"))?;
        }
    }
    Ok(())
}

fn copy_tree(
    from: &Path,
    to: &Path,
    relative: &Path,
    project: &Project,
    overrides: &Overrides,
    kept: &mut std::collections::BTreeSet<PathBuf>,
) -> std::io::Result<()> {
    std::fs::create_dir_all(to.join(relative))?;
    for entry in std::fs::read_dir(from.join(relative))? {
        let entry = entry?;
        let name = entry.file_name();
        let path = relative.join(&name);
        let source = from.join(&path);
        let top_target = name == "target" && relative.as_os_str().is_empty();
        if name == ".git" || source == project.target || top_target {
            continue;
        }
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_tree(from, to, &path, project, overrides, kept)?;
        } else if kind.is_file() {
            kept.insert(path.clone());
            let bytes = match overrides.get(&path) {
                Some(text) => text.as_bytes().to_vec(),
                None => std::fs::read(&source)?,
            };
            let destination = to.join(&path);
            if std::fs::read(&destination).ok().as_deref() != Some(&bytes[..]) {
                std::fs::write(&destination, &bytes)?;
            }
        }
    }
    Ok(())
}

fn remove_stale(
    copy: &Path,
    relative: &Path,
    kept: &std::collections::BTreeSet<PathBuf>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(copy.join(relative))? {
        let entry = entry?;
        let path = relative.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            remove_stale(copy, &path, kept)?;
        } else if !kept.contains(&path) && path != Path::new("Cargo.lock") {
            std::fs::remove_file(copy.join(&path))?;
        }
    }
    Ok(())
}

/// A relative path as mutastic shows it and keys it, `src/lib.rs`, with `/` between its parts on
/// every platform: cards, places and reports read the same on Windows as elsewhere, and Windows
/// takes `/` in a path as it takes `\\`. Elsewhere `\\` can be part of a name, so it's left as it is.
pub fn slashed(path: &Path) -> String {
    let text = path.display().to_string();
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text
    }
}

pub fn write_if_changed(path: &Path, text: &str) -> std::io::Result<()> {
    if std::fs::read_to_string(path).ok().as_deref() != Some(text) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A workspace that keeps a fixture out of it, as mutastic's own does: in the copy, Cargo
    /// takes the fixture for a package on its own, as it does in the workspace.
    #[test]
    fn an_excluded_package_stands_alone_in_the_copy() {
        let root = std::env::temp_dir().join(format!("mutastic-excluded-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let write = |path: &str, text: &str| {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "Cargo.toml",
            "[workspace]\nmembers = [\"app\"]\nexclude = [\"fixtures\"]\nresolver = \"2\"\n",
        );
        let package = |name: &str| {
            format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n")
        };
        write("app/Cargo.toml", &package("app"));
        write("app/src/lib.rs", "");
        write("fixtures/inner/Cargo.toml", &package("inner"));
        write("fixtures/inner/src/lib.rs", "");
        // A workspace of its own among the fixtures, whose packages are its members.
        write(
            "fixtures/nested/Cargo.toml",
            "[workspace]\nmembers = [\"part\"]\nresolver = \"2\"\n",
        );
        write("fixtures/nested/part/Cargo.toml", &package("part"));
        write("fixtures/nested/part/src/lib.rs", "");

        let project = read(&root.join("app").display().to_string()).unwrap();
        sync(&project, &Overrides::new()).unwrap();
        let metadata = |dir: PathBuf| {
            Command::new(cargo())
                .args(["metadata", "--no-deps", "--format-version", "1"])
                .current_dir(dir)
                .output()
                .unwrap()
        };
        let inner = metadata(project.copy_dir().join("fixtures/inner"));
        let nested = metadata(project.copy_dir().join("fixtures/nested"));
        let app = std::fs::read_to_string(project.copy_dir().join("app/Cargo.toml")).unwrap();
        std::fs::remove_dir_all(&root).ok();
        assert!(
            nested.status.success() && String::from_utf8_lossy(&nested.stdout).contains("\"part\""),
            "a nested workspace keeps its members: {}",
            String::from_utf8_lossy(&nested.stderr)
        );
        assert!(
            inner.status.success(),
            "{}",
            String::from_utf8_lossy(&inner.stderr)
        );
        assert!(
            !app.contains("[workspace]"),
            "a member stays a member: {app}"
        );
    }
}
