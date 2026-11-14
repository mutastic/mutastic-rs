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
/// files to look in, with their text. A generated test, `adds::case_1`, is written as the function
/// an earlier segment names.
pub fn reaches_in_test(sources: &[(bool, String)], name: &str) -> Vec<String> {
    let files: Vec<(bool, SyntaxNode)> = sources
        .iter()
        .map(|(whole_file, text)| (*whole_file, parse(text)))
        .collect();
    for segment in name.rsplit("::") {
        for (whole_file, file) in &files {
            let Some(test) = written_in(file, segment) else {
                continue;
            };
            let scope = test
                .ancestors()
                .find_map(ast::Module::cast)
                .map(|module| module.syntax().clone());
            let scope = match scope {
                Some(module) => module,
                None if *whole_file => file.clone(),
                None => test,
            };
            return varying_calls(&scope).into_iter().collect();
        }
    }
    Vec::new()
}

/// Where the test function `name` is written in `file`: a function with a test's attribute, or
/// else a macro whose input defines it, as `proptest! { #[test] fn name(..) {..} }` does.
fn written_in(file: &SyntaxNode, name: &str) -> Option<SyntaxNode> {
    let function = file
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| {
            function.name().is_some_and(|n| n.text() == name)
                && function
                    .attrs()
                    .any(|attr| attr.syntax().text().to_string().contains("test"))
        });
    if let Some(function) = function {
        return Some(function.syntax().clone());
    }
    file.descendants()
        .filter_map(ast::MacroCall::cast)
        .find(|call| {
            call.token_tree()
                .is_some_and(|tree| defines(&tree.syntax().text().to_string(), name))
        })
        .map(|call| call.syntax().clone())
}

/// Whether a macro's input has `fn name`.
fn defines(input: &str, name: &str) -> bool {
    let words: Vec<&str> = input
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|word| !word.is_empty())
        .collect();
    words.windows(2).any(|pair| pair == ["fn", name])
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

/// A test that fails each time it runs with no mutant in: as it's shown, how it failed, and the
/// command that runs it alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Red {
    pub display: String,
    pub timed_out: bool,
    /// The panic's message, on one line: `assertion `left == right` failed: left: 3, right: 4`.
    pub panic: Option<String>,
    pub rerun: String,
}

/// How many failing tests are named with how they failed, and given a command.
const RED_SHOWN: usize = 3;

/// `tests::t fails without any mutant, so mutastic can't judge one.`, how each failed, why a test
/// that passes under `cargo test` may fail here, where its output is, and the command that runs it
/// alone, as the Java edition says it. [`failing_tests`] reads the names back.
pub fn failed_message(failing: &[Red], log: &Path) -> String {
    let named: Vec<String> = failing
        .iter()
        .map(|red| {
            if red.timed_out {
                format!("{} (it timed out)", red.display)
            } else {
                red.display.clone()
            }
        })
        .collect();
    let one = failing.len() == 1;
    let how: Vec<String> = failing
        .iter()
        .take(RED_SHOWN)
        .filter_map(|red| {
            let panic = red.panic.as_ref()?;
            Some(if one {
                format!("It panicked: {panic}")
            } else {
                format!("{} panicked: {panic}", red.display)
            })
        })
        .collect();
    let commands: Vec<String> = failing
        .iter()
        .take(RED_SHOWN)
        .map(|red| format!("  {}", red.rerun))
        .collect();
    let (its, it) = if one {
        ("Its", "it")
    } else {
        ("Their", "each")
    };
    let mut lines = vec![format!(
        "{} {} without any mutant, so mutastic can't judge one.",
        named.join(", "),
        if one { "fails" } else { "fail" }
    )];
    lines.extend(how);
    lines.push(
        "mutastic runs each test alone: a test that passes with the others, but fails alone, \
         leans on one of them."
            .into(),
    );
    lines.push(format!(
        "{its} output is in {}. To run {it} alone:",
        log.display()
    ));
    lines.extend(commands);
    lines.join("\n")
}

const FAILS: &str = " without any mutant, so mutastic can't judge one.";

/// The tests [`failed_message`] names, if `error` is one, and the same in the Java edition's one
/// line, `the tests fail without any mutant: a::b (it timed out), c; their output is in <log>`,
/// as the gate quotes it.
pub fn failing_tests(error: &str) -> Option<(Vec<String>, String)> {
    let headline = error.lines().next()?;
    let list = headline
        .strip_suffix(FAILS)?
        .strip_suffix(" fails")
        .or_else(|| headline.strip_suffix(FAILS)?.strip_suffix(" fail"))?;
    let log = error
        .lines()
        .find_map(|line| line.split_once(" output is in ")?.1.split_once(". To run "))
        .map(|(log, _)| log)?;
    let names = list
        .split(", ")
        .map(|test| test.trim_end_matches(" (it timed out)").to_string())
        .collect();
    Some((
        names,
        format!("the tests fail without any mutant: {list}; their output is in {log}"),
    ))
}

/// The panic in a test's output, on one line: `thread 't' panicked at src/lib.rs:3:5:` is followed
/// by its message, whose lines are joined, up to the note that follows. Where it panicked isn't
/// kept: it's a line of mutastic's copy, where a mutant's code can move it.
pub fn panic_message(output: &str) -> Option<String> {
    let mut lines = output.lines();
    // `thread 't' panicked at`, or in newer Rust with the thread's id, `thread 't' (12345) panicked at`.
    let first = lines
        .by_ref()
        .find(|line| line.starts_with("thread '") && line.contains(" panicked at "))?;
    // Before Rust 1.73: `thread 't' panicked at 'message', src/lib.rs:3:5`.
    if let Some(rest) = first.split_once(" panicked at '").map(|(_, rest)| rest) {
        return rest
            .rsplit_once("', ")
            .map(|(message, _)| message.to_string());
    }
    let message: Vec<&str> = lines
        .take_while(|line| !line.starts_with("note:") && !line.trim().is_empty())
        .map(str::trim)
        .collect();
    let (head, rest) = message.split_first()?;
    let said = if rest.is_empty() {
        head.to_string()
    } else {
        format!("{head}: {}", rest.join(", "))
    };
    const MOST: usize = 300;
    Some(if said.chars().count() > MOST {
        format!("{}…", said.chars().take(MOST).collect::<String>())
    } else {
        said
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_failing_tests_are_read_back_from_their_message() {
        let red = |display: &str, timed_out: bool, panic: Option<&str>| Red {
            display: display.into(),
            timed_out,
            panic: panic.map(str::to_string),
            rerun: format!("cargo test -p red --lib -- --exact {display}"),
        };
        let log = Path::new("target/mutastic/tests.log");
        let message = failed_message(
            &[red("a::b", true, None), red("c", false, Some("boom"))],
            log,
        );
        assert_eq!(
            message,
            "a::b (it timed out), c fail without any mutant, so mutastic can't judge one.\n\
             c panicked: boom\n\
             mutastic runs each test alone: a test that passes with the others, but fails alone, \
             leans on one of them.\n\
             Their output is in target/mutastic/tests.log. To run each alone:\n  \
             cargo test -p red --lib -- --exact a::b\n  \
             cargo test -p red --lib -- --exact c"
        );
        assert_eq!(
            failing_tests(&message),
            Some((
                vec!["a::b".into(), "c".into()],
                "the tests fail without any mutant: a::b (it timed out), c; their output is in \
                 target/mutastic/tests.log"
                    .into()
            ))
        );
        let one = failed_message(&[red("t", false, Some("boom"))], log);
        assert!(
            one.starts_with(
                "t fails without any mutant, so mutastic can't judge one.\nIt panicked: boom\n"
            ),
            "{one}"
        );
        assert_eq!(failing_tests(&one).unwrap().0, ["t"]);
        assert_eq!(failing_tests("couldn't build"), None);
    }

    #[test]
    fn a_panic_is_read_on_one_line() {
        let output = "running 1 test\n\nthread 'tests::t' panicked at src/lib.rs:20:9:\n\
                      assertion `left == right` failed\n  left: 3\n right: 4\n\
                      note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\n";
        assert_eq!(
            panic_message(output).as_deref(),
            Some("assertion `left == right` failed: left: 3, right: 4")
        );
        let numbered =
            "thread 'sums_up' (48095868) panicked at tests/sums.rs:3:5:\ntwo and two make 4\n";
        assert_eq!(
            panic_message(numbered).as_deref(),
            Some("two and two make 4")
        );
        let old = "thread 't' panicked at 'fails every time', tests/raffle.rs:9:5\n";
        assert_eq!(panic_message(old).as_deref(), Some("fails every time"));
        assert_eq!(panic_message("test t ... ok\n"), None);
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

    /// A generated case is read as the function it's written as, and a test a macro defines as
    /// the macro's module.
    #[test]
    fn a_case_s_function_and_a_macro_s_test_are_read() {
        let source = "#[cfg(test)]\nmod cases {\n    fn clock() -> u128 { std::time::Instant::now().elapsed().as_nanos() }\n    #[rstest]\n    #[case(1)]\n    fn draws(#[case] n: u32) { clock(); }\n}\n\
                      #[cfg(test)]\nmod props {\n    proptest! {\n        #[test]\n        fn picks(n in 0..9) { let _ = std::time::SystemTime::now(); }\n    }\n}\n";
        let sources = [(false, source.to_string())];
        assert_eq!(
            reaches_in_test(&sources, "cases::draws::case_1"),
            ["Instant::now"]
        );
        assert_eq!(
            reaches_in_test(&sources, "props::picks"),
            ["SystemTime::now"]
        );
    }
}
