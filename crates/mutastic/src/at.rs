//! `--at=PATH:LINE[-LINE]`: the mutants on some lines of one of the package's source files, for
//! `run` and `mutants`, as the Java edition takes it (contract decision 24). A person fixing a line,
//! an agent on one function or an editor has a file and a line in hand, not mutant ids.
//!
//! A mutant is on a line when the line overlaps the lines its card points at: its claim's, from
//! the line the edit starts on to the one it ends on, as mutation.json's `location` has them. So a
//! mutant of an expression written over several lines is on each of them. Lines with no mutant on
//! them are refused, with the nearest that have one, never a run of none: a typo in a line number
//! would otherwise pass in silence.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::failure::{Code, Failure};
use crate::sites::{Crate, Mutant};
use crate::workspace::{self, Metadata, Project};

/// What `--at` takes, for its errors.
pub const TAKES: &str =
    "--at takes a source file and a line, as --at=src/lib.rs:17, or :17-20 for lines 17 to 20";

/// The most a line's number can be: no source file is longer.
const MOST: usize = 10_000_000;

/// The most lines a refusal names.
const NEAREST: usize = 5;

/// Lines of a source file that `--at` asks for, as typed: `src/lib.rs:17`, or `:17-20` for lines
/// 17 to 20, 1-based and inclusive. Which file the path names is found once the package is read
/// ([`files`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct At {
    /// The file as typed: absolute, or from where mutastic runs, or from the workspace.
    pub path: String,
    pub from: usize,
    pub to: usize,
}

impl At {
    /// `src/lib.rs:17`, or `src/lib.rs:17-20`: as `--at` takes it, with `file` for the path.
    pub fn typed(&self, file: &str) -> String {
        format!("{file}:{}", self.lines())
    }

    /// `17`, or `17-20`.
    pub fn lines(&self) -> String {
        if self.from == self.to {
            self.from.to_string()
        } else {
            format!("{}-{}", self.from, self.to)
        }
    }
}

/// Each of `--at=PATH:LINE[-LINE][,…]`'s files and lines. The lines are read from the right, so a
/// Windows path's drive, `C:\work\src\lib.rs:17`, stays in the path.
pub fn parse(given: &str) -> Result<Vec<At>, String> {
    if given.trim().is_empty() {
        return Err(format!("--at needs a value. {TAKES}"));
    }
    given
        .split(',')
        .map(|each| {
            if each.trim().is_empty() {
                Err(format!(
                    "--at={given} has an empty one between its commas. {TAKES}"
                ))
            } else {
                one(each.trim())
            }
        })
        .collect()
}

/// One file and its lines.
fn one(typed: &str) -> Result<At, String> {
    let no_line = || format!("--at={typed} has no line. {TAKES}");
    let (path, lines) = typed.rsplit_once(':').ok_or_else(no_line)?;
    let (from, to) = match lines.split_once('-') {
        Some((from, to)) => (from, Some(to)),
        None => (lines, None),
    };
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    if path.is_empty() || !digits(from) || !to.is_none_or(digits) {
        return Err(no_line());
    }
    // `mutants` prints a place to the column, `src/lib.rs:17:15`: the line is the one before it.
    if let Some((file, line)) = path.rsplit_once(':') {
        if !file.is_empty() && digits(line) {
            return Err(format!(
                "--at takes a line, not a column: --at={file}:{line}"
            ));
        }
    }
    let from = line(typed, from)?;
    let last = match to {
        Some(to) => line(typed, to)?,
        None => from,
    };
    if last < from {
        return Err(format!(
            "--at={typed} ends before it starts: --at={path}:{last}-{from}"
        ));
    }
    Ok(At {
        path: path.to_string(),
        from,
        to: last,
    })
}

/// A line's number, from 1.
fn line(typed: &str, number: &str) -> Result<usize, String> {
    let line = if number.len() > MOST.to_string().len() {
        0
    } else {
        number.parse().unwrap_or(0)
    };
    if (1..=MOST).contains(&line) {
        Ok(line)
    } else {
        Err(format!(
            "--at={typed}: a file's lines are numbered from 1, as an editor shows them"
        ))
    }
}

/// Lines of one of the package's source files, asked for by `--at`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lines {
    /// The file from the workspace, as a mutant's `file` names it.
    pub file: PathBuf,
    /// The file from the workspace with `/` between its parts, as a card shows it.
    pub shown: String,
    pub at: At,
}

impl Lines {
    /// `src/lib.rs:17-20`: as `--at` takes it, from the workspace.
    pub fn typed(&self) -> String {
        self.at.typed(&self.shown)
    }

    /// `lib.rs`.
    pub fn name(&self) -> String {
        self.file
            .file_name()
            .map_or_else(|| self.shown.clone(), |name| name.to_string_lossy().into())
    }

    /// `line 17 of lib.rs`, `lines 17–20 of lib.rs`.
    pub fn named(&self) -> String {
        let lines = if self.at.from == self.at.to {
            format!("line {}", self.at.from)
        } else {
            format!("lines {}–{}", self.at.from, self.at.to)
        };
        format!("{lines} of {}", self.name())
    }
}

/// `a`, `a and b`, `a, b and c`: as the Java edition lists things.
pub fn listed(items: &[String], word: &str) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} {word} {last}", rest.join(", ")),
    }
}

/// The files `typed` can name: itself when it's absolute, else from each of `bases`, its parts
/// split at either separator.
fn candidates(typed: &str, bases: &[PathBuf]) -> Vec<PathBuf> {
    let path = PathBuf::from(typed.replace('\\', "/"));
    let all: Vec<PathBuf> = if path.is_absolute() {
        vec![path]
    } else {
        bases.iter().map(|base| base.join(&path)).collect()
    };
    let mut seen = BTreeSet::new();
    all.into_iter()
        .filter(|file| file.is_file())
        .filter_map(|file| std::fs::canonicalize(file).ok())
        .filter(|file| seen.insert(file.clone()))
        .collect()
}

/// Each `--at`'s file among the package's sources, with its lines. A path is taken as given when
/// it's absolute, else from where mutastic runs, from the workspace or from the package.
pub fn files(asked: &[At], project: &Project, found: &Crate) -> Result<Vec<Lines>, Failure> {
    let bases = [
        crate::locate::here(),
        project.workspace.clone(),
        project.workspace.join(&project.package_dir),
    ];
    let sources = source_dirs(&project.roots);
    asked
        .iter()
        .map(|at| file(at, &bases, &project.workspace, found, &sources))
        .collect()
}

/// The directories of the package's crate roots, from the workspace: `src`, or `shop/src`, as the
/// refusal of a file outside them names them.
fn source_dirs(roots: &[PathBuf]) -> Vec<String> {
    let dirs: BTreeSet<PathBuf> = roots
        .iter()
        .filter_map(|root| root.parent().map(Path::to_path_buf))
        .collect();
    dirs.iter()
        .filter(|dir| {
            !dirs
                .iter()
                .any(|other| other != *dir && dir.starts_with(other))
        })
        .map(|dir| workspace::slashed(dir))
        .collect()
}

/// The source file `at` names, its lines; refused when it's none of the package's, or isn't there.
fn file(
    at: &At,
    bases: &[PathBuf],
    workspace: &Path,
    found: &Crate,
    sources: &[String],
) -> Result<Lines, Failure> {
    let there = candidates(&at.path, bases);
    let source = there.iter().find_map(|candidate| {
        found.files.iter().find(|file| {
            std::fs::canonicalize(workspace.join(&file.path)).is_ok_and(|real| &real == candidate)
        })
    });
    if let Some(source) = source {
        return Ok(Lines {
            file: source.path.clone(),
            shown: workspace::slashed(&source.path),
            at: at.clone(),
        });
    }
    let Some(first) = there.first() else {
        return Err(Code::OptionValue.with(format!(
            "there's no file {}, from here or from the project: --at takes a path from either",
            at.path
        )));
    };
    let shown = first
        .strip_prefix(workspace)
        .map_or_else(|_| first.display().to_string(), workspace::slashed);
    let named = if sources.is_empty() {
        "none here".to_string()
    } else {
        listed(sources, "or")
    };
    Err(Code::OutsideSources.with(format!(
        "{shown} isn't in the package's sources: --at takes a line of the code mutastic mutates, \
         in {named}"
    )))
}

/// The lines a mutant's card points at, first to last: its claim's, from the line the edit
/// starts on to the one it ends on, as mutation.json's `location` has them, and the card's own
/// line.
pub fn span(found: &Crate, mutant: &Mutant) -> (usize, usize) {
    let Some(file) = found.files.iter().find(|file| file.path == mutant.file) else {
        return (mutant.line, mutant.line);
    };
    let (range, _) = mutant.claim();
    let starts = crate::sites::line_starts(&file.text);
    let line = |offset: usize| crate::sites::line_of(&starts, offset.min(file.text.len()));
    (
        line(range.start).min(mutant.line),
        line(range.end).max(mutant.line),
    )
}

/// Whether any of lines `from` to `to` is one of `span`'s.
fn overlaps(span: (usize, usize), from: usize, to: usize) -> bool {
    from <= span.1 && to >= span.0
}

/// The ids of `found`'s mutants `chosen` keeps that are on any of `asked`: refused when one of
/// them has none. `dir` is the package's directory as given, for the command a refusal ends in.
pub fn on(
    asked: &[Lines],
    found: &Crate,
    chosen: impl Fn(&Mutant) -> bool,
    dir: &str,
) -> Result<BTreeSet<String>, Failure> {
    let mut ids = BTreeSet::new();
    for lines in asked {
        let in_file: Vec<&Mutant> = found
            .mutants
            .iter()
            .filter(|mutant| mutant.file == lines.file && chosen(mutant))
            .collect();
        let on: Vec<&&Mutant> = in_file
            .iter()
            .filter(|mutant| overlaps(span(found, mutant), lines.at.from, lines.at.to))
            .collect();
        if on.is_empty() {
            return Err(none(lines, &in_file, dir));
        }
        ids.extend(on.iter().map(|mutant| mutant.id.clone()));
    }
    Ok(ids)
}

/// `no mutant on lib.rs:16; there are on 17, 18 and 20`, or `there's one on 17` for one line, and
/// the command that lists them; or, with none in the file, the command that lists the package's.
fn none(lines: &Lines, in_file: &[&Mutant], dir: &str) -> Failure {
    let asked = lines.at.typed(&lines.name());
    let dir = crate::report::shell_word(dir);
    if in_file.is_empty() {
        return Code::NoMutantAt.with(format!(
            "no mutant on {asked}, nor on any other line of {}. To see the package's mutants:\n  \
             mutastic mutants {dir}",
            lines.name()
        ));
    }
    let nearest = nearest(&lines.at, in_file);
    let numbers: Vec<String> = nearest.iter().map(usize::to_string).collect();
    let listed = listed(&numbers, "and");
    let around = At {
        path: lines.at.path.clone(),
        from: nearest[0],
        to: nearest[nearest.len() - 1],
    };
    let (there, them) = if nearest.len() == 1 {
        (format!("there's one on {listed}"), "it")
    } else {
        (format!("there are on {listed}"), "them")
    };
    let command = format!("--at={}", around.typed(&lines.shown));
    Code::NoMutantAt.with(format!(
        "no mutant on {asked}; {there}. To see {them}:\n  mutastic mutants {} {dir}",
        crate::report::shell_word(&command)
    ))
}

/// The lines with a mutant nearest those asked for, at most [`NEAREST`], in order: each mutant's
/// as its card and `mutants` name it.
fn nearest(at: &At, in_file: &[&Mutant]) -> Vec<usize> {
    let lines: BTreeSet<usize> = in_file.iter().map(|mutant| mutant.line).collect();
    let distance = |line: usize| line.abs_diff(at.from).min(line.abs_diff(at.to));
    let mut by_distance: Vec<usize> = lines.into_iter().collect();
    by_distance.sort_by_key(|&line| (distance(line), line));
    by_distance.truncate(NEAREST);
    by_distance.sort_unstable();
    by_distance
}

/// The directory of the workspace member `--at`'s first file is in, when `given` names none: it's
/// the workspace's root, or a directory in none of its members. `run --at=shop/src/lib.rs:4` at a
/// workspace's root runs `shop`, as a directory inside a member runs that member. `None` when
/// `given` names a member, or the file is in none.
pub fn member(given: &str, asked: &[At]) -> Option<PathBuf> {
    let first = asked.first()?;
    let here = std::fs::canonicalize(given).ok()?;
    let metadata = Metadata::read(given).ok()?;
    let root = std::fs::canonicalize(&metadata.workspace).ok()?;
    if here != root && metadata.enclosing(&here).is_some() {
        return None;
    }
    let file = candidates(
        &first.path,
        &[crate::locate::here(), here.clone(), root.clone()],
    )
    .into_iter()
    .find(|file| file.starts_with(&root))?;
    metadata.enclosing(&file).filter(|member| *member != here)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(path: &str, from: usize, to: usize) -> At {
        At {
            path: path.to_string(),
            from,
            to,
        }
    }

    #[test]
    fn takes_a_file_and_a_line() {
        assert_eq!(parse("src/lib.rs:17"), Ok(vec![at("src/lib.rs", 17, 17)]));
    }

    #[test]
    fn takes_lines_from_one_to_another() {
        assert_eq!(parse("lib.rs:17-20"), Ok(vec![at("lib.rs", 17, 20)]));
        assert_eq!(parse("lib.rs:17-17"), Ok(vec![at("lib.rs", 17, 17)]));
    }

    #[test]
    fn takes_several_split_by_commas() {
        assert_eq!(
            parse("lib.rs:17, tier.rs:3-5"),
            Ok(vec![at("lib.rs", 17, 17), at("tier.rs", 3, 5)])
        );
    }

    /// The lines are read from the right, so a drive's colon stays in the path.
    #[test]
    fn keeps_a_windows_drive_in_the_path() {
        assert_eq!(
            parse(r"C:\work\pricing\src\lib.rs:17-20"),
            Ok(vec![at(r"C:\work\pricing\src\lib.rs", 17, 20)])
        );
        assert_eq!(
            parse("C:/work/pricing/src/lib.rs:17"),
            Ok(vec![at("C:/work/pricing/src/lib.rs", 17, 17)])
        );
    }

    #[test]
    fn writes_it_back_as_typed() {
        assert_eq!(at("lib.rs", 17, 17).typed("src/lib.rs"), "src/lib.rs:17");
        assert_eq!(at("lib.rs", 17, 20).typed("src/lib.rs"), "src/lib.rs:17-20");
    }

    #[test]
    fn refuses_a_file_without_a_line() {
        for typed in [
            "lib.rs",
            "lib.rs:",
            ":17",
            "lib.rs:x",
            r"C:\a.rs",
            "lib.rs:17-",
        ] {
            let refused = parse(typed).unwrap_err();
            assert!(refused.contains(TAKES), "{typed}: {refused}");
            assert!(
                refused.starts_with(&format!("--at={typed} has no line.")),
                "{refused}"
            );
        }
    }

    #[test]
    fn refuses_a_line_before_the_first() {
        assert_eq!(
            parse("lib.rs:0"),
            Err(
                "--at=lib.rs:0: a file's lines are numbered from 1, as an editor shows them".into()
            )
        );
        assert!(parse("lib.rs:99999999999")
            .unwrap_err()
            .contains("numbered from 1"));
    }

    #[test]
    fn refuses_lines_that_end_before_they_start() {
        assert_eq!(
            parse("lib.rs:20-17"),
            Err("--at=lib.rs:20-17 ends before it starts: --at=lib.rs:17-20".into())
        );
    }

    /// `mutants` prints a place to the column: `--at` takes its line.
    #[test]
    fn refuses_a_column_with_the_line_its_on() {
        assert_eq!(
            parse("src/lib.rs:17:15"),
            Err("--at takes a line, not a column: --at=src/lib.rs:17".into())
        );
    }

    #[test]
    fn refuses_an_empty_one_between_commas() {
        assert!(parse("a.rs:1,,b.rs:2")
            .unwrap_err()
            .starts_with("--at=a.rs:1,,b.rs:2 has an empty one between its commas."));
        assert!(parse("").unwrap_err().starts_with("--at needs a value."));
    }

    #[test]
    fn lists_as_people_do() {
        let words = |items: &[&str]| {
            items
                .iter()
                .map(|item| item.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(listed(&words(&["17"]), "and"), "17");
        assert_eq!(listed(&words(&["17", "18"]), "and"), "17 and 18");
        assert_eq!(listed(&words(&["17", "18", "20"]), "and"), "17, 18 and 20");
    }

    #[test]
    fn names_its_lines_as_the_count_line_does() {
        let lines = |from, to| Lines {
            file: PathBuf::from("src/lib.rs"),
            shown: "src/lib.rs".into(),
            at: at("lib.rs", from, to),
        };
        assert_eq!(lines(17, 17).named(), "line 17 of lib.rs");
        assert_eq!(lines(17, 20).named(), "lines 17–20 of lib.rs");
        assert_eq!(lines(17, 20).typed(), "src/lib.rs:17-20");
    }

    /// The source directories a refusal names: each crate root's, without one inside another.
    #[test]
    fn names_the_packages_source_directories() {
        let roots = [
            PathBuf::from("shop/src/lib.rs"),
            PathBuf::from("shop/src/bin/tool.rs"),
            PathBuf::from("shop/src/main.rs"),
        ];
        assert_eq!(source_dirs(&roots), ["shop/src"]);
    }

    /// A file of its own, as a crate's lib.rs, found with the core operators.
    struct Sample {
        dir: PathBuf,
        found: Crate,
    }

    impl Drop for Sample {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.dir).ok();
        }
    }

    fn sample(name: &str, source: &str) -> Sample {
        let dir = std::env::temp_dir().join(format!("mutastic-at-{name}-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let found = crate::sites::find(
            &dir,
            "sample",
            &[PathBuf::from("src/lib.rs")],
            crate::sites::DEFAULTS,
        );
        Sample { dir, found }
    }

    fn lines_of(sample: &Sample, from: usize, to: usize) -> Vec<Lines> {
        let bases = [std::fs::canonicalize(&sample.dir).unwrap()];
        let workspace = bases[0].clone();
        vec![file(
            &at("src/lib.rs", from, to),
            &bases,
            &workspace,
            &sample.found,
            &["src".to_string()],
        )
        .unwrap()]
    }

    /// The operators of the mutants on `from` to `to`, in the order found.
    fn changes_on(sample: &Sample, from: usize, to: usize) -> Result<Vec<&'static str>, Failure> {
        let ids = on(&lines_of(sample, from, to), &sample.found, |_| true, ".")?;
        Ok(sample
            .found
            .mutants
            .iter()
            .filter(|mutant| ids.contains(&mutant.id))
            .map(|mutant| mutant.operator.trim_start_matches("mutastic/"))
            .collect())
    }

    const SPLIT: &str = "\
pub fn allowed(over: bool, location: u32) -> bool {
    !over
        &&
        location < 9
}

pub fn fee(total: u32) -> u32 {
    if total > 100 {
        return 0;
    }
    5
}
";

    /// A mutant whose claim is written over several lines is on each of them, and one on a single
    /// line is on that line alone: the body's value, `!over && location < 9`, is on lines 2 to 4,
    /// the `&&` on 3, and `location < 9` on 4.
    #[test]
    fn a_line_has_the_mutants_whose_claim_covers_it() {
        let sample = sample("split", SPLIT);
        let on_line = |line| changes_on(&sample, line, line).unwrap();
        assert_eq!(on_line(2), ["return-value", "return-value"]);
        assert_eq!(
            on_line(3),
            ["return-value", "return-value", "logical-connector"]
        );
        assert_eq!(
            on_line(4),
            [
                "return-value",
                "return-value",
                "boundary",
                "remove-conditional",
                "remove-conditional"
            ]
        );
        // A range takes every line's, across functions.
        assert_eq!(
            changes_on(&sample, 4, 8).unwrap(),
            [
                "return-value",
                "return-value",
                "boundary",
                "remove-conditional",
                "remove-conditional",
                "boundary",
                "remove-conditional",
                "remove-conditional"
            ]
        );
    }

    /// A span is the claim's lines, as mutation.json's location has them, from where it starts to
    /// where it ends.
    #[test]
    fn a_span_is_its_claims_lines() {
        let sample = sample("span", SPLIT);
        let spans: Vec<(&str, (usize, usize))> = sample
            .found
            .mutants
            .iter()
            .filter(|mutant| mutant.function == 0)
            .map(|mutant| (mutant.operator, span(&sample.found, mutant)))
            .collect();
        assert_eq!(
            spans,
            [
                ("mutastic/return-value", (2, 4)),
                ("mutastic/return-value", (2, 4)),
                ("mutastic/logical-connector", (3, 3)),
                ("mutastic/boundary", (4, 4)),
                ("mutastic/remove-conditional", (4, 4)),
                ("mutastic/remove-conditional", (4, 4))
            ]
        );
    }

    #[test]
    fn lines_with_none_name_the_nearest_that_have_one() {
        let sample = sample("none", SPLIT);
        let refused = changes_on(&sample, 6, 6).unwrap_err();
        assert_eq!(refused.code, Code::NoMutantAt);
        let first = refused.message.lines().next().unwrap();
        assert!(
            first.starts_with("no mutant on lib.rs:6; there are on "),
            "{first}"
        );
        assert!(first.ends_with(". To see them:"), "{first}");
        assert!(refused
            .message
            .lines()
            .nth(1)
            .unwrap()
            .starts_with("  mutastic mutants --at=src/lib.rs:"));
    }

    /// For the one line with a mutant near, it's said as one.
    #[test]
    fn one_line_near_is_said_as_one() {
        let sample = sample("one", "pub fn five() -> u32 {\n    5\n}\n\n\n");
        let refused = changes_on(&sample, 4, 4).unwrap_err();
        assert_eq!(
            refused.message,
            "no mutant on lib.rs:4; there's one on 2. To see it:\n  \
             mutastic mutants --at=src/lib.rs:2 ."
        );
    }

    #[test]
    fn a_file_outside_the_sources_is_refused_naming_them() {
        let sample = sample("outside", SPLIT);
        std::fs::create_dir_all(sample.dir.join("tests")).unwrap();
        std::fs::write(sample.dir.join("tests/fees.rs"), "\n").unwrap();
        let bases = [std::fs::canonicalize(&sample.dir).unwrap()];
        let refused = file(
            &at("tests/fees.rs", 1, 1),
            &bases,
            &bases[0],
            &sample.found,
            &["src".to_string()],
        )
        .unwrap_err();
        assert_eq!(refused.code, Code::OutsideSources);
        assert_eq!(
            refused.message,
            "tests/fees.rs isn't in the package's sources: --at takes a line of the code \
             mutastic mutates, in src"
        );
        let missing = file(
            &at("src/nowhere.rs", 1, 1),
            &bases,
            &bases[0],
            &sample.found,
            &["src".to_string()],
        )
        .unwrap_err();
        assert_eq!(missing.code, Code::OptionValue);
    }

    /// A Windows path's `\` separates its parts on every platform.
    #[test]
    fn either_separator_splits_the_path() {
        let sample = sample("separator", SPLIT);
        let bases = [std::fs::canonicalize(&sample.dir).unwrap()];
        let found = file(
            &at(r"src\lib.rs", 2, 2),
            &bases,
            &bases[0],
            &sample.found,
            &["src".to_string()],
        )
        .unwrap();
        assert_eq!(found.shown, "src/lib.rs");
    }
}
