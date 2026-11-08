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
            if ignored.contains(&name) {
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
        .stderr(Stdio::null());
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
        };
    };
    let mut stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(out) = stdout.as_mut() {
            let _ = out.read_to_string(&mut text);
        }
        text
    });
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
    let Some(status) = status else {
        return Outcome {
            passed: false,
            timed_out: true,
            failed: None,
            failure: None,
            took,
            stdout: text,
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
    }
}

/// Runs `work` on each item on `workers` threads, keeping the items' order in the results.
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
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap_or_default())
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
