//! A run split across machines, put back together, as the Java edition does it. Each share
//! (`--shard=N/M`) keeps its events, as `--format=jsonl` writes them, in a folder of its own,
//! `shares/N-of-M` in `target/mutastic`, beside its own report files; and `run --from-shards=DIR`
//! reads every share's from DIR, wherever in it they are, and is the whole run made of them: the
//! mutants found again from the code, as the whole run finds them, each with the result its share
//! gave it.
//!
//! The shares must be every share of one run: each of the same count, there once and finished,
//! made by the same mutastic on the same code, with no mutant in two. Anything else is refused
//! rather than made into a score, since a score made of the wrong parts passes or fails wrongly.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::engine::{self, MutantResult, Options, Progress, Report, Status};
use crate::flaky::Flaky;
use crate::workspace::Project;

/// The file a share keeps its events in, as `--format=jsonl` writes them.
pub const EVENTS: &str = "events.jsonl";

/// One of a run's shares of the mutants: the `number`th of `of`, dealt by each mutant's id alone,
/// so a mutant is in exactly one share and the shares' results together are the whole run's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shard {
    pub number: usize,
    pub of: usize,
}

impl Shard {
    /// The most shares a run is split into, as in the Java edition: more machines than this each
    /// start a build for little.
    pub const MOST: usize = 64;

    /// `2/4`, as `--shard` takes it.
    pub fn parse(value: &str) -> Result<Shard, String> {
        let Some((number, of)) = value.split_once('/') else {
            return Err(format!(
                "--shard needs a share and how many, as 2/4, not {value}"
            ));
        };
        let of = whole_number(of, Shard::MOST)?;
        let number = whole_number(number, of)?;
        Ok(Shard { number, of })
    }

    /// Whether the mutant with this id is in this share. An id is `m` and the first eight hex
    /// digits of a SHA-256 of what the mutant is (see [`crate::id`]), so its digits are already a
    /// hash, the same on every platform, Rust version and run: the share is that number modulo
    /// `of`. std's `Hash` isn't used, as `RandomState` differs from run to run and
    /// `DefaultHasher`'s algorithm isn't promised to stay the same.
    pub fn holds(&self, id: &str) -> bool {
        let digits = id
            .strip_prefix('m')
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .unwrap_or(0);
        digits as usize % self.of == self.number - 1
    }

    /// Where a share keeps its events and its report files, in `target/mutastic`.
    pub fn folder(&self, work: &Path) -> PathBuf {
        work.join("shares")
            .join(format!("{}-of-{}", self.number, self.of))
    }
}

/// `4`, from 1 to `most`, for `--shard`.
fn whole_number(value: &str, most: usize) -> Result<usize, String> {
    value
        .parse()
        .ok()
        .filter(|n| (1..=most).contains(n))
        .ok_or_else(|| format!("--shard: {value} isn't a number from 1 to {most}"))
}

/// Which share of a split run this is, and what it ran on, as `run-started` says it: the shares of
/// one run are of the same count, mutastic and code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub number: usize,
    pub of: usize,
    pub version: String,
    pub code: String,
}

impl Tag {
    pub fn new(shard: Shard, code: String) -> Tag {
        Tag {
            number: shard.number,
            of: shard.of,
            version: env!("CARGO_PKG_VERSION").to_string(),
            code,
        }
    }

    pub fn json(&self) -> Value {
        json!({
            "number": self.number,
            "of": self.of,
            "version": self.version,
            "code": self.code,
        })
    }
}

/// A hash of the code a run tests: every Rust file and Cargo manifest in the workspace, by its
/// path and its bytes, skipping what mutastic's copy skips (`.git` and the target directory). The
/// same in every share of one run, and in the run that puts them together.
pub fn code(project: &Project) -> Result<String, String> {
    let mut files = BTreeMap::new();
    sources(&project.workspace, Path::new(""), project, &mut files)
        .map_err(|error| format!("couldn't read the workspace: {error}"))?;
    let mut hasher = Sha256::new();
    for (path, bytes) in files {
        hasher.update(path.as_bytes());
        hasher.update([0u8]);
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Every `.rs` file and `Cargo.toml` under `relative`, by its path with `/` between the parts.
fn sources(
    root: &Path,
    relative: &Path,
    project: &Project,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let name = entry.file_name();
        let path = relative.join(&name);
        let top_target = name == "target" && relative.as_os_str().is_empty();
        if name == ".git" || top_target || root.join(&path) == project.target {
            continue;
        }
        let kind = entry.file_type()?;
        if kind.is_dir() {
            sources(root, &path, project, files)?;
        } else if kind.is_file()
            && (name == "Cargo.toml" || path.extension() == Some("rs".as_ref()))
        {
            let shown: Vec<String> = path
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect();
            files.insert(shown.join("/"), std::fs::read(root.join(&path))?);
        }
    }
    Ok(())
}

/// Forgets the events a share kept in `file` before: a share that doesn't finish this time
/// mustn't leave the last time's to be put together with the other shares'.
pub fn forget(file: &Path) -> Result<(), String> {
    match std::fs::remove_file(file) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("couldn't remove {}: {error}", file.display()))
        }
        _ => Ok(()),
    }
}

/// Keeps a share's events in `file`.
pub fn keep(file: &Path, events: &str) -> Result<(), String> {
    let written = file
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(file, events));
    written.map_err(|error| format!("couldn't write {}: {error}", file.display()))
}

/// One share, read: where from, which it is, and its events.
struct Read {
    file: PathBuf,
    tag: Tag,
    started: Value,
    /// Each mutant's last result, its `mutant-revised` or else its `mutant-finished`, by id.
    results: BTreeMap<String, Value>,
    finished: Option<Value>,
}

impl Read {
    fn named(&self) -> String {
        format!(
            "share {} of {} ({})",
            self.tag.number,
            self.tag.of,
            self.file.display()
        )
    }

    fn project(&self) -> &str {
        self.started["project"].as_str().unwrap_or_default()
    }

    /// What the share ran that the whole run can't be put together from: a change's lines.
    fn partial(&self) -> Option<&'static str> {
        let finished = self.finished.as_ref()?;
        finished
            .get("scope")
            .map(|_| "ran only a change's lines (--since)")
    }

    /// A number `run-finished` gives, `0` when it doesn't.
    fn finished_number(&self, pointer: &str) -> u64 {
        self.finished
            .as_ref()
            .and_then(|finished| finished.pointer(pointer))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    }
}

/// A run's shares, read from a directory and checked to be every share of one run.
pub struct Shares {
    directory: PathBuf,
    shares: Vec<Read>,
}

impl Shares {
    /// Every share's events in `directory`, at any depth, checked to be all of one run's, made by
    /// this mutastic, `version`.
    pub fn read(directory: &Path, version: &str) -> Result<Shares, String> {
        let mut shares = Vec::new();
        for file in events_in(directory)? {
            shares.push(share(&file)?);
        }
        if shares.is_empty() {
            return Err(format!(
                "{} holds no share's {EVENTS}: each run with --shard keeps its own in \
                 shares/N-of-M, in target/mutastic",
                directory.display()
            ));
        }
        shares.sort_by_key(|read| read.tag.number);
        let found = Shares {
            directory: directory.to_path_buf(),
            shares,
        };
        found.check_counts()?;
        found.check_finished()?;
        found.check_made_alike(version)?;
        found.check_apart()?;
        Ok(found)
    }

    /// Every share of the same count, each there once.
    fn check_counts(&self) -> Result<(), String> {
        let first = &self.shares[0];
        if let Some(other) = self.shares.iter().find(|read| read.tag.of != first.tag.of) {
            return Err(format!(
                "{} and {} are shares of runs split differently: put together only one run's \
                 shares",
                first.named(),
                other.named()
            ));
        }
        let mut seen: BTreeMap<usize, &Read> = BTreeMap::new();
        for read in &self.shares {
            if let Some(before) = seen.insert(read.tag.number, read) {
                return Err(format!(
                    "share {} of {} is there twice, in {} and {}",
                    read.tag.number,
                    read.tag.of,
                    before.file.display(),
                    read.file.display()
                ));
            }
        }
        match (1..=first.tag.of).find(|number| !seen.contains_key(number)) {
            Some(missing) => Err(format!(
                "share {missing} of {} is missing from {}: its run didn't keep its {EVENTS} there",
                first.tag.of,
                self.directory.display()
            )),
            None => Ok(()),
        }
    }

    fn check_finished(&self) -> Result<(), String> {
        for read in &self.shares {
            if read.finished.is_none() {
                return Err(format!(
                    "{} didn't finish: its events stop before run-finished",
                    read.named()
                ));
            }
            if let Some(partial) = read.partial() {
                return Err(format!(
                    "{} {partial}, so it isn't a share of a whole run",
                    read.named()
                ));
            }
        }
        Ok(())
    }

    /// Every share made by this mutastic, on the same code as the others.
    fn check_made_alike(&self, version: &str) -> Result<(), String> {
        let first = &self.shares[0];
        for read in &self.shares {
            if read.tag.version != version {
                return Err(format!(
                    "{} was run by mutastic {}, and this is {version}: run the shares and put \
                     them together with the same mutastic",
                    read.named(),
                    read.tag.version
                ));
            }
            if read.tag.code != first.tag.code {
                return Err(format!(
                    "{} and {} ran on different code: every share of a run runs on the same \
                     commit",
                    first.named(),
                    read.named()
                ));
            }
        }
        Ok(())
    }

    /// No mutant in two shares.
    fn check_apart(&self) -> Result<(), String> {
        let mut holder: BTreeMap<&str, &Read> = BTreeMap::new();
        for read in &self.shares {
            for id in read.results.keys() {
                if let Some(before) = holder.insert(id, read) {
                    return Err(format!(
                        "{id} is in {} and in {}: a mutant is in one share of a run",
                        before.named(),
                        read.named()
                    ));
                }
            }
        }
        Ok(())
    }

    /// How many shares there are, and how many mutants they ran, as the progress shows it.
    pub fn summary(&self) -> String {
        let mutants: usize = self.shares.iter().map(|read| read.results.len()).sum();
        format!(
            "shares    {} from {} · {mutants} mutant{}",
            self.shares.len(),
            self.directory.display(),
            if mutants == 1 { "" } else { "s" }
        )
    }

    /// The whole run of `project`, whose code hashes to `code`: the mutants found as a run with
    /// `options` finds them, none run, each with its share's result, told to `progress` as a run
    /// tells it.
    pub fn whole(
        &self,
        project: &Project,
        code: &str,
        options: &Options,
        progress: &(dyn Fn(Progress) + Sync),
    ) -> Result<Report, String> {
        let first = &self.shares[0];
        if first.project() != project.package {
            return Err(format!(
                "the shares in {} are of {}, not {}",
                self.directory.display(),
                first.project(),
                project.package
            ));
        }
        if first.tag.code != code {
            return Err(format!(
                "the shares in {} ran on other code than this: put them together on the commit \
                 they ran on",
                self.directory.display()
            ));
        }
        let (found, stale_ignores) = engine::find_for_run(project, options)?;
        let results = self.results(&found)?;
        let summary = self.summary();
        progress(Progress::FromShares {
            summary: &summary,
            mutants: results.len(),
        });
        let order: Vec<usize> = (0..found.mutants.len()).collect();
        let files = engine::Files::new(&found, &order);
        for result in &results {
            files.finished(&found, result, (0, std::time::Duration::ZERO), progress);
        }
        Ok(self.report(project, found, results, stale_ignores, options))
    }

    /// Each mutant a run finds, with the result its share gave it.
    fn results(&self, found: &crate::sites::Crate) -> Result<Vec<MutantResult>, String> {
        let mut by_id: BTreeMap<&str, &Value> = self
            .shares
            .iter()
            .flat_map(|read| read.results.iter())
            .map(|(id, event)| (id.as_str(), event))
            .collect();
        let mut results = Vec::new();
        for (index, mutant) in found.mutants.iter().enumerate() {
            let result = by_id
                .remove(mutant.id.as_str())
                .and_then(|event| result_of(index, event))
                .filter(|result| skipped_alike(mutant.skipped.is_some(), result))
                .ok_or_else(|| self.unlike(&mutant.id))?;
            results.push(result);
        }
        match by_id.keys().next() {
            Some(id) => Err(self.unlike(id)),
            None => Ok(results),
        }
    }

    fn unlike(&self, id: &str) -> String {
        format!(
            "the shares in {} didn't run the mutants this run finds ({id}, for one): they ran \
             with other operators, settings or ignores than these",
            self.directory.display()
        )
    }

    /// The whole run's report: the shares' tests and flaky tests together, the most workers any
    /// ran side by side, and the longest share's times.
    fn report(
        &self,
        project: &Project,
        found: crate::sites::Crate,
        results: Vec<MutantResult>,
        stale_ignores: Vec<String>,
        options: &Options,
    ) -> Report {
        let most = |pointer: &str| {
            self.shares
                .iter()
                .map(|read| read.finished_number(pointer))
                .max()
                .unwrap_or(0)
        };
        let millis = |pointer: &str| Duration::from_millis(most(pointer));
        let (took, setup, coverage) = (
            millis("/durationMs"),
            millis("/build/setupMs"),
            millis("/coverageMs"),
        );
        let first = &self.shares[0];
        // A share with no mutant to run built nothing, so its profile has no opt-level.
        let profile = &self
            .shares
            .iter()
            .find(|read| !read.started["profile"]["optLevel"].is_null())
            .unwrap_or(first)
            .started["profile"];
        let rustc = first
            .finished
            .as_ref()
            .and_then(|finished| finished["toolchain"]["version"].as_str())
            .unwrap_or_default()
            .to_string();
        Report {
            project: project.clone(),
            found,
            test_runs: results.iter().map(|result| result.tests_run).sum(),
            results,
            tests: most("/tests") as usize,
            rounds: most("/build/rounds") as usize,
            workers: (most("/workers") as usize).max(1),
            rustc,
            threshold: options.threshold,
            setup,
            build: std::time::Duration::ZERO,
            coverage,
            mutants: took.saturating_sub(setup + coverage),
            checking: None,
            streamed: false,
            took,
            unsafe_mode: options.unsafe_mode,
            profile: json!({
                "opt_level": profile["optLevel"],
                "overflow_checks": profile["overflowChecks"],
                "debug_assertions": profile["debugAssertions"],
            }),
            started_at: engine::clock(),
            reports: Vec::new(),
            stale_ignores,
            scope: None,
            flaky: self.flaky(),
            // A share's `mutant-revised` has replaced its `mutant-finished`: the whole run's
            // events give each mutant's last result once.
            revised: BTreeMap::new(),
            share: None,
            operators: options.operators.clone(),
            comparison: None,
            fail_on_new: None,
            built: true,
            only: false,
        }
    }

    /// Every share's flaky tests, each once.
    fn flaky(&self) -> Vec<Flaky> {
        let mut seen = BTreeSet::new();
        let mut flaky = Vec::new();
        for read in &self.shares {
            let evidence = read
                .finished
                .as_ref()
                .and_then(|finished| finished["flakyEvidence"].as_array().cloned())
                .unwrap_or_default();
            for found in evidence {
                let test = found["test"].as_str().unwrap_or_default().to_string();
                if seen.insert(test.clone()) {
                    flaky.push(Flaky {
                        test,
                        fails_alone: found["failsAlone"] == true,
                        reaches: strings(&found["reaches"]),
                        reaches_in_test: strings(&found["reachesInTest"]),
                    });
                }
            }
        }
        flaky
    }
}

/// Whether a share skipped a mutant as the whole run would: one the run skips (an ignore, a rule,
/// a filter) the share skipped too, and one it doesn't the share ran, unless it didn't compile
/// there, which only a build can tell.
fn skipped_alike(skipped_here: bool, result: &MutantResult) -> bool {
    match (skipped_here, result.status) {
        (true, status) => status == Status::Ignored,
        (false, Status::Ignored) => result
            .reason
            .as_deref()
            .is_some_and(|reason| reason.starts_with("unviable:")),
        (false, _) => true,
    }
}

/// Every share's events file in `directory`, at any depth, in the order of their paths.
fn events_in(directory: &Path) -> Result<Vec<PathBuf>, String> {
    if !directory.is_dir() {
        return Err(format!("there's no directory {}", directory.display()));
    }
    let mut found = Vec::new();
    let mut stack = vec![directory.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .map_err(|error| format!("couldn't read {}: {error}", dir.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if entry.file_name() == EVENTS {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}

/// One share's events, read from `file`.
fn share(file: &Path) -> Result<Read, String> {
    let not_events = |why: String| format!("{} isn't a share's events: {why}", file.display());
    let text = std::fs::read_to_string(file).map_err(|error| not_events(error.to_string()))?;
    let mut started = None;
    let mut results = BTreeMap::new();
    let mut finished = None;
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event: Value = serde_json::from_str(line)
            .map_err(|error| not_events(format!("line {} isn't JSON ({error})", index + 1)))?;
        match event["reason"].as_str() {
            Some("run-started") => started = Some(event),
            // A `mutant-revised` comes after the mutant's `mutant-finished`, and replaces it.
            Some("mutant-finished" | "mutant-revised") => {
                let id = event["id"].as_str().unwrap_or_default().to_string();
                results.insert(id, event);
            }
            Some("run-finished") => finished = Some(event),
            _ => {}
        }
    }
    let started = started.ok_or_else(|| not_events("it has no run-started".into()))?;
    let tag = tag_of(&started["share"])
        .ok_or_else(|| not_events("its run-started names no share".into()))?;
    Ok(Read {
        file: file.to_path_buf(),
        tag,
        started,
        results,
        finished,
    })
}

fn tag_of(share: &Value) -> Option<Tag> {
    Some(Tag {
        number: share["number"].as_u64()? as usize,
        of: share["of"].as_u64()? as usize,
        version: share["version"].as_str()?.to_string(),
        code: share["code"].as_str()?.to_string(),
    })
}

/// A share's result for the mutant at `index`, from its `mutant-finished`; `None` for a status
/// this mutastic doesn't know.
fn result_of(index: usize, event: &Value) -> Option<MutantResult> {
    let text = |key: &str| event[key].as_str().map(str::to_string);
    Some(MutantResult {
        mutant: index,
        status: Status::from_schema_name(event["status"].as_str()?)?,
        reason: text("statusReason"),
        covered_by: strings(&event["coveredBy"]),
        killed_by: text("caughtBy"),
        killed_with: text("caughtWith"),
        false_catch_by: text("falseCatchBy"),
        tests_run: event["testsRun"].as_u64().unwrap_or(0) as usize,
        took: Duration::from_millis(event["durationMs"].as_u64().unwrap_or(0)),
        inputs: event["inputs"]
            .as_object()
            .map(|inputs| {
                inputs
                    .iter()
                    .map(|(name, values)| (name.clone(), numbers(values)))
                    .collect()
            })
            .unwrap_or_default(),
        suggestion: suggestion_of(&event["suggestion"]),
    })
}

/// The suggestion a card is written from, from the event's `suggestion`: the receiver's numbers
/// first, then the function's, as the probe found them.
fn suggestion_of(suggestion: &Value) -> Option<crate::suggest::Suggestion> {
    let mut values: Vec<i128> = suggestion["receiver"]
        .as_array()?
        .iter()
        .flat_map(|instance| numbers(&instance["arguments"]))
        .collect();
    values.extend(numbers(&suggestion["arguments"]));
    Some(crate::suggest::Suggestion {
        values,
        today: suggestion["today"].as_str()?.to_string(),
        mutated: suggestion["withMutant"].as_str()?.to_string(),
        built_by: suggestion["builtBy"].as_str().map(str::to_string),
    })
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn numbers(value: &Value) -> Vec<i128> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_i64()
                        .map(i128::from)
                        .or_else(|| item.as_u64().map(i128::from))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A whole number as JSON keeps it: within `i64`, or `u64` above it, as the events write inputs
/// and a suggestion's numbers. One beyond both, which only an `i128` or `u128` holds, is written
/// at the nearest end.
pub fn whole(value: i128) -> Value {
    match (i64::try_from(value), u64::try_from(value)) {
        (Ok(small), _) => json!(small),
        (_, Ok(large)) => json!(large),
        _ if value < 0 => json!(i64::MIN),
        _ => json!(u64::MAX),
    }
}

#[cfg(test)]
mod tests {
    use super::Shard;

    #[test]
    fn a_share_is_read_as_n_of_m() {
        assert_eq!(Shard::parse("2/4"), Ok(Shard { number: 2, of: 4 }));
        assert_eq!(Shard::parse("1/1"), Ok(Shard { number: 1, of: 1 }));
        assert_eq!(Shard::parse("64/64"), Ok(Shard { number: 64, of: 64 }));
        for wrong in ["0/4", "5/4", "1/65", "1/0", "a/4", "2", "2/4/8", "-1/4", ""] {
            assert!(Shard::parse(wrong).is_err(), "{wrong}");
        }
    }

    #[test]
    fn each_mutant_is_in_exactly_one_share() {
        let ids: Vec<String> = (0..500u32)
            .map(|n| format!("m{:08x}", n.wrapping_mul(2_654_435_761)))
            .collect();
        for of in 1..=7 {
            let mut sizes = vec![0; of];
            for id in &ids {
                let holding: Vec<usize> = (1..=of)
                    .filter(|&number| Shard { number, of }.holds(id))
                    .collect();
                assert_eq!(holding.len(), 1, "{id} in {holding:?} of {of}");
                sizes[holding[0] - 1] += 1;
            }
            // Dealt by a hash, every share gets some.
            assert!(sizes.iter().all(|&size| size > 0), "{sizes:?}");
        }
    }

    #[test]
    fn the_deal_is_the_ids_own_digits() {
        // `m…0` and `m…2` are even, `m…7` odd: the same on every machine and every Rust.
        let half = |number| Shard { number, of: 2 };
        assert!(half(1).holds("mc9e13290"));
        assert!(half(2).holds("mb2f8ae57"));
        assert!(Shard { number: 3, of: 3 }.holds("m00000005"));
    }

    #[test]
    fn a_whole_number_keeps_its_value_where_json_can() {
        assert_eq!(super::whole(-3), serde_json::json!(-3));
        assert_eq!(super::whole(u64::MAX as i128), serde_json::json!(u64::MAX));
        assert_eq!(super::whole(i128::MAX), serde_json::json!(u64::MAX));
        assert_eq!(super::whole(i128::MIN), serde_json::json!(i64::MIN));
    }
}
