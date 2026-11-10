//! The Java edition, for a Maven or Gradle project: found where its releases are kept, fetched the
//! first time one is needed, and handed the command unchanged.
//!
//! Releases are kept as the Java edition's own installer keeps them, each in
//! `~/.mutastic/mutastic-<version>/` with its command at `bin/mutastic`, so one installed that way
//! is used as it is. Fetching downloads `mutastic-<version>.tar` from the release, checks it against
//! the release's `SHA256SUMS`, and unpacks it there. Nothing is fetched for a Rust project.
//!
//! Settings, each optional:
//! - `MUTASTIC_VERSION`: the Java edition's release to use, fetched if it isn't kept. Otherwise,
//!   from 1.0.0, the release with this mutastic's own version, as the editions are released
//!   together at matching versions; before 1.0.0, the newest kept, or the latest release when none
//!   is.
//! - `MUTASTIC_HOME`: where releases are kept, `~/.mutastic` unless set.
//! - `MUTASTIC_JAVA_ENGINE`: a Java edition's `bin/mutastic` to run instead, such as one built
//!   from source. Nothing is fetched.
//! - `MUTASTIC_JAVA_RELEASES`: where releases are downloaded from, for a mirror.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use sha2::{Digest, Sha256};

/// Where the Java edition's releases are, unless `MUTASTIC_JAVA_RELEASES` says.
pub const RELEASES: &str = "https://github.com/mutastic/mutastic/releases";

/// The JDK the Java edition runs on.
pub const JDK: u32 = 25;

/// Where releases are kept.
pub fn home() -> Result<PathBuf, String> {
    if let Some(home) = std::env::var_os("MUTASTIC_HOME").filter(|home| !home.is_empty()) {
        return Ok(PathBuf::from(home));
    }
    user_home()
        .map(|home| home.join(".mutastic"))
        .ok_or_else(|| {
            format!(
                "neither MUTASTIC_HOME nor {USER_HOME} is set, so there's nowhere to keep the \
                 Java edition"
            )
        })
}

/// The variable naming the person's home directory: Windows' own, where `HOME` is usually unset
/// (Git Bash sets it, PowerShell doesn't), so `.mutastic` is the same folder from either.
const USER_HOME: &str = if cfg!(windows) { "USERPROFILE" } else { "HOME" };

fn user_home() -> Option<PathBuf> {
    std::env::var_os(USER_HOME)
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

/// Where output that isn't wanted goes, as curl's `-o` takes it.
const NULL_DEVICE: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };

fn releases() -> String {
    std::env::var("MUTASTIC_JAVA_RELEASES")
        .ok()
        .filter(|base| !base.is_empty())
        .unwrap_or_else(|| RELEASES.to_string())
        .trim_end_matches('/')
        .to_string()
}

fn wanted() -> Option<String> {
    std::env::var("MUTASTIC_VERSION")
        .ok()
        .filter(|version| !version.is_empty())
}

/// The Java edition that goes with this mutastic, `own`, unless `MUTASTIC_VERSION` says: from
/// 1.0.0 the editions are released together, at matching versions, so it's the release with the
/// same version. Before 1.0.0 their versions don't match, and there's none.
fn matching(own: &str) -> Option<String> {
    release_numbers(own)
        .filter(|(major, _, _)| *major >= 1)
        .map(|_| own.to_string())
}

/// The Java edition's release to use, without fetching: the one asked for, the one matching this
/// mutastic's version, or the newest kept, in that order. `None` when it's the latest release.
fn chosen(
    wanted: Option<String>,
    own: &str,
    newest: impl FnOnce() -> Option<String>,
) -> Option<String> {
    wanted.or_else(|| matching(own)).or_else(newest)
}

/// This mutastic's version.
const OWN: &str = env!("CARGO_PKG_VERSION");

/// A release's version, `0.5.0`, as numbers to compare: only plain releases, never a snapshot.
fn release_numbers(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.split('.').map(|part| {
        (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
            .then(|| part.parse::<u64>().ok())
            .flatten()
    });
    let numbers = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(numbers)
}

/// The command of the release `version` kept in `home`: on Windows its `.bat`, as the release's
/// `bin/mutastic` is a shell script.
pub fn launcher(home: &Path, version: &str) -> PathBuf {
    home.join(format!("mutastic-{version}"))
        .join("bin")
        .join(LAUNCHER)
}

const LAUNCHER: &str = if cfg!(windows) {
    "mutastic.bat"
} else {
    "mutastic"
};

/// The newest release kept in `home`, by version.
pub fn newest(home: &Path) -> Option<String> {
    std::fs::read_dir(home)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let version = name.strip_prefix("mutastic-")?.to_string();
            let numbers = release_numbers(&version)?;
            launcher(home, &version)
                .is_file()
                .then_some((numbers, version))
        })
        .max()
        .map(|(_, version)| version)
}

/// The Java edition present without fetching: its version and its command.
pub fn present() -> Option<(String, PathBuf)> {
    if let Some(given) = std::env::var_os("MUTASTIC_JAVA_ENGINE").filter(|g| !g.is_empty()) {
        let given = PathBuf::from(given);
        let output = Command::new(&given).arg("--version").output().ok()?;
        let printed = String::from_utf8_lossy(&output.stdout);
        let version = printed
            .lines()
            .next()?
            .trim()
            .trim_start_matches("mutastic ")
            .to_string();
        return Some((version, given));
    }
    let home = home().ok()?;
    let version = chosen(wanted(), OWN, || newest(&home))?;
    let command = launcher(&home, &version);
    command.is_file().then_some((version, command))
}

/// The Java edition's command: `MUTASTIC_JAVA_ENGINE`, a kept release, or one fetched now.
pub fn engine() -> Result<PathBuf, String> {
    if let Some(given) = std::env::var_os("MUTASTIC_JAVA_ENGINE").filter(|g| !g.is_empty()) {
        let given = PathBuf::from(given);
        return if given.is_file() {
            Ok(given)
        } else {
            Err(format!(
                "MUTASTIC_JAVA_ENGINE is {}, which isn't a file: it names a Java edition's bin/mutastic",
                given.display()
            ))
        };
    }
    let home = home()?;
    let releases = releases();
    let version = match chosen(wanted(), OWN, || newest(&home)) {
        Some(version) => version,
        None => latest(&releases)?,
    };
    let command = launcher(&home, &version);
    if command.is_file() {
        return Ok(command);
    }
    jdk()?;
    fetch(&home, &version, &releases)?;
    Ok(command)
}

/// Runs the Java edition with `args`, as they were given: its output and exit status are the
/// command's. On Unix it takes this process's place, so signals reach it as they would directly.
pub fn forward(command: &Path, args: &[String]) -> ExitCode {
    let mut process = Command::new(command);
    process.args(args);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = process.exec();
        eprintln!(
            "mutastic: couldn't run the Java edition at {}: {error}",
            command.display()
        );
        ExitCode::from(2)
    }
    #[cfg(not(unix))]
    match process.status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(2).clamp(0, 255) as u8),
        Err(error) => {
            eprintln!(
                "mutastic: couldn't run the Java edition at {}: {error}",
                command.display()
            );
            ExitCode::from(2)
        }
    }
}

/// The JDK's feature version, from what `java -XshowSettings:properties -version` prints.
fn feature(printed: &str) -> Option<u32> {
    printed.lines().find_map(|line| {
        line.trim()
            .strip_prefix("java.specification.version = ")?
            .trim()
            .parse()
            .ok()
    })
}

/// Checks there's a JDK the Java edition runs on, through `JAVA_HOME` or the `PATH`, before
/// anything is downloaded for it.
fn jdk() -> Result<(), String> {
    let java = match std::env::var_os("JAVA_HOME").filter(|home| !home.is_empty()) {
        Some(home) => Path::new(&home).join("bin").join("java"),
        None => PathBuf::from("java"),
    };
    let found = Command::new(&java)
        .args(["-XshowSettings:properties", "-version"])
        .output()
        .ok()
        .and_then(|output| feature(&String::from_utf8_lossy(&output.stderr)));
    match found {
        Some(version) if version >= JDK => Ok(()),
        Some(version) => Err(format!(
            "this is a Java project, and mutastic's Java edition runs on JDK {JDK}, but the java \
             found is JDK {version}: set JAVA_HOME to a JDK {JDK}, then run mutastic again"
        )),
        None => Err(format!(
            "this is a Java project, and mutastic's Java edition runs on JDK {JDK}, but no java \
             was found: install a JDK {JDK} and set JAVA_HOME to it, then run mutastic again"
        )),
    }
}

/// The latest release's version, from where the releases' `latest` page redirects to.
fn latest(releases: &str) -> Result<String, String> {
    let output = Command::new("curl")
        .args(["-fsSLI", "-o", NULL_DEVICE, "-w", "%{url_effective}"])
        .arg(format!("{releases}/latest"))
        .output()
        .map_err(|_| "fetching the Java edition needs curl, which isn't on the PATH".to_string())?;
    let url = String::from_utf8_lossy(&output.stdout).into_owned();
    url.rsplit_once("/v")
        .map(|(_, version)| version.to_string())
        .filter(|version| output.status.success() && release_numbers(version).is_some())
        .ok_or_else(|| format!("couldn't find the Java edition's latest release at {releases}"))
}

fn download(url: &str, to: &Path) -> Result<bool, String> {
    Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(to)
        .arg(url)
        .status()
        .map(|status| status.success())
        .map_err(|_| "fetching the Java edition needs curl, which isn't on the PATH".to_string())
}

fn sha256(file: &Path) -> Result<String, String> {
    let bytes = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// The checksum `SHA256SUMS` gives `name`: lines of a checksum and a file name, as `sha256sum`
/// writes them.
pub fn listed(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (sum, file) = line.split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name).then(|| sum.to_lowercase())
    })
}

/// Checks a download against its release's `SHA256SUMS`, when the release has one: releases
/// before 0.4.0 don't.
fn verify(tar: &Path, name: &str, version: &str, releases: &str) -> Result<(), String> {
    let sums = tar.with_file_name("SHA256SUMS");
    if !download(&format!("{releases}/download/v{version}/SHA256SUMS"), &sums)? {
        eprintln!("mutastic: this release has no checksums to check the download against");
        return Ok(());
    }
    let text = std::fs::read_to_string(&sums).map_err(|e| e.to_string())?;
    let expected = listed(&text, name).ok_or_else(|| format!("SHA256SUMS doesn't list {name}"))?;
    if sha256(tar)? != expected {
        return Err(format!(
            "the Java edition's download doesn't match its checksum, so it wasn't kept: {name}"
        ));
    }
    Ok(())
}

/// Downloads, checks and unpacks a release into `home`, where it's kept.
fn fetch(home: &Path, version: &str, releases: &str) -> Result<(), String> {
    eprintln!(
        "mutastic: this is a Java project, so mutastic fetches its Java edition, {version}, into \
         {}: once",
        home.display()
    );
    let work = home.join(format!(".fetching-{}", std::process::id()));
    std::fs::create_dir_all(&work).map_err(|e| format!("{}: {e}", work.display()))?;
    let kept = unpack(&work, home, version, releases);
    std::fs::remove_dir_all(&work).ok();
    kept
}

fn unpack(work: &Path, home: &Path, version: &str, releases: &str) -> Result<(), String> {
    let name = format!("mutastic-{version}.tar");
    let tar = work.join(&name);
    if !download(&format!("{releases}/download/v{version}/{name}"), &tar)? {
        return Err(format!(
            "couldn't download the Java edition {version}: is it a release? ({releases})"
        ));
    }
    verify(&tar, &name, version, releases)?;
    // Unpacked from inside `work`, by the archive's bare name: a tar given `C:\...` can take it
    // for a remote host, as Git Bash's does.
    let unpacked = Command::new("tar")
        .arg("-xf")
        .arg(&name)
        .current_dir(work)
        .status()
        .map_err(|_| "fetching the Java edition needs tar, which isn't on the PATH".to_string())?;
    let release = work.join(format!("mutastic-{version}"));
    if !unpacked.success() || !release.join("bin").join(LAUNCHER).is_file() {
        return Err(format!(
            "the Java edition's download, {name}, had no mutastic command in it"
        ));
    }
    let destination = home.join(format!("mutastic-{version}"));
    // Another run may have kept it meanwhile: theirs is as good.
    if std::fs::rename(&release, &destination).is_err() && !launcher(home, version).is_file() {
        return Err(format!(
            "couldn't keep the Java edition in {}",
            destination.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_releases_are_versions() {
        assert_eq!(release_numbers("0.5.0"), Some((0, 5, 0)));
        assert_eq!(release_numbers("10.20.30"), Some((10, 20, 30)));
        assert_eq!(release_numbers("0.3.0-SNAPSHOT"), None);
        assert_eq!(release_numbers("0.5"), None);
        assert_eq!(release_numbers("0.5.0.1"), None);
        assert_eq!(release_numbers("a.b.c"), None);
    }

    #[test]
    fn the_newest_kept_release_is_by_number_not_by_name() {
        let home = std::env::temp_dir().join(format!("mutastic-newest-{}", std::process::id()));
        for version in ["0.9.0", "0.10.0", "0.11.0-SNAPSHOT"] {
            let command = launcher(&home, version);
            std::fs::create_dir_all(command.parent().unwrap()).unwrap();
            std::fs::write(command, "").unwrap();
        }
        std::fs::create_dir_all(home.join("mutastic-0.12.0")).unwrap();
        assert_eq!(newest(&home).as_deref(), Some("0.10.0"));
        std::fs::remove_dir_all(home).ok();
    }

    #[test]
    fn from_1_0_the_java_edition_is_the_one_with_the_same_version() {
        let newest = || Some("1.4.0".to_string());
        assert_eq!(chosen(None, "1.2.0", newest).as_deref(), Some("1.2.0"));
        assert_eq!(chosen(None, "2.0.1", newest).as_deref(), Some("2.0.1"));
        // MUTASTIC_VERSION still says which.
        assert_eq!(
            chosen(Some("1.1.0".into()), "1.2.0", newest).as_deref(),
            Some("1.1.0")
        );
        // Before 1.0.0 the versions don't match: the newest kept, or the latest when none is.
        assert_eq!(chosen(None, "0.2.1", newest).as_deref(), Some("1.4.0"));
        assert_eq!(chosen(None, "0.2.1", || None), None);
        assert_eq!(matching("1.0.0-dev"), None);
    }

    #[test]
    fn the_jdk_is_read_from_its_settings() {
        let printed =
            "Property settings:\n    java.home = /jdk\n    java.specification.version = 25\n";
        assert_eq!(feature(printed), Some(25));
        assert_eq!(feature("    java.specification.version = 17\n"), Some(17));
        assert_eq!(feature("Unable to locate a Java Runtime."), None);
    }

    #[test]
    fn a_checksum_is_found_by_its_files_whole_name() {
        let sums =
            "aa11  mutastic-0.5.0.tar.asc\nBB22 *mutastic-0.5.0.tar\ncc33  mutastic-0.5.0.zip\n";
        assert_eq!(listed(sums, "mutastic-0.5.0.tar").as_deref(), Some("bb22"));
        assert_eq!(listed(sums, "mutastic-0.4.0.tar"), None);
    }
}
