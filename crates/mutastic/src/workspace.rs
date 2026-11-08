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
    Ok(Project {
        given: dir.to_string(),
        package_dir: relative(&manifest_dir(package)),
        workspace: workspace_real,
        target,
        package: package["name"].as_str().unwrap_or_default().to_string(),
        roots,
        crate_name,
        edition: package["edition"].as_str().unwrap_or("2021").to_string(),
    })
}

pub fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())
}

/// Files whose copy has other text than the original: the rewritten sources.
pub type Overrides = std::collections::BTreeMap<PathBuf, String>;

/// Brings the copy up to date with the workspace, writing only what changed, with `overrides` in
/// place of the originals.
pub fn sync(project: &Project, overrides: &Overrides) -> Result<(), String> {
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

pub fn write_if_changed(path: &Path, text: &str) -> std::io::Result<()> {
    if std::fs::read_to_string(path).ok().as_deref() != Some(text) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)?;
    }
    Ok(())
}
