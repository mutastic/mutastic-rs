//! The gate's count of the tests a task starts with, made in the background, as the Java edition
//! makes its count of a parameterized test's cases: `gate open` returns at once, so the person
//! never waits before the agent starts.
//!
//! `gate open` notes the working tree as git would commit it, and starts this binary again,
//! detached, with [`MARKER`] set. That process checks the tree out into a directory of its own,
//! builds its tests in a copy beside the gate's state, lists them as a run does (each `#[case]`,
//! test-case row and matrix cell a test of its own, and the ignored apart), and writes [`RESULT`],
//! tagged with the token `gate open` gave it. `gate check` waits for it, within what
//! [`crate::gate_budget`] allows, and falls back on the source's test attributes, saying so, when it failed or isn't done. `gate close`, and
//! a new `gate open`, stop it and remove what it made.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::testrun::Names;
use crate::workspace::{self, Project};

/// Set in the count's environment, to its token: the binary knows it's the count.
pub const MARKER: &str = "MUTASTIC_GATE_COUNT";

/// The count's result, in the gate's directory.
const RESULT: &str = "gate-count.properties";

/// The running count's process and token, in the gate's directory.
const PID: &str = "gate-count.pid";

/// Whether this process is the count.
pub fn active() -> bool {
    std::env::var_os(MARKER).is_some()
}

/// Where the count builds: beside the gate's state, kept from task to task so its build is warm.
fn build_root(work: &Path) -> PathBuf {
    work.join("gate-count")
}

/// The tree the count checks out, of its own: outside the workspace, so Cargo finds no workspace
/// above it.
fn snapshot(token: &str) -> PathBuf {
    std::env::temp_dir().join(format!("mutastic-gate-count-{token}"))
}

/// A token for one count, so a result from an earlier task's is never read as this one's.
fn token() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    format!("{:x}{:x}", std::process::id(), nanos)
}

/// Starts the count of the tests in `tree`, the working tree of `dir` as `gate open` found it, and
/// returns its token at once; empty when it couldn't start, which a check reports as a count that
/// failed.
pub fn start(dir: &Path, work: &Path, tree: &str) -> String {
    stop(work);
    if tree.is_empty() {
        return String::new();
    }
    let token = token();
    let exe = std::env::var_os("MUTASTIC_EXE")
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok());
    let (Some(exe), Ok(dir)) = (exe, std::fs::canonicalize(dir)) else {
        return String::new();
    };
    let mut command = Command::new(exe);
    command
        .arg(&dir)
        .arg(tree)
        .env(MARKER, &token)
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach(&mut command);
    let Ok(child) = command.spawn() else {
        return String::new();
    };
    let _ = std::fs::create_dir_all(work);
    let _ = std::fs::write(work.join(PID), format!("{} {token}\n", child.id()));
    token
}

/// The count in a process group of its own, so the hook that ran `gate open` can end, or be
/// stopped, without taking the count with it, and `gate close` can stop the count with the builds
/// it started.
#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    command.process_group(0);
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt as _;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(any(unix, windows)))]
fn detach(_: &mut Command) {}

/// Stops a count that's still running, with the builds it started, and removes what it made: its
/// tree, its result and its note of itself. Its build is kept, warm for the next task's.
pub fn stop(work: &Path) {
    let pid_file = work.join(PID);
    if let Ok(text) = std::fs::read_to_string(&pid_file) {
        let mut words = text.split_whitespace();
        if let (Some(pid), Some(token)) = (words.next(), words.next()) {
            kill_group(pid);
            let _ = std::fs::remove_dir_all(snapshot(token));
        }
    }
    let _ = std::fs::remove_file(pid_file);
    let _ = std::fs::remove_file(work.join(RESULT));
}

#[cfg(unix)]
fn kill_group(pid: &str) {
    if pid.parse::<u32>().is_ok() {
        let _ = Command::new("kill")
            .args(["-KILL", "--", &format!("-{pid}")])
            .stderr(Stdio::null())
            .status();
    }
}

#[cfg(windows)]
fn kill_group(pid: &str) {
    if pid.parse::<u32>().is_ok() {
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

#[cfg(not(any(unix, windows)))]
fn kill_group(_: &str) {}

/// The count's result once it's written, waiting up to `wait`: the tests, or why there are none.
pub fn wait(work: &Path, token: &str, wait: Duration) -> Result<Names, String> {
    if token.is_empty() {
        return Err("the count couldn't start".into());
    }
    let until = Instant::now() + wait;
    loop {
        if let Some(result) = read(work, token) {
            let _ = std::fs::remove_file(work.join(PID));
            return result;
        }
        if Instant::now() >= until {
            return Err(format!("the count didn't finish in {}s", wait.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// [`RESULT`], when it's this count's.
fn read(work: &Path, token: &str) -> Option<Result<Names, String>> {
    let text = std::fs::read_to_string(work.join(RESULT)).ok()?;
    let mut names = Names::default();
    let mut ours = false;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "token" => ours = value == token,
            "failed" => return ours.then(|| Err(value.to_string())),
            "run" => {
                names.run.insert(value.to_string());
            }
            "ignored" => {
                names.ignored.insert(value.to_string());
            }
            _ => {}
        }
    }
    ours.then_some(Ok(names))
}

/// The count itself: `args` are the binary, the package's directory and the tree. It writes
/// [`RESULT`] whatever happens, so a check needn't wait out its time for a count that failed.
pub fn run(args: &[std::ffi::OsString]) -> std::process::ExitCode {
    let token = std::env::var(MARKER).unwrap_or_default();
    let (Some(dir), Some(tree)) = (args.get(1), args.get(2)) else {
        return std::process::ExitCode::FAILURE;
    };
    let dir = PathBuf::from(dir);
    let tree = tree.to_string_lossy().into_owned();
    let Ok(project) = workspace::read(&dir.display().to_string()) else {
        return std::process::ExitCode::FAILURE;
    };
    let work = project.mutastic_dir();
    let listed = count(&project, &dir, &tree, &token);
    let mut lines = vec![format!("token={token}")];
    match listed {
        Ok(names) => {
            lines.extend(names.run.iter().map(|name| format!("run={name}")));
            lines.extend(names.ignored.iter().map(|name| format!("ignored={name}")));
        }
        Err(why) => {
            let first = why.lines().next().unwrap_or_default();
            lines.push(format!("failed={first}"));
        }
    }
    let written = work.join(format!("{RESULT}.{token}"));
    if std::fs::write(&written, lines.join("\n") + "\n").is_ok() {
        let _ = std::fs::rename(&written, work.join(RESULT));
    }
    let _ = std::fs::remove_dir_all(snapshot(&token));
    std::process::ExitCode::SUCCESS
}

/// The tests in `tree`: checked out of git into a directory of its own, and listed as the gate
/// lists the package's, in a copy built beside the gate's state.
fn count(project: &Project, dir: &Path, tree: &str, token: &str) -> Result<Names, String> {
    let checked_out = snapshot(token);
    let _ = std::fs::remove_dir_all(&checked_out);
    std::fs::create_dir_all(&checked_out).map_err(|error| error.to_string())?;
    let prefix = crate::since::git(dir, &["rev-parse", "--show-prefix"]).unwrap_or_default();
    let index = std::env::temp_dir().join(format!("mutastic-gate-count-{token}.index"));
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_INDEX_FILE", &index)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    };
    let into = format!("{}/", workspace::slashed(&checked_out));
    let copied =
        git(&["read-tree", tree]) && git(&["checkout-index", "-a", &format!("--prefix={into}")]);
    let _ = std::fs::remove_file(&index);
    if !copied {
        return Err("git couldn't check out the tree the task started from".into());
    }
    let package = checked_out.join(prefix.trim());
    let mut counted =
        workspace::read(&package.display().to_string()).map_err(|failure| failure.message)?;
    counted.target = build_root(&project.mutastic_dir());
    crate::gate::listed(&counted).map_err(|failure| failure.message)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A result is read only when it's this count's, a failure says why, and a count that isn't
    /// done is waited for only as long as asked.
    #[test]
    fn a_result_is_this_count_s_or_none() {
        let work = std::env::temp_dir().join(format!("mutastic-count-{}", std::process::id()));
        std::fs::create_dir_all(&work).unwrap();
        let result = work.join(RESULT);
        assert_eq!(
            wait(&work, "t1", Duration::ZERO),
            Err("the count didn't finish in 0s".into())
        );
        std::fs::write(&result, "token=t0\nrun=tests::a\n").unwrap();
        assert!(read(&work, "t1").is_none(), "an earlier task's");
        std::fs::write(&result, "token=t1\nrun=tests::a\nignored=tests::b\n").unwrap();
        let names = wait(&work, "t1", Duration::ZERO).unwrap();
        assert_eq!(names.run.len(), 1);
        assert!(names.ignored.contains("tests::b"));
        std::fs::write(&result, "token=t1\nfailed=it doesn't build\n").unwrap();
        assert_eq!(read(&work, "t1"), Some(Err("it doesn't build".into())));
        assert_eq!(
            wait(&work, "", Duration::ZERO),
            Err("the count couldn't start".into())
        );
        stop(&work);
        assert!(!result.exists());
        std::fs::remove_dir_all(&work).ok();
    }
}
