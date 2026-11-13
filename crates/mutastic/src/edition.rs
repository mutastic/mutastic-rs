//! Which edition runs a command: this one, the Rust edition, or the Java edition, which [`java`]
//! finds or fetches and hands the command to unchanged.
//!
//! The directory a command points at decides, by its build file: `Cargo.toml` is the Rust
//! edition's, and `pom.xml`, `build.gradle(.kts)` or `settings.gradle(.kts)` the Java edition's.
//! A directory with neither is decided by the nearest one above it that has one. A directory with
//! both is a question only the person can answer, with `--edition`. One build nested in another of
//! the other edition, such as a Rust crate inside a Gradle build, is its own project: pointing at
//! the outer one runs the outer one's edition, and `run` says how to point at the inner one.
//!
//! [`java`]: crate::java

use std::path::{Path, PathBuf};

/// The Java edition's build files: Maven's, then Gradle's.
pub const JVM_BUILDS: &[&str] = &[
    "pom.xml",
    "build.gradle.kts",
    "build.gradle",
    "settings.gradle.kts",
    "settings.gradle",
];

/// The Rust edition's build file.
pub const CARGO_BUILD: &str = "Cargo.toml";

/// Directories never searched for a nested build: build output, dependencies and hidden ones.
const NOT_SEARCHED: &[&str] = &["target", "build", "node_modules", "out"];

/// How deep below a project a nested build of the other edition is looked for.
const NESTED_DEPTH: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edition {
    Rust,
    Java,
}

impl Edition {
    pub fn parse(name: &str) -> Option<Edition> {
        match name {
            "rust" => Some(Edition::Rust),
            "java" => Some(Edition::Java),
            _ => None,
        }
    }
}

/// Where a command goes, and why.
#[derive(Debug, PartialEq, Eq)]
pub struct Route {
    pub edition: Edition,
    /// The directory whose build file decided, if one did.
    pub project: Option<PathBuf>,
    /// The build file that decided: `Cargo.toml`, `pom.xml`, `build.gradle.kts`…
    pub build: Option<&'static str>,
}

/// The build files in `dir` of each edition: the first Java one, and whether there's a
/// `Cargo.toml`.
fn builds_in(dir: &Path) -> (Option<&'static str>, bool) {
    let jvm = JVM_BUILDS
        .iter()
        .copied()
        .find(|name| dir.join(name).is_file());
    (jvm, dir.join(CARGO_BUILD).is_file())
}

/// Which edition reads `dir`: its own build file's, or the nearest one above it. With no build
/// file anywhere above, the Rust edition, which says there's no project here as it always has.
pub fn route(dir: &Path) -> Result<Route, String> {
    let start = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    for here in start.ancestors() {
        let (jvm, cargo) = builds_in(here);
        let (edition, build) = match (jvm, cargo) {
            (Some(jvm), true) => {
                return Err(format!(
                    "{} has both {CARGO_BUILD} and {jvm}, so either edition could read it: say \
                     which with --edition=rust or --edition=java",
                    here.display()
                ))
            }
            (Some(jvm), false) => (Edition::Java, jvm),
            (None, true) => (Edition::Rust, CARGO_BUILD),
            (None, false) => continue,
        };
        return Ok(Route {
            edition,
            project: Some(here.to_path_buf()),
            build: Some(build),
        });
    }
    Ok(Route {
        edition: Edition::Rust,
        project: None,
        build: None,
    })
}

/// `--edition=<edition>`, if given, and the arguments without it, which is how either edition
/// receives them.
pub fn chosen(args: &[String]) -> Result<(Option<Edition>, Vec<String>), String> {
    let mut edition = None;
    let mut rest = Vec::with_capacity(args.len());
    for arg in args {
        match arg.strip_prefix("--edition=") {
            Some(name) => {
                edition = Some(
                    Edition::parse(name)
                        .ok_or_else(|| format!("--edition: {name} isn't one of java or rust"))?,
                )
            }
            None if arg == "--edition" => {
                return Err("--edition needs a value: --edition=java or --edition=rust".into())
            }
            None => rest.push(arg.clone()),
        }
    }
    Ok((edition, rest))
}

/// The directory a command points at, as given: the first argument after the command that names
/// one. Ids, `:module`s, options and `gate`'s action never do. Without one, the current directory.
pub fn pointed_at(args: &[String]) -> String {
    let action = usize::from(args.first().is_some_and(|command| command == "gate"));
    args.iter()
        .skip(1)
        .filter(|arg| !arg.starts_with('-') && !arg.starts_with(':'))
        .skip(action)
        .find(|arg| Path::new(arg).is_dir())
        .cloned()
        .unwrap_or_else(|| ".".to_string())
}

/// The projects of the other edition below `project`, nearest first, relative to it: a Rust crate
/// inside a Gradle build, say.
pub fn nested(project: &Path, edition: Edition) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut level = vec![PathBuf::new()];
    for _ in 0..NESTED_DEPTH {
        let mut next = Vec::new();
        for relative in level {
            let Ok(entries) = std::fs::read_dir(project.join(&relative)) else {
                continue;
            };
            let mut dirs: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                .map(|entry| entry.file_name())
                .filter(|name| {
                    let name = name.to_string_lossy();
                    !name.starts_with('.') && !NOT_SEARCHED.contains(&name.as_ref())
                })
                .map(|name| relative.join(name))
                .collect();
            dirs.sort();
            for dir in dirs {
                let (jvm, cargo) = builds_in(&project.join(&dir));
                let other = match edition {
                    Edition::Java => cargo,
                    Edition::Rust => jvm.is_some(),
                };
                if other {
                    // A project's own subprojects are its business: look no deeper.
                    found.push(dir);
                } else {
                    next.push(dir);
                }
            }
        }
        level = next;
    }
    found
}

/// Commands to copy, one a line, each indented two spaces: `mutastic <command> <dir> <options>` for
/// each directory, where `command` is the command and its words before the directory (`run`,
/// `verify m1a2b3c4d`). The current directory, `.`, is left out, as a person would type it; one with
/// a space or a quote in it is quoted, as a shell takes it, and a `<placeholder>` is left as it is.
pub fn commands(command: &str, dirs: &[String], options: &[String]) -> String {
    dirs.iter()
        .map(|dir| {
            let quoted = if dir.contains([' ', '\'', '"']) && !dir.starts_with('<') {
                format!("\"{}\"", dir.replace('"', "\\\""))
            } else {
                dir.clone()
            };
            let words = std::iter::once(command)
                .chain((dir != ".").then_some(quoted.as_str()))
                .chain(options.iter().map(String::as_str));
            format!("  mutastic {}", words.collect::<Vec<_>>().join(" "))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// What `run` says when the project it runs holds one of the other edition's: which edition runs,
/// and the command for the other.
pub fn nested_note(given: &str, route: &Route) -> Option<String> {
    let project = route.project.as_ref()?;
    let inner = nested(project, route.edition);
    if inner.is_empty() {
        return None;
    }
    let (this, other, kind) = match route.edition {
        Edition::Java if route.build == Some("pom.xml") => ("a Maven project", "Rust", "crate"),
        Edition::Java => ("a Gradle build", "Rust", "crate"),
        Edition::Rust => ("a Cargo project", "Java", "project"),
    };
    let edition = match route.edition {
        Edition::Java => "Java",
        Edition::Rust => "Rust",
    };
    let dirs: Vec<String> = inner
        .iter()
        .map(|dir| {
            let dir = crate::workspace::slashed(dir);
            if given == "." {
                dir
            } else {
                format!("{}/{dir}", given.trim_end_matches(['/', '\\']))
            }
        })
        .collect();
    let name = project.file_name().map_or_else(
        || project.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let held = if inner.len() == 1 {
        format!("a {other} {kind}")
    } else {
        format!("{other} {kind}s")
    };
    Some(format!(
        "mutastic: {name} is {this}, so the {edition} edition runs here. It also holds {held}, \
         which is a project of its own; to test it, point mutastic at it:\n{}",
        commands("run", &dirs, &[])
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    /// A directory tree with the given files, each empty.
    fn tree(files: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mutastic-edition-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for file in files {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }
        std::fs::canonicalize(dir).unwrap()
    }

    #[test]
    fn each_build_file_picks_its_edition() {
        for (file, edition) in [
            ("Cargo.toml", Edition::Rust),
            ("pom.xml", Edition::Java),
            ("build.gradle", Edition::Java),
            ("build.gradle.kts", Edition::Java),
            ("settings.gradle", Edition::Java),
            ("settings.gradle.kts", Edition::Java),
        ] {
            let dir = tree(&[file]);
            let route = route(&dir).unwrap();
            assert_eq!(
                (route.edition, route.build, route.project.as_deref()),
                (edition, Some(file), Some(dir.as_path())),
                "{file}"
            );
            std::fs::remove_dir_all(dir).ok();
        }
    }

    #[test]
    fn a_mixed_tree_is_decided_by_the_directory_pointed_at() {
        let dir = tree(&[
            "settings.gradle.kts",
            "app/build.gradle.kts",
            "app/src/main/java/App.java",
            "native/build.gradle.kts",
            "native/src/main/rust/Cargo.toml",
            "native/src/main/rust/tictactoe/lib.rs",
            "native/src/main/rust/target/debug/Cargo.toml",
        ]);
        let at = |relative: &str| {
            let route = route(&dir.join(relative)).unwrap();
            (route.edition, route.project.unwrap())
        };
        assert_eq!(at(""), (Edition::Java, dir.clone()));
        assert_eq!(at("app/src/main/java"), (Edition::Java, dir.join("app")));
        assert_eq!(at("native"), (Edition::Java, dir.join("native")));
        let crate_dir = dir.join("native/src/main/rust");
        assert_eq!(
            at("native/src/main/rust"),
            (Edition::Rust, crate_dir.clone())
        );
        assert_eq!(
            at("native/src/main/rust/tictactoe"),
            (Edition::Rust, crate_dir)
        );
        let java = route(&dir).unwrap();
        assert_eq!(
            nested(&dir, Edition::Java),
            vec![PathBuf::from("native/src/main/rust")]
        );
        assert_eq!(
            nested_note(".", &java).unwrap().lines().last().unwrap(),
            "  mutastic run native/src/main/rust"
        );
        let rust = route(&dir.join("native/src/main/rust")).unwrap();
        assert_eq!(nested_note(".", &rust), None);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn each_command_shown_is_one_a_shell_takes_as_typed() {
        let dirs = [".", "crates/shop", "my crates/shop", "<package directory>"].map(String::from);
        assert_eq!(
            commands("verify m1a2b3c4d", &dirs, &["--format=agent".to_string()]),
            "  mutastic verify m1a2b3c4d --format=agent\n  \
             mutastic verify m1a2b3c4d crates/shop --format=agent\n  \
             mutastic verify m1a2b3c4d \"my crates/shop\" --format=agent\n  \
             mutastic verify m1a2b3c4d <package directory> --format=agent"
        );
    }

    #[test]
    fn both_editions_in_one_directory_is_the_persons_to_choose() {
        let dir = tree(&["Cargo.toml", "pom.xml"]);
        let error = route(&dir).unwrap_err();
        assert!(
            error.contains("--edition=rust or --edition=java"),
            "{error}"
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn no_build_file_anywhere_is_the_rust_editions_to_answer() {
        // The filesystem's root has no build file on any machine CI runs on.
        let route = route(Path::new("/")).unwrap();
        assert_eq!((route.edition, route.project), (Edition::Rust, None));
    }

    #[test]
    fn the_edition_option_is_taken_out_and_checked() {
        let args: Vec<String> = ["run", "--edition=java", "x"].map(String::from).to_vec();
        let (edition, rest) = chosen(&args).unwrap();
        assert_eq!(edition, Some(Edition::Java));
        assert_eq!(rest, ["run", "x"]);
        assert!(chosen(&["--edition=go".to_string()]).is_err());
        assert!(chosen(&["--edition".to_string()]).is_err());
    }

    #[test]
    fn the_directory_pointed_at_is_never_an_id_a_module_or_an_action() {
        let dir = tree(&["pom.xml"]);
        let path = dir.display().to_string();
        let args = |words: &[&str]| words.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        assert_eq!(pointed_at(&args(&["run"])), ".");
        assert_eq!(pointed_at(&args(&["run", &path, "--since=main"])), path);
        assert_eq!(pointed_at(&args(&["verify", "m1234abcd", &path])), path);
        assert_eq!(
            pointed_at(&args(&["gate", "check", &path, "--hook=claude"])),
            path
        );
        assert_eq!(pointed_at(&args(&["run", ":app"])), ".");
        std::fs::remove_dir_all(dir).ok();
    }
}
