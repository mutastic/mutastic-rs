//! The Java edition, for a Maven or Gradle project: found where its releases are kept, fetched the
//! first time one is needed, and handed the command unchanged.
//!
//! Releases are kept as the Java edition's own installer keeps them, each in
//! `~/.mutastic/mutastic-<version>/` with its command at `bin/mutastic`, so one installed that way
//! is used as it is. Fetching downloads `mutastic-<version>.tar` from the release, checks it against
//! the release's `SHA256SUMS`, and unpacks it there. Nothing is fetched for a Rust project.
//!
//! Settings, each optional:
//! - `MUTASTIC_VERSION`: the Java edition's release to use, exactly, fetched if it isn't kept.
//!   Otherwise, from 1.0.0, the newest release of this mutastic's own major and minor version, as
//!   the editions release each minor together and may each ship a patch alone: the releases are
//!   looked up at most once a day. Before 1.0.0, the newest kept, or the latest release when none
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

/// The major and minor version this mutastic, `own`, matches the Java edition by: from 1.0.0 the
/// editions release each minor version together, and either may ship a patch alone. Before 1.0.0
/// their versions don't match, and there's none.
fn line(own: &str) -> Option<(u64, u64)> {
    release_numbers(own)
        .filter(|(major, _, _)| *major >= 1)
        .map(|(major, minor, _)| (major, minor))
}

/// The newest of `versions` in `own`'s major and minor version.
fn newest_patch<'a>(own: &str, versions: impl IntoIterator<Item = &'a String>) -> Option<String> {
    let (major, minor) = line(own)?;
    versions
        .into_iter()
        .filter_map(|version| Some((release_numbers(version)?, version)))
        .filter(|((ma, mi, _), _)| (*ma, *mi) == (major, minor))
        .max()
        .map(|(_, version)| version.clone())
}

/// How long a look at the releases is trusted before mutastic looks again: a day.
const DAY: u64 = 24 * 60 * 60;

/// The releases mutastic last saw at `source`, kept in `home`, and whether that was within a day
/// of `now` (in seconds since the epoch). Seen at another source, they count for nothing.
fn seen(home: &Path, source: &str, now: u64) -> (Vec<String>, bool) {
    let Ok(text) = std::fs::read_to_string(home.join(SEEN)) else {
        return (Vec::new(), false);
    };
    let mut lines = text.lines();
    let checked = lines
        .next()
        .and_then(|l| l.strip_prefix("checked ")?.parse::<u64>().ok());
    let from = lines.next().and_then(|l| l.strip_prefix("from "));
    match (checked, from) {
        (Some(checked), Some(from)) if from == source => (
            lines.map(str::to_string).collect(),
            now.saturating_sub(checked) < DAY,
        ),
        _ => (Vec::new(), false),
    }
}

/// Where the releases mutastic last saw are written down, in its home.
const SEEN: &str = ".java-releases";

/// Writes down the releases seen at `source` at `now`. Failing to is no reason to stop.
fn remember(home: &Path, source: &str, now: u64, releases: &[String]) {
    let mut text = format!("checked {now}\nfrom {source}\n");
    for release in releases {
        text.push_str(release);
        text.push('\n');
    }
    let partial = home.join(format!("{SEEN}-{}", std::process::id()));
    if std::fs::create_dir_all(home).is_ok() && std::fs::write(&partial, text).is_ok() {
        std::fs::rename(&partial, home.join(SEEN)).ok();
    }
    std::fs::remove_file(partial).ok();
}

/// The Java edition's release for this mutastic, `own`, from 1.0.0: the newest of its major and
/// minor version, kept in `home` or released at `source`. The releases are looked up, through
/// `look`, only when the last look is over a day old or found nothing to run; when a look fails,
/// what's kept is used.
fn matched(
    home: &Path,
    source: &str,
    own: &str,
    now: u64,
    look: impl FnOnce() -> Option<Vec<String>>,
) -> Result<String, String> {
    let kept = kept(home);
    let (before, fresh) = seen(home, source, now);
    let known = newest_patch(own, kept.iter().chain(&before));
    if let (true, Some(version)) = (fresh, &known) {
        return Ok(version.clone());
    }
    let (major, minor) = line(own).unwrap_or_default();
    match look() {
        Some(released) => {
            remember(home, source, now, &released);
            newest_patch(own, kept.iter().chain(&released)).ok_or_else(|| {
                format!(
                    "mutastic {own} runs the Java edition {major}.{minor}.x, and there's no such \
                     release at {source} yet: set MUTASTIC_VERSION to a release to use"
                )
            })
        }
        None => {
            // Don't look again on every run while the releases can't be reached.
            if known.is_some() {
                remember(home, source, now, &before);
            }
            known.ok_or_else(|| {
                format!(
                    "mutastic {own} runs the Java edition {major}.{minor}.x, and couldn't look up \
                     its releases at {source}: check the connection, or set MUTASTIC_VERSION to a \
                     release to use"
                )
            })
        }
    }
}

/// The versions a releases feed links to, `.../tag/v1.0.2`: plain releases only.
fn feed_versions(feed: &str) -> Vec<String> {
    feed.split("/tag/v")
        .skip(1)
        .filter_map(|rest| {
            let end = rest.find(['"', '<', '\'', ' ', '\n']).unwrap_or(rest.len());
            let version = &rest[..end];
            release_numbers(version).map(|_| version.to_string())
        })
        .collect()
}

/// The releases at `releases`, from its feed, `releases.atom`, newest first; or, without a feed,
/// the latest release alone.
fn published(releases: &str) -> Option<Vec<String>> {
    let output = Command::new("curl")
        .args(["-fsSL", "--connect-timeout", "5", "--max-time", "20"])
        .arg(format!("{releases}.atom"))
        .output()
        .ok()?;
    let found = feed_versions(&String::from_utf8_lossy(&output.stdout));
    if output.status.success() && !found.is_empty() {
        return Some(found);
    }
    latest(releases).ok().map(|version| vec![version])
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default()
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

/// The releases kept in `home`.
fn kept(home: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(home) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let version = name.strip_prefix("mutastic-")?.to_string();
            release_numbers(&version)?;
            launcher(home, &version).is_file().then_some(version)
        })
        .collect()
}

/// The newest release kept in `home`, by version.
pub fn newest(home: &Path) -> Option<String> {
    kept(home)
        .into_iter()
        .filter_map(|version| Some((release_numbers(&version)?, version)))
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
    let version = match (wanted(), line(OWN)) {
        (Some(version), _) => version,
        (None, Some(_)) => newest_patch(OWN, &kept(&home))?,
        (None, None) => newest(&home)?,
    };
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
    let version = match (wanted(), line(OWN)) {
        (Some(version), _) => version,
        (None, Some(_)) => matched(&home, &releases, OWN, now(), || published(&releases))?,
        (None, None) => match newest(&home) {
            Some(version) => version,
            None => latest(&releases)?,
        },
    };
    let command = launcher(&home, &version);
    if command.is_file() {
        return Ok(command);
    }
    jdk()?;
    match fetch(&home, &version, &releases) {
        Ok(()) => Ok(command),
        Err(error) => {
            match meanwhile(OWN, wanted().is_some(), &kept(&home)) {
                Some(kept) => {
                    stderr_line!("mutastic: {error}\nmutastic: running the Java edition {kept}, kept, meanwhile");
                    Ok(launcher(&home, &kept))
                }
                None => Err(error),
            }
        }
    }
}

/// What runs when the release chosen can't be fetched: from 1.0.0, unless `MUTASTIC_VERSION` asked
/// for it, the newest kept of the same minor version, as a patch just published may not have its
/// files attached yet, or the download may have failed. Otherwise nothing, and the error stands.
fn meanwhile(own: &str, asked: bool, kept: &[String]) -> Option<String> {
    if asked {
        return None;
    }
    newest_patch(own, kept)
}

/// Runs the Java edition with `args`, as they were given: its output and exit status are the
/// command's. On Unix it takes this process's place, so signals reach it as they would directly.
pub fn forward(command: &Path, args: &[String]) -> ExitCode {
    let mut process = Command::new(command);
    process.args(args);
    // The JDK can ask whether stdin and stdout are a terminal, not stderr, where progress goes: so
    // it's told, unless whoever ran this already said.
    if std::env::var_os("MUTASTIC_STDERR_TTY").is_none() {
        use std::io::IsTerminal as _;
        let terminal = std::io::stderr().is_terminal();
        process.env("MUTASTIC_STDERR_TTY", if terminal { "1" } else { "0" });
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = process.exec();
        stderr_line!(
            "mutastic: couldn't run the Java edition at {}: {error}",
            command.display()
        );
        ExitCode::from(2)
    }
    #[cfg(not(unix))]
    match process.status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(2).clamp(0, 255) as u8),
        Err(error) => {
            stderr_line!(
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
        stderr_line!("mutastic: this release has no checksums to check the download against");
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
    stderr_line!(
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

    fn versions(list: &[&str]) -> Vec<String> {
        list.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn from_1_0_the_java_edition_is_the_newest_patch_of_the_same_minor() {
        let released = versions(&["1.3.0", "1.2.10", "1.2.9", "1.2.3-rc1", "2.2.0", "1.1.4"]);
        assert_eq!(newest_patch("1.2.0", &released).as_deref(), Some("1.2.10"));
        assert_eq!(newest_patch("1.2.11", &released).as_deref(), Some("1.2.10"));
        assert_eq!(newest_patch("1.4.0", &released), None);
        // Before 1.0.0 the versions don't match.
        assert_eq!(newest_patch("0.2.1", &versions(&["0.2.1", "0.5.0"])), None);
        assert_eq!(line("1.0.0-dev"), None);
    }

    #[test]
    fn a_patch_that_cant_be_fetched_leaves_the_kept_one_running() {
        let kept = versions(&["1.0.1", "1.1.0"]);
        assert_eq!(meanwhile("1.0.0", false, &kept).as_deref(), Some("1.0.1"));
        // Asked for exactly, or none kept of the same minor, or before 1.0.0: the error stands.
        assert_eq!(meanwhile("1.0.0", true, &kept), None);
        assert_eq!(meanwhile("1.2.0", false, &kept), None);
        assert_eq!(meanwhile("0.2.1", false, &versions(&["0.5.0"])), None);
    }

    #[test]
    fn a_releases_feed_gives_its_plain_releases() {
        let feed = r#"<feed><link href="https://github.com/mutastic/mutastic/releases"/>
            <entry><link rel="alternate" href="https://github.com/mutastic/mutastic/releases/tag/v1.0.2"/></entry>
            <entry><link rel="alternate" href="https://github.com/mutastic/mutastic/releases/tag/v1.0.2-rc1"/></entry>
            <entry><id>tag:github.com,2008:Repository/1/v1.0.1</id>
            <link rel="alternate" href="https://github.com/mutastic/mutastic/releases/tag/v1.0.1"/></entry>
            </feed>"#;
        assert_eq!(feed_versions(feed), ["1.0.2", "1.0.1"]);
        assert!(feed_versions("<html>Not Found</html>").is_empty());
    }

    /// A home of the test's own, with `kept` releases in it.
    fn home_keeping(name: &str, kept: &[&str]) -> PathBuf {
        let home = std::env::temp_dir().join(format!("mutastic-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&home).ok();
        for version in kept {
            let command = launcher(&home, version);
            std::fs::create_dir_all(command.parent().unwrap()).unwrap();
            std::fs::write(command, "").unwrap();
        }
        std::fs::create_dir_all(&home).unwrap();
        home
    }

    #[test]
    fn the_releases_are_looked_up_at_most_once_a_day() {
        let home = home_keeping("looked-up", &["1.0.1"]);
        let source = "https://example.com/releases";
        let looks = std::cell::Cell::new(0);
        let look = |found: Option<&[&str]>| {
            looks.set(looks.get() + 1);
            found.map(versions)
        };
        let day = 1_000_000;
        // The first run looks, and the newer patch is chosen over the one kept.
        let first = matched(&home, source, "1.0.0", day, || {
            look(Some(&["1.0.2", "1.1.0"]))
        });
        assert_eq!((first.as_deref(), looks.get()), (Ok("1.0.2"), 1));
        // Within the day, what was seen counts, without looking.
        let again = matched(&home, source, "1.0.4", day + DAY - 1, || {
            look(Some(&["1.0.3"]))
        });
        assert_eq!((again.as_deref(), looks.get()), (Ok("1.0.2"), 1));
        // A day on, it looks again.
        let next = matched(&home, source, "1.0.0", day + DAY, || look(Some(&["1.0.3"])));
        assert_eq!((next.as_deref(), looks.get()), (Ok("1.0.3"), 2));
        // Another source's releases don't count.
        let mirror = matched(&home, "file:///mirror", "1.0.0", day + DAY, || look(None));
        assert_eq!((mirror.as_deref(), looks.get()), (Ok("1.0.1"), 3));
        std::fs::remove_dir_all(home).ok();
    }

    #[test]
    fn without_a_matching_release_it_looks_and_says_what_to_do() {
        let home = home_keeping("unmatched", &["1.0.1"]);
        let source = "https://example.com/releases";
        let day = 1_000_000;
        remember(&home, source, day, &versions(&["1.0.1"]));
        // Seen today, but nothing of 1.1: it looks again rather than wait a day.
        let looked = matched(&home, source, "1.1.0", day + 1, || {
            Some(versions(&["1.1.0"]))
        });
        assert_eq!(looked.as_deref(), Ok("1.1.0"));
        let none = matched(&home, source, "1.2.0", day + 2, || {
            Some(versions(&["1.1.0"]))
        });
        assert!(none
            .unwrap_err()
            .contains("runs the Java edition 1.2.x, and there's no such release"));
        // When the releases can't be reached, what's kept is used, and only that.
        let offline = matched(&home, source, "1.0.0", day + 3 * DAY, || None);
        assert_eq!(offline.as_deref(), Ok("1.0.1"));
        let nothing = matched(&home, source, "1.3.0", day + 3 * DAY, || None);
        assert!(nothing
            .unwrap_err()
            .contains("couldn't look up its releases"));
        std::fs::remove_dir_all(home).ok();
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
