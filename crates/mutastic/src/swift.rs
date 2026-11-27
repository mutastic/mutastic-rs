//! The Swift edition, for a SwiftPM package or an Xcode project: not released yet, so a Swift
//! project is refused with what to do instead, unless `MUTASTIC_SWIFT_ENGINE` names a Swift
//! edition built from its source, which is handed the command unchanged, as [`java`] hands the
//! Java edition its.
//!
//! [`java`]: crate::java

#![expect(clippy::disallowed_methods, reason = "starts the Swift edition")]

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::edition::Route;
use crate::failure::{Code, Failure};

/// The release the Swift edition is planned for.
pub const PLANNED: &str = "1.5";

/// The variable naming a Swift edition's command to run.
pub const ENGINE: &str = "MUTASTIC_SWIFT_ENGINE";

/// Whether `MUTASTIC_SWIFT_ENGINE` names a Swift edition to run.
pub fn given() -> bool {
    std::env::var_os(ENGINE).is_some_and(|given| !given.is_empty())
}

/// The Swift edition's command: the one `MUTASTIC_SWIFT_ENGINE` names. Without one, the refusal
/// that says the Swift edition isn't released yet, for the project `route` found.
pub fn engine(route: &Route) -> Result<PathBuf, Failure> {
    let Some(given) = std::env::var_os(ENGINE).filter(|given| !given.is_empty()) else {
        return Err(Code::SwiftEdition.with(unreleased(route)));
    };
    let given = PathBuf::from(given);
    if given.is_file() {
        Ok(given)
    } else {
        Err(Code::SwiftEdition.with(format!(
            "{ENGINE} is {}, which isn't a file: it names a Swift edition's mutastic, such as one \
             built from its source",
            given.display()
        )))
    }
}

/// What a Swift project is told: what it is, that the Swift edition isn't out, and how to run one
/// built from its source meanwhile.
fn unreleased(route: &Route) -> String {
    let what = match (&route.project, route.build) {
        (Some(project), Some(build)) => {
            let name = project.file_name().map_or_else(
                || project.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            format!("{name} is {}, and the", kind(build))
        }
        _ => "the".to_string(),
    };
    format!(
        "{what} Swift edition isn't released yet: it's planned for {PLANNED}. Until then mutastic \
         tests Rust, Maven and Gradle projects. To run a Swift edition built from its source, name \
         its mutastic in {ENGINE}:\n  {ENGINE}=<path to its mutastic> mutastic run"
    )
}

/// What a Swift build file makes its directory, as a person says it.
fn kind(build: &str) -> &'static str {
    match build {
        ".xcworkspace" => "an Xcode workspace",
        ".xcodeproj" => "an Xcode project",
        _ => "a Swift package",
    }
}

/// Runs the Swift edition at `command` with `args`, as they were given: its output and exit status
/// are the command's. On Unix it takes this process's place, so signals reach it as they would
/// directly.
pub fn forward(command: &Path, args: &[String]) -> ExitCode {
    let mut process = Command::new(command);
    process.args(args);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = process.exec();
        stderr_line!(
            "mutastic: couldn't run the Swift edition at {}: {error}",
            command.display()
        );
        ExitCode::from(2)
    }
    #[cfg(not(unix))]
    match process.status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(2).clamp(0, 255) as u8),
        Err(error) => {
            stderr_line!(
                "mutastic: couldn't run the Swift edition at {}: {error}",
                command.display()
            );
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edition::Edition;

    fn route(project: Option<&str>, build: Option<&'static str>) -> Route {
        Route {
            edition: Edition::Swift,
            project: project.map(PathBuf::from),
            build,
        }
    }

    #[test]
    fn a_swift_project_is_told_the_edition_isnt_out_and_what_to_do() {
        let said = unreleased(&route(Some("/home/you/shop"), Some("Package.swift")));
        assert!(
            said.starts_with(
                "shop is a Swift package, and the Swift edition isn't released yet: it's planned \
                 for 1.5."
            ),
            "{said}"
        );
        assert!(said.ends_with("\n  MUTASTIC_SWIFT_ENGINE=<path to its mutastic> mutastic run"));
        let xcode = unreleased(&route(Some("/home/you/App"), Some(".xcodeproj")));
        assert!(
            xcode.starts_with("App is an Xcode project, and the"),
            "{xcode}"
        );
        let workspace = unreleased(&route(Some("/w/App"), Some(".xcworkspace")));
        assert!(workspace.starts_with("App is an Xcode workspace, and the"));
        assert!(unreleased(&route(None, None)).starts_with("the Swift edition isn't released yet"));
    }
}
