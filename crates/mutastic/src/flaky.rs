//! Flaky tests: a test that failed in the coverage pass and passed when run again on its own, and
//! what it reaches that answers differently from one run to the next. As in the Java edition, the
//! run goes on without it, and says so, since which of its own code or the code it runs varies
//! changes what to do about it.

use std::collections::BTreeSet;
use std::path::Path;

use ra_ap_syntax::ast::{self, HasAttrs, HasName};
use ra_ap_syntax::{AstNode, Edition, SourceFile, SyntaxKind, SyntaxNode};

use crate::sites::Crate;

/// How many times a test that failed the coverage pass runs again on its own before it counts as
/// failing, as in the Java edition: one rerun let a test that fails about half the time stop a run.
pub const RERUNS: usize = 3;

/// A test left out of the run because it failed, then passed when run again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flaky {
    /// The test, as the run shows it: `raffle::a_draw_is_a_ticket`.
    pub test: String,
    /// What the code it ran reaches that answers differently every run: `SystemTime::now`.
    pub reaches: Vec<String>,
    /// The same, in the test's own code: its module, or its file under `tests/`.
    pub reaches_in_test: Vec<String>,
}

/// Calls whose answer changes from one run to the next, by what they're written as, and the name
/// the report gives each. A seeded generator (`StdRng::seed_from_u64`) answers the same every
/// run, so it isn't one.
const VARYING_CALLS: &[(&str, &str)] = &[
    ("Instant::now", "Instant::now"),
    ("SystemTime::now", "SystemTime::now"),
    ("RandomState::new", "RandomState::new"),
    ("RandomState::default", "RandomState::default"),
    ("thread_rng", "rand::thread_rng"),
    ("rand::random", "rand::random"),
    ("rand::rng", "rand::rng"),
    ("from_entropy", "SeedableRng::from_entropy"),
    ("from_os_rng", "SeedableRng::from_os_rng"),
    ("OsRng", "OsRng"),
    ("Uuid::new_v4", "Uuid::new_v4"),
    ("Utc::now", "Utc::now"),
    ("Local::now", "Local::now"),
];

/// Types whose every value answers differently: a field of one, read through `self`, makes the
/// method that reads it vary. `RandomState` is keyed afresh for each value.
const VARYING_TYPES: &[&str] = &["RandomState", "ThreadRng", "OsRng"];

/// What the functions `reached` (indexes into `found.functions`) reach that varies.
pub fn reaches(found: &Crate, reached: &BTreeSet<usize>) -> Vec<String> {
    let mut names = BTreeSet::new();
    for &index in reached {
        let function = &found.functions[index];
        let Some(file) = found.files.iter().find(|file| file.path == function.file) else {
            continue;
        };
        let Some(text) = file.text.get(function.range.clone()) else {
            continue;
        };
        let code = parse(text);
        names.extend(varying_calls(&code));
        names.extend(varying_fields(found, &function.path, &code));
    }
    names.into_iter().collect()
}

/// What the test `name` (libtest's, `tests::draws`) reaches in its own code: the `mod` it's in, or
/// its whole file under `tests/`, as the Java edition reads the test's class. `sources` are the
/// files to look in, with their text.
pub fn reaches_in_test(sources: &[(bool, String)], name: &str) -> Vec<String> {
    let last = name.rsplit("::").next().unwrap_or(name);
    for (whole_file, text) in sources {
        let file = SourceFile::parse(text, Edition::CURRENT).tree();
        let test = file
            .syntax()
            .descendants()
            .filter_map(ast::Fn::cast)
            .find(|function| {
                function.name().is_some_and(|n| n.text() == last)
                    && function
                        .attrs()
                        .any(|attr| attr.syntax().text().to_string().contains("test"))
            });
        let Some(test) = test else {
            continue;
        };
        let scope = test
            .syntax()
            .ancestors()
            .find_map(ast::Module::cast)
            .map(|module| module.syntax().clone());
        let scope = match scope {
            Some(module) => module,
            None if *whole_file => file.syntax().clone(),
            None => test.syntax().clone(),
        };
        return varying_calls(&scope).into_iter().collect();
    }
    Vec::new()
}

/// The sources a test may be in: for an integration test, `tests/<target>.rs` or
/// `tests/<target>/main.rs` under the package (each a whole file of tests); for the others, the
/// crate's own files.
pub fn test_sources(
    found: &Crate,
    package: &Path,
    kind: &str,
    target: &str,
) -> Vec<(bool, String)> {
    if kind == "test" {
        return [
            package.join("tests").join(format!("{target}.rs")),
            package.join("tests").join(target).join("main.rs"),
        ]
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .map(|text| (true, text))
        .collect();
    }
    found
        .files
        .iter()
        .map(|file| (false, file.text.clone()))
        .collect()
}

fn parse(text: &str) -> SyntaxNode {
    SourceFile::parse(text, Edition::CURRENT)
        .tree()
        .syntax()
        .clone()
}

/// The code's tokens, without comments or strings, run together: `SystemTime::now()`.
fn code_text(node: &SyntaxNode) -> String {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| {
            !token.kind().is_trivia()
                && !matches!(
                    token.kind(),
                    SyntaxKind::STRING | SyntaxKind::BYTE_STRING | SyntaxKind::C_STRING
                )
        })
        .map(|token| token.text().to_string())
        .collect()
}

fn varying_calls(node: &SyntaxNode) -> BTreeSet<String> {
    let code = code_text(node);
    VARYING_CALLS
        .iter()
        .filter(|(written, _)| contains_word(&code, written))
        .map(|(_, name)| name.to_string())
        .collect()
}

/// Whether `code` has `word`, not as part of a longer name.
fn contains_word(code: &str, word: &str) -> bool {
    let is_name = |c: char| c.is_alphanumeric() || c == '_';
    code.match_indices(word).any(|(at, _)| {
        let before = code[..at].chars().next_back();
        let after = code[at + word.len()..].chars().next();
        !before.is_some_and(is_name) && !after.is_some_and(is_name)
    })
}

/// The varying types of the fields a method reads through `self`, by the struct its `impl` is for
/// (the second-last part of its path, `Lottery` of `lottery::Lottery::below`).
fn varying_fields(found: &Crate, path: &str, code: &SyntaxNode) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Some(owner) = path.rsplit("::").nth(1) else {
        return names;
    };
    let read: BTreeSet<String> = code
        .descendants()
        .filter_map(ast::FieldExpr::cast)
        .filter(|field| {
            field
                .expr()
                .is_some_and(|receiver| receiver.syntax().text() == "self")
        })
        .filter_map(|field| field.name_ref().map(|name| name.text().to_string()))
        .collect();
    if read.is_empty() {
        return names;
    }
    for file in &found.files {
        let tree = SourceFile::parse(&file.text, Edition::CURRENT).tree();
        for item in tree.syntax().descendants().filter_map(ast::Struct::cast) {
            if item.name().is_none_or(|name| name.text() != owner) {
                continue;
            }
            let Some(ast::FieldList::RecordFieldList(fields)) = item.field_list() else {
                continue;
            };
            for field in fields.fields() {
                let named = field
                    .name()
                    .is_some_and(|n| read.contains(n.text().as_str()));
                let ty = field
                    .ty()
                    .map(|ty| code_text(ty.syntax()))
                    .unwrap_or_default();
                let last = ty
                    .rsplit("::")
                    .next()
                    .unwrap_or_default()
                    .trim_end_matches('>');
                if named && VARYING_TYPES.contains(&last) {
                    names.insert(last.to_string());
                }
            }
        }
    }
    names
}

/// `the tests fail without any mutant: a::b (it timed out), c; their output is in <log>`: the
/// Java edition's words for tests that fail each time they run.
pub fn failed_message(failing: &[(String, bool)], log: &Path) -> String {
    let named: Vec<String> = failing
        .iter()
        .map(|(test, timed_out)| {
            if *timed_out {
                format!("{test} (it timed out)")
            } else {
                test.clone()
            }
        })
        .collect();
    format!(
        "{FAILED}{}; their output is in {}",
        named.join(", "),
        log.display()
    )
}

const FAILED: &str = "the tests fail without any mutant: ";

/// The tests [`failed_message`] names, if `error` is one.
pub fn failing_tests(error: &str) -> Option<Vec<String>> {
    let rest = error.strip_prefix(FAILED)?;
    let (list, _) = rest.split_once("; their output is in ")?;
    Some(
        list.split(", ")
            .map(|test| test.trim_end_matches(" (it timed out)").to_string())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_failing_tests_are_read_back_from_their_message() {
        let message = failed_message(
            &[("a::b".into(), true), ("c".into(), false)],
            Path::new("target/mutastic/tests.log"),
        );
        assert_eq!(
            message,
            "the tests fail without any mutant: a::b (it timed out), c; their output is in target/mutastic/tests.log"
        );
        assert_eq!(
            failing_tests(&message),
            Some(vec!["a::b".into(), "c".into()])
        );
        assert_eq!(failing_tests("couldn't build"), None);
    }

    #[test]
    fn what_varies_is_found_in_code_not_comments_or_strings() {
        let code = parse(
            "fn f() { let t = std::time::SystemTime::now(); // Instant::now\n let s = \"OsRng\"; let r = rand::thread_rng(); let n = MyInstant::now(); }",
        );
        assert_eq!(
            varying_calls(&code).into_iter().collect::<Vec<_>>(),
            ["SystemTime::now", "rand::thread_rng"]
        );
    }

    #[test]
    fn a_field_whose_type_varies_is_reached_through_self() {
        let dir = std::env::temp_dir().join(format!("mutastic-flaky-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let source = "use std::hash::{BuildHasher, RandomState};\n\
            #[derive(Default)]\n\
            pub struct Lottery { random: RandomState, fixed: u64 }\n\
            impl Lottery {\n\
                pub fn below(&self, n: u64) -> u64 { self.random.hash_one(n) % (n + 1) }\n\
                pub fn steady(&self, n: u64) -> u64 { self.fixed % (n + 1) }\n\
            }\n";
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let found = crate::sites::find(
            &dir,
            "lottery",
            &[std::path::PathBuf::from("src/lib.rs")],
            crate::sites::DEFAULTS,
        );
        std::fs::remove_dir_all(&dir).ok();
        let index = |name: &str| found.functions.iter().position(|f| f.name == name).unwrap();
        assert_eq!(
            reaches(&found, &BTreeSet::from([index("below")])),
            ["RandomState"]
        );
        assert!(reaches(&found, &BTreeSet::from([index("steady")])).is_empty());
    }

    #[test]
    fn a_test_s_own_module_is_read() {
        let source = "pub fn f() {}\n#[cfg(test)]\nmod tests {\n    fn clock() -> u128 { std::time::Instant::now().elapsed().as_nanos() }\n    #[test]\n    fn draws() { clock(); }\n}\n";
        assert_eq!(
            reaches_in_test(&[(false, source.into())], "tests::draws"),
            ["Instant::now"]
        );
        assert!(reaches_in_test(&[(false, source.into())], "tests::other").is_empty());
    }
}
