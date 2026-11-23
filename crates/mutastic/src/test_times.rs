//! What the tests take at each site, kept in `target/mutastic/test-times.tsv` beside
//! `last-run.tsv`, so `mutastic mutants --operators=…` can say what a set would add to a run before
//! it's run, as the Java edition says it.
//!
//! A whole run writes it, from its coverage pass; so does `mutastic mutants --estimate`, from a
//! coverage pass of every mutant's site. The file is one entry per line, tab-separated: `kind`
//! (`run` or `coverage pass`), `at` (UTC, to the second), `operators` (the ids, sorted), `code`
//! (the hash of the crate's source files), a `base` line for each mutant of that set, a `measured`
//! line for each site the pass measured, and `site <site> <ms>` with the summed time of the tests
//! that reach it, 0 for one no test reaches. A site no pass measured blocks an estimate; one with
//! 0 costs nothing.
//!
//! A mutant's cost is the engine's own expected cost for it: what the tests that reach its site
//! took, or nothing for one that's skipped, a twin, or reached by no test. It's "up to", as a
//! caught mutant stops at its first failing test.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::progress::count;
use crate::sites::{Crate, Mutant};

/// The file's name, in the work directory.
pub const FILE: &str = "test-times.tsv";

/// What measured the times: a run's coverage pass, or one `--estimate` ran on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Run,
    CoveragePass,
}

impl Kind {
    fn word(self) -> &'static str {
        match self {
            Kind::Run => "run",
            Kind::CoveragePass => "coverage pass",
        }
    }

    fn named(word: &str) -> Option<Kind> {
        [Kind::Run, Kind::CoveragePass]
            .into_iter()
            .find(|kind| kind.word() == word)
    }
}

/// The times a coverage pass measured, and what they were measured on.
#[derive(Clone, Debug, PartialEq)]
pub struct TestTimes {
    pub kind: Kind,
    /// When, `2026-11-22T14:03:00Z`.
    pub at: String,
    /// The ids of the operators whose mutants are `base`, sorted and comma-joined.
    pub operators: String,
    /// The hash of the crate's source files ([`code`]).
    pub code: String,
    /// The ids of the mutants of `operators`: a set's mutants beyond them are what it adds.
    pub base: BTreeSet<String>,
    /// Each site measured ([`site`]), with the summed time of the tests that reach it, to the
    /// millisecond.
    pub sites: BTreeMap<String, Duration>,
}

/// What an estimate is made against: the code as it is, the project's own operators, and the
/// mutants listed, as a run would find them.
pub struct Now<'a> {
    pub code: &'a str,
    /// The project's operators, sorted and comma-joined: `mutastic.yaml`'s, or the defaults.
    pub configured: &'a str,
    pub mutants: &'a [Mutant],
}

impl TestTimes {
    /// The record in `file`: none if there's none, or it can't be read, as an unreadable record
    /// is no record.
    pub fn read(file: &Path) -> Option<TestTimes> {
        let text = std::fs::read_to_string(file).ok()?;
        let mut fields: BTreeMap<&str, &str> = BTreeMap::new();
        let mut base = BTreeSet::new();
        let mut measured = BTreeSet::new();
        let mut timed = BTreeMap::new();
        for line in text.lines() {
            match line.split('\t').collect::<Vec<_>>().as_slice() {
                ["base", id] => {
                    base.insert(id.to_string());
                }
                ["measured", site] => {
                    measured.insert(site.to_string());
                }
                ["site", site, millis] => {
                    timed.insert(
                        site.to_string(),
                        Duration::from_millis(millis.parse().ok()?),
                    );
                }
                [name @ ("kind" | "at" | "operators" | "code"), value] => {
                    fields.insert(name, value);
                }
                _ => return None,
            }
        }
        let at = fields.get("at")?.to_string();
        when(&at)?;
        let sites = measured
            .into_iter()
            .map(|site| {
                let took = timed.get(&site).copied().unwrap_or_default();
                (site, took)
            })
            .collect();
        Some(TestTimes {
            kind: Kind::named(fields.get("kind")?)?,
            at,
            operators: fields.get("operators")?.to_string(),
            code: fields.get("code")?.to_string(),
            base,
            sites,
        })
    }

    /// Writes it to `file`, through a file beside it renamed into place, so a reader meanwhile
    /// reads the last record or this one, never half of one.
    pub fn write(&self, file: &Path) -> std::io::Result<()> {
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let partial = file.with_extension(format!("tsv.{}", std::process::id()));
        std::fs::write(&partial, self.lines())?;
        std::fs::rename(&partial, file)
    }

    fn lines(&self) -> String {
        let mut out = format!(
            "kind\t{}\nat\t{}\noperators\t{}\ncode\t{}\n",
            self.kind.word(),
            self.at,
            self.operators,
            self.code
        );
        for id in &self.base {
            out.push_str(&format!("base\t{id}\n"));
        }
        for site in self.sites.keys() {
            out.push_str(&format!("measured\t{site}\n"));
        }
        for (site, took) in &self.sites {
            out.push_str(&format!("site\t{site}\t{}\n", kept(*took).as_millis()));
        }
        out
    }

    /// `the run of 22 Nov 2026 14:03 UTC`, `the coverage pass of …`: where an estimate is from.
    pub fn source(&self) -> String {
        format!(
            "the {} of {}",
            self.kind.word(),
            when(&self.at).unwrap_or_else(|| self.at.clone())
        )
    }

    /// What `mutant` should take, as the engine expects it: nothing when it's skipped (a twin
    /// among them), else what the tests that reach its site took; none when its site wasn't
    /// measured.
    pub fn cost(&self, mutant: &Mutant) -> Option<Duration> {
        if mutant.skipped.is_some() {
            return Some(Duration::ZERO);
        }
        self.sites.get(&site(mutant)).copied().map(kept)
    }

    /// The line before `mutants`' count with `--operators`: what the mutants listed add to the
    /// tests' time over this record's, or why there's no estimate. `named` names a set of
    /// operators by its ids: `defaults`, or `5 operators`.
    pub fn against(&self, now: &Now, named: impl Fn(&str) -> String) -> String {
        let source = self.source();
        if self.code != now.code {
            return format!("no estimate: the code has changed since {source}");
        }
        let base = named(&self.operators);
        if self.operators != now.configured {
            return format!(
                "no estimate: {source} was under {base}, not this project's {}",
                named(now.configured)
            );
        }
        let extra: Vec<&Mutant> = now
            .mutants
            .iter()
            .filter(|mutant| !self.base.contains(&mutant.id))
            .collect();
        let costs: Vec<Option<Duration>> = extra.iter().map(|mutant| self.cost(mutant)).collect();
        let unmeasured = costs.iter().filter(|cost| cost.is_none()).count();
        if unmeasured > 0 {
            return format!(
                "no estimate: {unmeasured} of the {} {} in code {source} didn't measure; add \
                 --estimate to measure it all",
                count(extra.len(), "more mutant"),
                if unmeasured == 1 { "is" } else { "are" }
            );
        }
        if extra.is_empty() {
            return format!("estimate: no more mutants than {base}, from {source}");
        }
        let more: Duration = costs.into_iter().flatten().sum();
        format!(
            "estimate: {} than {base}, up to {} more of tests, from {source}",
            count(extra.len(), "more mutant"),
            crate::report::duration(more)
        )
    }

    /// `estimate: stronger, 42 mutants, up to 9.8s of tests, from …`: what every mutant of the set
    /// `name` should take, all told.
    pub fn whole_set(&self, name: &str, mutants: &[Mutant]) -> String {
        let total: Duration = mutants.iter().filter_map(|mutant| self.cost(mutant)).sum();
        format!(
            "estimate: {name}, {}, up to {} of tests, from {}",
            count(mutants.len(), "mutant"),
            crate::report::duration(total),
            self.source()
        )
    }
}

/// A time as the record keeps it, and so as every estimate counts it, read back or not: to the
/// millisecond, rounded up, so a site some test reaches never reads as one none does.
fn kept(took: Duration) -> Duration {
    let millis = took.as_micros().div_ceil(1000);
    Duration::from_millis(u64::try_from(millis).unwrap_or(u64::MAX))
}

/// A set of operators as a record keeps it: the ids, sorted and comma-joined.
pub fn joined(operators: &[&str]) -> String {
    let mut sorted = operators.to_vec();
    sorted.sort_unstable();
    sorted.join(",")
}

/// The line for `--operators` with no record to estimate from.
pub const NO_RECORD: &str =
    "no estimate: no run is recorded here yet; run once to get estimates, or add --estimate";

/// `src/lib.rs:120-131`: where a mutant is, by its file and the bytes of its site, as the coverage
/// pass measures it. Every mutant at one site shares it.
pub fn site(mutant: &Mutant) -> String {
    let file: Vec<String> = mutant
        .file
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    format!(
        "{}:{}-{}",
        file.join("/"),
        mutant.site.start,
        mutant.site.end
    )
}

/// The hash of the crate's source files, the code its mutants are made from, without building
/// anything: what `mutants` can read to tell whether a record still holds. Test files outside
/// the crate's sources are left out, as `mutants` reads none.
pub fn code(found: &Crate) -> String {
    let mut hasher = Sha256::new();
    for file in &found.files {
        let path = file.path.to_string_lossy();
        hasher.update(path.as_bytes());
        hasher.update([0u8]);
        hasher.update((file.text.len() as u64).to_le_bytes());
        hasher.update(file.text.as_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// `22 Nov 2026 14:03 UTC` for `2026-11-22T14:03:00Z`; none for a time not written that way.
pub fn when(at: &str) -> Option<String> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let (date, time) = at.strip_suffix('Z')?.split_once('T')?;
    let mut parts = date.split('-');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    let month = MONTHS.get(month.parse::<usize>().ok()?.checked_sub(1)?)?;
    let day: u32 = day.parse().ok()?;
    let year: u32 = year.parse().ok()?;
    let minutes = time
        .get(..5)
        .filter(|hm| hm.as_bytes().get(2) == Some(&b':'))?;
    Some(format!("{day} {month} {year} {minutes} UTC"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> TestTimes {
        TestTimes {
            kind: Kind::Run,
            at: "2026-11-22T14:03:00Z".into(),
            operators: "boundary,math".into(),
            code: "c".into(),
            base: BTreeSet::from(["m1".to_string()]),
            sites: BTreeMap::from([
                ("src/lib.rs:10-20".to_string(), Duration::from_millis(2100)),
                ("src/lib.rs:30-40".to_string(), Duration::ZERO),
            ]),
        }
    }

    fn mutant(id: &str, site: std::ops::Range<usize>) -> Mutant {
        let found = crate::sites::find(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pricing"),
            "pricing",
            &[std::path::PathBuf::from("src/lib.rs")],
            crate::sites::DEFAULTS,
        );
        let mut mutant = found
            .mutants
            .into_iter()
            .next()
            .expect("the fixture's mutants");
        mutant.id = id.to_string();
        mutant.file = "src/lib.rs".into();
        mutant.site = site;
        mutant.skipped = None;
        mutant
    }

    fn now<'a>(mutants: &'a [Mutant]) -> Now<'a> {
        Now {
            code: "c",
            configured: "boundary,math",
            mutants,
        }
    }

    fn named(ids: &str) -> String {
        if ids == "boundary,math" {
            "defaults".into()
        } else {
            format!("{} operators", ids.split(',').count())
        }
    }

    #[test]
    fn a_record_reads_back_as_it_was_written() {
        let file = std::env::temp_dir().join(format!("mutastic-times-{}.tsv", std::process::id()));
        record().write(&file).unwrap();
        let read = TestTimes::read(&file);
        std::fs::write(&file, "kind\trun\nat\tyesterday\noperators\tx\ncode\tc\n").unwrap();
        let badly_dated = TestTimes::read(&file);
        std::fs::write(&file, "kind\trun\nsomething else\n").unwrap();
        let unreadable = TestTimes::read(&file);
        std::fs::remove_file(&file).ok();
        assert_eq!(read, Some(record()));
        assert_eq!(badly_dated, None);
        assert_eq!(unreadable, None);
    }

    #[test]
    fn a_time_reads_as_a_date_in_utc() {
        assert_eq!(
            when("2026-11-22T14:03:59Z").as_deref(),
            Some("22 Nov 2026 14:03 UTC")
        );
        assert_eq!(
            when("2027-01-05T09:00:00Z").as_deref(),
            Some("5 Jan 2027 09:00 UTC")
        );
        assert_eq!(when("2026-13-01T00:00:00Z"), None);
        assert_eq!(when("22 Nov"), None);
    }

    #[test]
    fn what_a_set_adds_is_the_time_of_the_sites_its_new_mutants_are_at() {
        let mutants = [
            mutant("m1", 10..20),
            mutant("m2", 10..20),
            mutant("m3", 30..40),
        ];
        assert_eq!(
            record().against(&now(&mutants), named),
            "estimate: 2 more mutants than defaults, up to 2.1s more of tests, from the run of \
             22 Nov 2026 14:03 UTC"
        );
        assert_eq!(
            record().against(&now(&mutants[..1]), named),
            "estimate: no more mutants than defaults, from the run of 22 Nov 2026 14:03 UTC"
        );
    }

    #[test]
    fn a_new_mutant_where_no_pass_measured_leaves_no_estimate_unless_it_is_skipped() {
        let mut mutants = [mutant("m2", 10..20), mutant("m3", 50..60)];
        assert_eq!(
            record().against(&now(&mutants), named),
            "no estimate: 1 of the 2 more mutants is in code the run of 22 Nov 2026 14:03 UTC \
             didn't measure; add --estimate to measure it all"
        );
        mutants[1].skipped = Some("twin: same as m2".into());
        assert!(record()
            .against(&now(&mutants), named)
            .starts_with("estimate: 2 more mutants than defaults, up to 2.1s more"));
    }

    #[test]
    fn a_record_of_other_code_or_operators_gives_no_estimate() {
        let mutants = [mutant("m2", 10..20)];
        let mut changed = now(&mutants);
        changed.code = "d";
        assert_eq!(
            record().against(&changed, named),
            "no estimate: the code has changed since the run of 22 Nov 2026 14:03 UTC"
        );
        let mut other = record();
        other.operators = "boundary".into();
        other.kind = Kind::CoveragePass;
        assert_eq!(
            other.against(&now(&mutants), named),
            "no estimate: the coverage pass of 22 Nov 2026 14:03 UTC was under 1 operators, not \
             this project's defaults"
        );
    }

    #[test]
    fn a_whole_set_costs_every_mutant_it_has() {
        let mutants = [mutant("m1", 10..20), mutant("m2", 10..20)];
        assert_eq!(
            record().whole_set("stronger", &mutants),
            "estimate: stronger, 2 mutants, up to 4.2s of tests, from the run of 22 Nov 2026 \
             14:03 UTC"
        );
    }
}
