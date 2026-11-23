//! The row a test is, when it's one case of a table: rstest's `#[case]` and test-case's
//! `#[test_case]` each write a test of their own, named for the row's place,
//! `rstest_tests::subtracts::case_1` and `rows::multiplies::test_case_2_two_by_one`. That name is
//! what mutastic selects and reports, as libtest lists it; the row as written,
//! `#[case(30, 0, 30)]`, is what tells a reader which case it is, so a card names it after the test
//! as evidence of this run. The place moves when a row is added above it, so nothing keys on it.

use ra_ap_syntax::ast::{self, HasAttrs, HasName};
use ra_ap_syntax::{AstNode, Edition, SourceFile};

use crate::workspace::Project;

/// One case of a table, as the events' `caughtCase` gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The `fn` the case is a row of, `rstest_tests::subtracts`: the test's name without its last
    /// segment.
    pub test: String,
    /// Its place among the `fn`'s rows, from 1: the number in its name.
    pub index: usize,
    /// The row as written, on one line: `#[case::at_the_edge(100, 10)]`.
    pub name: String,
}

impl Case {
    /// ` (case #[case(30, 0, 30)])`, as a line says it after the test, its text shown as text.
    pub fn said(&self) -> String {
        format!(" (case {})", crate::untrusted::value(&self.name))
    }
}

/// The most of a row its name keeps: a row can hold a long expression.
const NAME: usize = 200;

/// The attribute a generated test's name says it's a row of, and its place: `case_2` and
/// `case_02_at_the_edge` (rstest pads the number when there are ten rows or more) are rstest's
/// `case`, `test_case_2_two_by_one` test-case's `test_case`.
fn row(name: &str) -> Option<(&'static str, usize)> {
    let (attribute, rest) = match name.strip_prefix("test_case_") {
        Some(rest) => ("test_case", rest),
        None => ("case", name.strip_prefix("case_")?),
    };
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let after = &rest[digits..];
    if digits == 0 || !(after.is_empty() || after.starts_with('_')) {
        return None;
    }
    Some((attribute, rest[..digits].parse().ok()?))
}

/// The case the test a run shows as `display` is, when it's a row of an rstest or test-case table
/// the source still has; `None` for any other test.
pub fn of(project: &Project, display: &str) -> Option<Case> {
    let (test, last) = display.rsplit_once("::")?;
    let (attribute, index) = row(last)?;
    let place = crate::test_files::place(project, test)?;
    let (line, column) = place.at?;
    let text = std::fs::read_to_string(project.workspace.join(&place.file)).ok()?;
    let starts = crate::sites::line_starts(&text);
    let function = SourceFile::parse(&text, Edition::CURRENT)
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| {
            function.name().is_some_and(|name| {
                let offset = usize::from(name.syntax().text_range().start());
                let at = crate::sites::line_of(&starts, offset);
                at == line && text[starts[at - 1]..offset].chars().count() + 1 == column
            })
        })?;
    let written = function
        .attrs()
        .filter(|attr| is_row(attr, attribute))
        .nth(index.checked_sub(1)?)?;
    Some(Case {
        test: test.to_string(),
        index,
        name: one_line(&written.syntax().text().to_string()),
    })
}

/// Whether `attr` is a row: rstest's `#[case(…)]` or `#[case::name(…)]`, or test-case's
/// `#[test_case(…)]`, as `attribute` says which.
fn is_row(attr: &ast::Attr, attribute: &str) -> bool {
    let Some(path) = attr.path() else {
        return false;
    };
    let path = path.syntax().text().to_string();
    let path = path.trim();
    if attribute == "case" {
        path == "case" || path.starts_with("case::")
    } else {
        path == "test_case" || path == "test_case::test_case"
    }
}

/// Its whitespace made single spaces, cut to what a name keeps.
fn one_line(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match line.char_indices().nth(NAME) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line,
    }
}

/// The case of the test that caught a mutant, when it's a row of a table.
pub fn caught(project: &Project, result: &crate::engine::MutantResult) -> Option<Case> {
    of(project, result.killed_by.as_deref()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn project(files: &[(&str, &str)]) -> Project {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let workspace =
            std::env::temp_dir().join(format!("mutastic-cases-{}-{n}", std::process::id()));
        for (path, text) in files {
            let file = workspace.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        }
        let target = |name: &str, lib: bool, root: &str| crate::workspace::Target {
            name: name.into(),
            lib,
            root: root.into(),
        };
        Project {
            given: ".".into(),
            workspace,
            target: PathBuf::new(),
            package: "shop".into(),
            package_dir: PathBuf::new(),
            roots: vec!["src/lib.rs".into()],
            targets: vec![
                target("shop", true, "src/lib.rs"),
                target("rows", false, "tests/rows.rs"),
            ],
            crate_name: "shop".into(),
            edition: "2021".into(),
            features: Default::default(),
            members: Default::default(),
        }
    }

    #[test]
    fn a_row_is_found_by_its_place_among_its_fns_rows() {
        let project = project(&[
            (
                "src/lib.rs",
                "#[cfg(test)]\nmod tests {\n    #[rstest]\n    #[case(1, 1)]\n    #[case::at_the_edge(\n        100,\n        10\n    )]\n    fn discounts(#[case] a: i64, #[case] b: i64) {}\n\n    #[test]\n    fn case_1() {}\n}\n",
            ),
            (
                "tests/rows.rs",
                "#[test_case(1, 1 ; \"one by one\")]\n#[allow(unused)]\n#[test_case(2, 1 ; \"two by one\")]\nfn multiplies(a: i64, b: i64) {}\n",
            ),
        ]);
        let case = |test: &str, index, name: &str| {
            Some(Case {
                test: test.into(),
                index,
                name: name.into(),
            })
        };
        assert_eq!(
            of(&project, "tests::discounts::case_1"),
            case("tests::discounts", 1, "#[case(1, 1)]")
        );
        assert_eq!(
            of(&project, "tests::discounts::case_02_at_the_edge"),
            case("tests::discounts", 2, "#[case::at_the_edge( 100, 10 )]")
        );
        assert_eq!(
            of(&project, "rows::multiplies::test_case_2_two_by_one"),
            case("rows::multiplies", 2, "#[test_case(2, 1 ; \"two by one\")]")
        );
        // No such row, a test that's no row, and a fn named like one that has none.
        assert_eq!(of(&project, "tests::discounts::case_3"), None);
        assert_eq!(of(&project, "tests::discounts"), None);
        assert_eq!(of(&project, "tests::case_1"), None);
        assert_eq!(
            of(&project, "tests::discounts::case_1::a_1_2"),
            None,
            "a cell of a matrix is no row"
        );
        std::fs::remove_dir_all(&project.workspace).ok();
    }

    #[test]
    fn a_long_row_is_cut() {
        let long = format!("#[case({})]", "1, ".repeat(100));
        let name = one_line(&long);
        assert_eq!(name.chars().count(), NAME + 1);
        assert!(name.ends_with('…'));
    }
}
