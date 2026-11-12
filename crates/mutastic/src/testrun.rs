//! Running the project's tests: libtest binaries, run by mutastic itself with `--exact`, each in a
//! process of its own, so a mutant that loops forever can be killed.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crate::compile::Binary;

#[derive(Clone, Debug)]
pub struct Test {
    /// Index into the binaries.
    pub binary: usize,
    /// libtest's name, `pricing::tests::big_orders`.
    pub name: String,
    /// The name shown: libtest's for the library's tests, with the target for the others
    /// (`pricing::big_orders` for `tests/pricing.rs`).
    pub display: String,
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub passed: bool,
    pub timed_out: bool,
    /// The first test that failed, by libtest's name.
    pub failed: Option<String>,
    /// How it failed: `panic`, or `crash` for a process that died another way.
    pub failure: Option<String>,
    pub took: Duration,
    /// What the tests printed.
    pub stdout: String,
    /// What reached stderr: with `--nocapture`, the tests' own output and panics.
    pub stderr: String,
    /// The signal that ended the process, when one did.
    pub signal: Option<i32>,
}

/// Every test in the binaries that isn't `#[ignore]`d.
pub fn list(binaries: &[Binary], cwd: &Path) -> Result<Vec<Test>, String> {
    let mut tests = Vec::new();
    for (index, binary) in binaries.iter().enumerate() {
        let names = |extra: &[&str]| -> Result<Vec<String>, String> {
            let output = Command::new(&binary.path)
                .args(["--list", "--format=terse"])
                .args(extra)
                .current_dir(cwd)
                .output()
                .map_err(|error| format!("couldn't list {}: {error}", binary.path.display()))?;
            Ok(String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(|line| line.strip_suffix(": test"))
                .map(str::to_string)
                .collect())
        };
        let ignored = names(&["--ignored"])?;
        for name in names(&[])? {
            if ignored.contains(&name) || is_probe(&name) {
                continue;
            }
            let display = if binary.kind == "lib" {
                name.clone()
            } else {
                format!("{}::{name}", binary.target)
            };
            tests.push(Test {
                binary: index,
                name,
                display,
            });
        }
    }
    Ok(tests)
}

/// Whether a test is one of mutastic's probes, which it builds beside a function to call it: not
/// one of the project's tests.
pub fn is_probe(name: &str) -> bool {
    name.rsplit("::")
        .next()
        .is_some_and(|last| last.starts_with(crate::sites::callable::PROBE))
}

/// The probes in the binaries, by libtest's name, with the binary each is in.
pub fn probes(binaries: &[Binary], cwd: &Path) -> std::collections::BTreeMap<String, usize> {
    let mut found = std::collections::BTreeMap::new();
    for (index, binary) in binaries.iter().enumerate() {
        let Ok(output) = Command::new(&binary.path)
            .args(["--list", "--format=terse"])
            .current_dir(cwd)
            .output()
        else {
            continue;
        };
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some(name) = line.strip_suffix(": test").filter(|name| is_probe(name)) {
                found.entry(name.to_string()).or_insert(index);
            }
        }
    }
    found
}

/// Runs the named tests of one binary, with `env` set, killing it after `timeout`.
pub fn run(
    binary: &Binary,
    cwd: &Path,
    names: &[&str],
    env: &[(&str, String)],
    timeout: Duration,
) -> Outcome {
    let started = Instant::now();
    let mut command = Command::new(&binary.path);
    command
        .arg("--exact")
        .args(names)
        .current_dir(cwd)
        .env("CARGO_MANIFEST_DIR", cwd)
        .env("MUTASTIC", "1")
        .env_remove("MUTASTIC_ACTIVE")
        .env_remove("MUTASTIC_COVERAGE")
        .env("RUST_BACKTRACE", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        command.env(key, value);
    }
    let Ok(mut child) = command.spawn() else {
        return Outcome {
            passed: false,
            timed_out: false,
            failed: None,
            failure: Some("couldn't start".into()),
            took: started.elapsed(),
            stdout: String::new(),
            stderr: String::new(),
            signal: None,
        };
    };
    let reader = read_all(child.stdout.take());
    let errors = read_all(child.stderr.take());
    let mut pause = Duration::from_micros(200);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if started.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => {
                std::thread::sleep(pause);
                pause = (pause * 2).min(Duration::from_millis(5));
            }
            Err(_) => break None,
        }
    };
    let took = started.elapsed();
    let text = reader.join().unwrap_or_default();
    let stderr = errors.join().unwrap_or_default();
    let Some(status) = status else {
        return Outcome {
            passed: false,
            timed_out: true,
            failed: None,
            failure: None,
            took,
            stdout: text,
            stderr,
            signal: None,
        };
    };
    let failed = text.lines().find_map(|line| {
        line.strip_prefix("test ")
            .and_then(|rest| rest.strip_suffix(" ... FAILED"))
            .map(str::to_string)
    });
    let failure = if status.success() {
        None
    } else if failed.is_some() {
        Some("panic".into())
    } else {
        Some("crash".into())
    };
    Outcome {
        passed: status.success(),
        timed_out: false,
        failed,
        failure,
        took,
        stdout: text,
        stderr,
        signal: signal_of(&status),
    }
}

fn read_all(stream: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut stream) = stream {
            let _ = stream.read_to_string(&mut text);
        }
        text
    })
}

#[cfg(unix)]
fn signal_of(status: &std::process::ExitStatus) -> Option<i32> {
    std::os::unix::process::ExitStatusExt::signal(status)
}

/// Windows has no signals: a crash ends the process with an exception's status instead. Each is
/// named as the signal the same crash raises elsewhere, so the card reads the same on every
/// platform: Rust's abort (a failed `unsafe` precondition, a panic in a panic, a stack overflow)
/// ends it with `STATUS_STACK_BUFFER_OVERRUN`, which is `SIGABRT` elsewhere.
#[cfg(windows)]
fn signal_of(status: &std::process::ExitStatus) -> Option<i32> {
    windows_signal(status.code()?)
}

#[cfg(any(windows, test))]
fn windows_signal(code: i32) -> Option<i32> {
    // The status is an NTSTATUS, an unsigned code that `code()` gives as an `i32`.
    match code.cast_unsigned() {
        0xC000_0409 | 0xC000_00FD => Some(6), // fast fail (abort), stack overflow
        0xC000_0005 => Some(11),              // access violation
        0xC000_001D | 0xC000_0096 => Some(4), // illegal and privileged instruction
        0xC000_0094 | 0xC000_0095 => Some(8), // integer divide by zero and overflow
        0x8000_0003 => Some(5),               // breakpoint
        _ => None,
    }
}

#[cfg(not(any(unix, windows)))]
fn signal_of(_: &std::process::ExitStatus) -> Option<i32> {
    None
}

/// `SIGABRT` for 6: the names of the signals a crashing test dies by.
pub fn signal_name(signal: i32) -> String {
    let name = match signal {
        4 => "SIGILL",
        5 => "SIGTRAP",
        6 => "SIGABRT",
        8 => "SIGFPE",
        9 => "SIGKILL",
        11 => "SIGSEGV",
        #[cfg(target_os = "linux")]
        7 => "SIGBUS",
        #[cfg(not(target_os = "linux"))]
        10 => "SIGBUS",
        _ => return format!("signal {signal}"),
    };
    name.to_string()
}

/// The message of a panic the language raises itself, `attempt to subtract with overflow`, when
/// that's how the tests failed.
pub fn language_panic(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| {
            (line.starts_with("attempt to ")
                && (line.contains("overflow") || line.contains("zero")))
                || line.starts_with("index out of bounds")
        })
        .map(str::to_string)
}

/// Runs `work` on each item on `workers` threads, keeping the items' order in the results.
/// A panic in `work` is the caller's: it's raised again here, never a shorter result.
pub fn parallel<T: Sync, R: Send>(
    items: &[T],
    workers: usize,
    work: impl Fn(&T) -> R + Sync,
) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let mut results: Vec<(usize, R)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers.max(1))
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(index) else {
                            break;
                        };
                        done.push((index, work(item)));
                    }
                    done
                })
            })
            .collect();
        // A worker that panicked takes its results with it: the run fails, rather than going on as
        // if those items had never been, which a kept record would then compare against.
        handles
            .into_iter()
            .flat_map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    });
    results.sort_by_key(|(index, _)| *index);
    results.into_iter().map(|(_, result)| result).collect()
}

/// Half the cores, at most 4, as the Java edition's workers.
pub fn default_workers() -> usize {
    std::thread::available_parallelism()
        .map(|cores| (cores.get() / 2).clamp(1, 4))
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A worker's panic isn't a shorter result: every item comes back, or the panic does.
    #[test]
    #[should_panic(expected = "the third")]
    fn a_workers_panic_is_raised_not_dropped() {
        let items: Vec<usize> = (0..8).collect();
        let _ = parallel(&items, 3, |&item| {
            assert!(item != 2, "the third");
            item
        });
    }

    #[test]
    fn every_item_comes_back_in_order() {
        let items: Vec<usize> = (0..50).collect();
        assert_eq!(
            parallel(&items, 4, |&item| item * 2),
            (0..50).map(|i| i * 2).collect::<Vec<_>>()
        );
    }

    /// A Windows crash is named as the same crash is elsewhere: Rust's abort as `SIGABRT`.
    #[test]
    fn a_windows_crash_is_named_as_its_signal_elsewhere() {
        let named = |code: u32| windows_signal(code.cast_signed()).map(signal_name);
        assert_eq!(named(0xC000_0409).as_deref(), Some("SIGABRT"));
        assert_eq!(named(0xC000_00FD).as_deref(), Some("SIGABRT"));
        assert_eq!(named(0xC000_0005).as_deref(), Some("SIGSEGV"));
        assert_eq!(named(0xC000_001D).as_deref(), Some("SIGILL"));
        assert_eq!(named(0xC000_0094).as_deref(), Some("SIGFPE"));
        assert_eq!(named(0x8000_0003).as_deref(), Some("SIGTRAP"));
        assert_eq!(named(101), None, "a test harness's own exit isn't a crash");
    }
}
