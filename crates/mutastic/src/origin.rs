//! Where a catching test's failure came from, as `failureOrigin` says it in the events and
//! `mutation.json`: `assertion` when a check failed, `test` when the test's own code panicked,
//! `code` when the code under test did, and `unknown` when the panic can't be found or is outside
//! the package. The Java edition reads the exception's stack; here the panic's line, which libtest
//! prints under the test's name, says where.

use std::path::Path;

use ra_ap_syntax::{AstNode, Edition, SourceFile, TextSize};

/// The failure's origin, for the test libtest names `test`, from what its run printed. A source
/// path in a panic is looked for under each of `roots` in turn.
pub fn of(output: &str, test: &str, roots: &[&Path]) -> &'static str {
    let Some((file, line, message)) = panic(output, test) else {
        return "unknown";
    };
    if message.starts_with("assertion") {
        return "assertion";
    }
    let Some(source) = roots
        .iter()
        .find_map(|root| std::fs::read_to_string(root.join(file)).ok())
    else {
        return "unknown";
    };
    let Some(start) = line_start(&source, line) else {
        return "unknown";
    };
    if is_assertion(&source[start..]) {
        "assertion"
    } else if is_test_file(file) || is_test_code(&source, start) {
        "test"
    } else {
        "code"
    }
}

/// The file, line and first line of the message of `test`'s panic: libtest prints `thread
/// 'tests::big' panicked at src/lib.rs:50:9:`, or with the thread's id, `thread 'tests::big' (8123)
/// panicked at …`, and the message on the next line.
fn panic<'a>(output: &'a str, test: &str) -> Option<(&'a str, usize, &'a str)> {
    let opening = format!("thread '{test}' ");
    let mut lines = output.lines();
    let at = lines.find_map(|line| {
        let rest = line.strip_prefix(opening.as_str())?;
        let rest = match rest.strip_prefix('(') {
            Some(thread) => thread.split_once(") ")?.1,
            None => rest,
        };
        rest.strip_prefix("panicked at ")
    })?;
    let at = at.strip_suffix(':').unwrap_or(at);
    let mut parts = at.rsplitn(3, ':');
    let _column = parts.next()?;
    let line = parts.next()?.parse().ok()?;
    let file = parts.next()?;
    Some((file, line, lines.next().unwrap_or_default().trim()))
}

/// Where line `line` (from 1) starts in `source`.
fn line_start(source: &str, line: usize) -> Option<usize> {
    if line == 1 {
        return Some(0);
    }
    source
        .match_indices('\n')
        .nth(line.checked_sub(2)?)
        .map(|(at, _)| at + 1)
}

/// Whether the code from here is an assertion macro: `assert!`, `assert_eq!`, `debug_assert_ne!`,
/// `prop_assert!` or another whose name has `assert` in it.
fn is_assertion(code: &str) -> bool {
    let code = code.trim_start();
    let name: String = code
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':')
        .collect();
    code[name.len()..].starts_with('!')
        && name
            .rsplit("::")
            .next()
            .is_some_and(|last| last.contains("assert"))
}

/// A file of a test target, a bench, or a module they share: under `tests/` or `benches/`.
fn is_test_file(file: &str) -> bool {
    Path::new(file)
        .components()
        .any(|part| matches!(part.as_os_str().to_str(), Some("tests" | "benches")))
}

/// Whether the code at `at` is test code: in a `#[test]` function or a `#[cfg(test)]` module.
fn is_test_code(source: &str, at: usize) -> bool {
    let parsed = SourceFile::parse(source, Edition::CURRENT);
    let syntax = parsed.tree().syntax().clone();
    let Ok(offset) = u32::try_from(at) else {
        return false;
    };
    let indent = source[at..].len() - source[at..].trim_start().len();
    let Ok(indent) = u32::try_from(indent) else {
        return false;
    };
    syntax
        .token_at_offset(TextSize::from(offset + indent))
        .right_biased()
        .and_then(|token| token.parent())
        .is_some_and(|node| crate::sites::is_test_only(&node))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mutastic-origin-{name}-{}", std::process::id()));
        for (path, text) in files {
            let path = dir.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        dir
    }

    fn panicked(test: &str, at: &str, message: &str) -> String {
        format!(
            "running 1 test\ntest {test} ... FAILED\n\nfailures:\n\n---- {test} stdout ----\n\n\
             thread '{test}' panicked at {at}:\n{message}\nnote: run with `RUST_BACKTRACE=1`\n"
        )
    }

    #[test]
    fn a_failed_check_is_an_assertion_and_a_panic_says_whose_code() {
        let lib = "pub fn half(n: u32) -> u32 {\n    n.checked_div(2).unwrap()\n}\n\
                   #[cfg(test)]\nmod tests {\n    #[test]\n    fn halves() {\n        \
                   assert!(super::half(4) == 2, \"half of four\");\n        \
                   let v: Option<u32> = None;\n        v.unwrap();\n    }\n}\n";
        let dir = written(
            "lib",
            &[
                ("src/lib.rs", lib),
                ("tests/it.rs", "fn f() {\n    panic!(\"no\");\n}\n"),
            ],
        );
        let roots = [dir.as_path()];
        let origin = |at: &str, message: &str| {
            of(
                &panicked("tests::halves", at, message),
                "tests::halves",
                &roots,
            )
        };
        assert_eq!(
            origin("src/lib.rs:8:9", "assertion `left == right` failed"),
            "assertion"
        );
        assert_eq!(origin("src/lib.rs:8:9", "half of four"), "assertion");
        assert_eq!(
            origin(
                "src/lib.rs:10:11",
                "called `Option::unwrap()` on a `None` value"
            ),
            "test"
        );
        assert_eq!(
            origin("src/lib.rs:2:22", "attempt to divide by zero"),
            "code"
        );
        assert_eq!(origin("tests/it.rs:2:5", "no"), "test");
        let numbered = panicked(
            "tests::halves",
            "src/lib.rs:2:22",
            "attempt to divide by zero",
        )
        .replace(
            "thread 'tests::halves' panicked",
            "thread 'tests::halves' (8123) panicked",
        );
        assert_eq!(of(&numbered, "tests::halves", &roots), "code");
        assert_eq!(
            origin("/rustc/abc/library/core/src/option.rs:2:5", "x"),
            "unknown"
        );
        assert_eq!(
            of("test tests::halves ... FAILED\n", "tests::halves", &roots),
            "unknown"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
