//! Where a command runs: the directory it names, checked before anything runs there, and the
//! package that holds it, found as Cargo finds one. When it's wrong, mutastic says so in the
//! person's own words: what they typed, where it looked, and last, the command to run instead.

use std::path::{Path, PathBuf};

use crate::edition::{self, CARGO_BUILD, JVM_BUILDS};
use crate::failure::{Code, Failure};
use crate::workspace::{self, Metadata};

/// How many of a workspace's packages are shown as commands; past this many, by name.
const LISTED: usize = 10;

/// Directories never offered as the one meant: build output and dependencies.
const NOT_OFFERED: &[&str] = &["target", "node_modules"];

/// The command as typed, around its directory: the command and its words before the directory
/// (`run`, `verify m1a2b3c4d`), and its options. What mutastic offers instead is this line with
/// only the directory changed.
pub struct Line {
    command: String,
    options: Vec<String>,
}

impl Line {
    pub fn new(command: &str, options: &[String]) -> Line {
        Line {
            command: command.to_string(),
            options: options.to_vec(),
        }
    }

    /// The line for each directory, to copy, one a line.
    fn commands(&self, dirs: &[String]) -> String {
        edition::commands(&self.command, dirs, &self.options)
    }
}

/// The directory a command runs in, and what to say when it isn't the one typed.
#[derive(Debug, PartialEq, Eq)]
pub struct Located {
    pub dir: String,
    pub note: Option<String>,
}

/// The directory `given` names, as a command runs in it: `given` itself when it's a package's
/// directory, the package that holds it when it's below one, or the package of that name when
/// there's no such directory and it's a package's name exactly. Anything else is said, with the
/// command to run instead: no such directory, no project at all, or a workspace's root.
pub fn resolve(given: &str, line: &Line) -> Result<Located, Failure> {
    let cwd = std::env::current_dir()
        .and_then(std::fs::canonicalize)
        .map_err(|error| {
            Code::Other.with(format!("couldn't read the current directory: {error}"))
        })?;
    let unchanged = Ok(Located {
        dir: given.to_string(),
        note: None,
    });
    if !Path::new(given).is_dir() {
        return missing(given, line, &cwd);
    }
    let here = std::fs::canonicalize(given)
        .map_err(|error| Code::Other.with(format!("{given}: {error}")))?;
    if !any_above(&here, &[CARGO_BUILD]) {
        // Only `--edition=rust` brings a Java build here: Cargo says what it makes of it.
        return if any_above(&here, JVM_BUILDS) {
            unchanged
        } else {
            Err(no_project(given, &here, line))
        };
    }
    if is_package(&here) {
        return unchanged;
    }
    let metadata = Metadata::read(given)?;
    match metadata.enclosing(&here) {
        Some(dir) if dir == here => unchanged,
        Some(dir) => Ok(Located {
            dir: shown(given, &cwd, &dir),
            note: None,
        }),
        None if metadata.members().len() == 1 => unchanged,
        None => Err(not_a_package(given, &here, &metadata, line)),
    }
}

/// Whether `dir` or a directory above it has one of the build files named.
fn any_above(dir: &Path, builds: &[&str]) -> bool {
    dir.ancestors()
        .any(|above| builds.iter().any(|build| above.join(build).is_file()))
}

/// Whether `dir` is a package's own directory: its `Cargo.toml` has a `[package]`.
fn is_package(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join(CARGO_BUILD))
        .is_ok_and(|text| text.lines().any(|line| line.trim() == "[package]"))
}

/// `dir` as the person would type it where they are: from the current directory, or whole when
/// they gave a whole path.
fn shown(given: &str, cwd: &Path, dir: &Path) -> String {
    if Path::new(given).is_absolute() {
        return workspace::slashed(dir);
    }
    let relative = crate::config::relative(cwd, dir);
    if relative.as_os_str().is_empty() {
        ".".to_string()
    } else {
        workspace::slashed(&relative)
    }
}

/// No build file of either edition in `here` or above it.
fn no_project(given: &str, here: &Path, line: &Line) -> Failure {
    let place = if given == "." {
        "here".to_string()
    } else {
        format!("in {given}")
    };
    Code::NoProject.with(format!(
        "there's no project {place} for mutastic to test.\n\
         It looked in {} and every directory above it, for Cargo.toml, pom.xml, \
         build.gradle(.kts) or settings.gradle(.kts).\n\
         Go to your project, or name its directory:\n{}",
        here.display(),
        line.commands(&["<project directory>".to_string()])
    ))
}

/// `here` is in a workspace but in none of its packages: at its root, or in a directory of its
/// own. Each package, as the command to test it.
pub fn not_a_package(given: &str, here: &Path, metadata: &Metadata, line: &Line) -> Failure {
    let root = std::fs::canonicalize(&metadata.workspace).unwrap_or(metadata.workspace.clone());
    let what = match (given == ".", here == root) {
        (true, true) => "this is a workspace's root".to_string(),
        (false, true) => format!("{given} is a workspace's root"),
        (true, false) => "this directory is in a workspace, but in none of its packages".into(),
        (false, false) => format!("{given} is in a workspace, but in none of its packages"),
    };
    let cwd = std::env::current_dir()
        .and_then(std::fs::canonicalize)
        .unwrap_or_default();
    Code::WorkspaceRoot.with(format!(
        "{what}, and mutastic tests one package at a time.\n{}",
        packages("It", given, &cwd, &metadata.members(), line)
    ))
}

/// A workspace's packages, as commands to copy, or by name past [`LISTED`] of them.
fn packages(
    subject: &str,
    given: &str,
    cwd: &Path,
    members: &[(String, PathBuf)],
    line: &Line,
) -> String {
    let count = members.len();
    if count > LISTED {
        let names: Vec<&str> = members.iter().map(|(name, _)| name.as_str()).collect();
        return format!(
            "{subject} has {count} packages: {}. To test one, name its directory:\n{}",
            names.join(", "),
            line.commands(&["<package directory>".to_string()])
        );
    }
    let dirs: Vec<String> = members
        .iter()
        .map(|(_, dir)| shown(given, cwd, dir))
        .collect();
    format!(
        "{subject} has {count} packages. To test one:\n{}",
        line.commands(&dirs)
    )
}

/// `given` isn't a directory. A package's name typed alone runs that package, and says so; else
/// what's wrong, the one name close to it, if any, and the command.
fn missing(given: &str, line: &Line, cwd: &Path) -> Result<Located, Failure> {
    let path = Path::new(given);
    let base = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let typed_alone = base == Path::new(".") && !given.starts_with('.');
    let real_base = std::fs::canonicalize(&base).ok();
    let members = match &real_base {
        Some(real) if any_above(real, &[CARGO_BUILD]) => {
            Metadata::read(&base.display().to_string())
                .map(|metadata| metadata.members())
                .unwrap_or_default()
        }
        _ => Vec::new(),
    };
    if typed_alone {
        if let Some((_, dir)) = members.iter().find(|(name, _)| name == given) {
            let dir = shown(given, cwd, dir);
            let place = if dir == "." {
                "here, so mutastic runs here".to_string()
            } else {
                format!("in {dir}, so mutastic runs there")
            };
            return Ok(Located {
                note: Some(format!("mutastic: {given} is the package {place}")),
                dir,
            });
        }
    }
    let headline = if path.is_absolute() {
        format!("there's no directory {given}.")
    } else {
        format!("there's no directory {given} in {}.", cwd.display())
    };
    let name = path
        .file_name()
        .map_or_else(|| given.to_string(), |n| n.to_string_lossy().into_owned());
    let offered = offer(&name, &base, &members, given, cwd);
    let (workspace, package) = if base == Path::new(".") {
        ("This workspace", "The package here")
    } else {
        ("The workspace there", "The package there")
    };
    let rest = match offered {
        Some((what, dir)) => format!("There's {what}. To test it:\n{}", line.commands(&[dir])),
        None if members.len() > 1 => packages(workspace, given, cwd, &members, line),
        None => match members.first() {
            Some((name, dir)) => {
                let dir = shown(given, cwd, dir);
                let place = if dir == "." {
                    String::new()
                } else {
                    format!(", in {dir}")
                };
                format!(
                    "{package} is {name}{place}. To test it:\n{}",
                    line.commands(&[dir])
                )
            }
            None => format!(
                "Name a directory that holds a project:\n{}",
                line.commands(&["<project directory>".to_string()])
            ),
        },
    };
    Err(Code::NoDirectory.with(format!("{headline}\n{rest}")))
}

/// The one package or directory whose name is close to `name`: what to call it, and its
/// directory as the person would type it. Packages are offered before directories.
fn offer(
    name: &str,
    base: &Path,
    members: &[(String, PathBuf)],
    given: &str,
    cwd: &Path,
) -> Option<(String, String)> {
    let mut candidates: Vec<(String, String, String)> = Vec::new();
    for (package, dir) in members {
        let dir = shown(given, cwd, dir);
        let what = if dir == *package {
            format!("a package called {package}")
        } else {
            format!("a package called {package}, in {dir}")
        };
        let folder = dir.rsplit('/').next().unwrap_or_default().to_string();
        candidates.push((package.clone(), what.clone(), dir.clone()));
        candidates.push((folder, what, dir));
    }
    let mut folders: Vec<String> = std::fs::read_dir(base)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|folder| !folder.starts_with('.') && !NOT_OFFERED.contains(&folder.as_str()))
        .collect();
    folders.sort();
    for folder in folders {
        let dir = if base == Path::new(".") {
            folder.clone()
        } else {
            format!(
                "{}/{folder}",
                workspace::slashed(base).trim_end_matches('/')
            )
        };
        candidates.push((folder, format!("a directory called {dir}"), dir));
    }
    let found = crate::similar::closest(name, candidates.iter().map(|(key, _, _)| key.as_str()))?
        .to_string();
    candidates
        .into_iter()
        .find(|(key, _, _)| *key == found)
        .map(|(_, what, dir)| (what, dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_workspace_lists_ten_packages_as_commands_and_more_by_name() {
        let cwd = std::env::temp_dir().join("ws");
        let members = |count: usize| -> Vec<(String, PathBuf)> {
            (1..=count)
                .map(|n| {
                    (
                        format!("p{n:02}"),
                        cwd.join("crates").join(format!("p{n:02}")),
                    )
                })
                .collect()
        };
        let line = Line::new("run", &[]);
        let ten = packages("It", ".", &cwd, &members(10), &line);
        assert!(
            ten.starts_with("It has 10 packages. To test one:\n  mutastic run crates/p01\n")
                && ten.ends_with("\n  mutastic run crates/p10"),
            "{ten}"
        );
        assert_eq!(
            packages("It", ".", &cwd, &members(11), &line),
            "It has 11 packages: p01, p02, p03, p04, p05, p06, p07, p08, p09, p10, p11. To test \
             one, name its directory:\n  mutastic run <package directory>"
        );
    }
}
