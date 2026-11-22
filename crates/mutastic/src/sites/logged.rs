//! A value worked out only for a log line or a print is arid, as in the Java edition:
//!
//! ```text
//! let took = start.elapsed().as_millis() - base; // the `-` only feeds a log line
//! info!("took {took} ms");
//! let d = a - b;                                  // and here
//! println!("{}", d);
//! ```
//!
//! Nothing inside a macro's tokens is mutated, so `info!("{}", a - b)` has no mutants already. What
//! is mutated is a `let` whose value is then only printed. A mutant in the `let`'s value is skipped
//! when, by syntax within the function:
//!
//! - the change reaches the value only through arithmetic, a comparison, a cast, a `-` or `!`, or a
//!   method that only works out a value (`abs`, `min`, `as_millis`, `to_string`), and not through
//!   an `if`, a `match`, a `&&` or any other call, which may do something with it;
//! - the binding is a plain name, not `mut`, and no other binding in the function has that name: a
//!   shadow, a parameter or a closure's, read by text in a macro, could be either;
//! - every read of it is in an output macro, [`super::operators::OUTPUT_MACROS`]: as one of its
//!   arguments, `{took}` in its format string, a tracing field (`?took`, `took = took`), or with a
//!   method of the list called on it, and not inside a closure, which may run any number of times,
//!   or inside another macro in the arguments. Any other read keeps the mutant: a comparison, a
//!   `return`, a call, `format!`, `write!`, `assert!`.

use super::operators::OUTPUT_MACROS;
use super::*;
use crate::arid::ONLY_FEEDS_A_LOG_LINE;

use ra_ap_syntax::{NodeOrToken, SyntaxToken};

/// The operators that change a value worked out, or given, by the code.
const VALUES: &[&str] = &[
    STRING_CONSTANT,
    CONSTANT_VALUE,
    ARITHMETIC_REPLACEMENT,
    OTHER_ARITHMETIC,
    KEEP_OPERAND,
    REMOVE_NEGATION,
    BITWISE,
    MATH,
];

/// Methods that only work out a value from the one they're called on and their arguments.
const VALUE_METHODS: &[&str] = &[
    "abs",
    "pow",
    "min",
    "max",
    "clamp",
    "signum",
    "saturating_add",
    "saturating_sub",
    "saturating_mul",
    "wrapping_add",
    "wrapping_sub",
    "wrapping_mul",
    "as_millis",
    "as_secs",
    "as_secs_f32",
    "as_secs_f64",
    "as_micros",
    "as_nanos",
    "subsec_millis",
    "subsec_micros",
    "subsec_nanos",
    "to_string",
    "round",
    "floor",
    "ceil",
    "sqrt",
];

/// `Duration`'s constructors: a value from a number.
const DURATIONS: &[&str] = &[
    "from_secs",
    "from_millis",
    "from_micros",
    "from_nanos",
    "from_secs_f32",
    "from_secs_f64",
];

/// Why the mutant `operator` makes at `node` shouldn't run, if it only changes a value a `let`
/// gives a name that only output macros read.
pub(super) fn reason(operator: &str, node: &SyntaxNode) -> Option<String> {
    if !VALUES.contains(&operator) {
        return None;
    }
    let (name, statement) = bound_by(node)?;
    let scope = statement.syntax().ancestors().find(|ancestor| {
        ast::Fn::can_cast(ancestor.kind()) || ast::ClosureExpr::can_cast(ancestor.kind())
    })?;
    only_printed(&name, &scope).then(|| ONLY_FEEDS_A_LOG_LINE.to_string())
}

/// The name the `let` whose value `node` is part of gives it, and the `let`, if `node` reaches the
/// value through nothing but what works out a value.
fn bound_by(node: &SyntaxNode) -> Option<(String, ast::LetStmt)> {
    let mut child = node.clone();
    loop {
        let parent = child.parent()?;
        if let Some(statement) = ast::LetStmt::cast(parent.clone()) {
            if statement.initializer()?.syntax() != &child || statement.let_else().is_some() {
                return None;
            }
            let ast::Pat::IdentPat(pattern) = statement.pat()? else {
                return None;
            };
            if pattern.mut_token().is_some()
                || pattern.ref_token().is_some()
                || pattern.pat().is_some()
            {
                return None;
            }
            return Some((pattern.name()?.text().to_string(), statement));
        }
        if !passes_on_a_value(&parent, &child) {
            return None;
        }
        child = parent;
    }
}

/// Whether `parent` only works out a value from `child`, doing nothing else with it.
fn passes_on_a_value(parent: &SyntaxNode, child: &SyntaxNode) -> bool {
    match parent.kind() {
        SyntaxKind::PAREN_EXPR | SyntaxKind::CAST_EXPR => true,
        SyntaxKind::BIN_EXPR => ast::BinExpr::cast(parent.clone())
            .and_then(|binary| binary.op_kind())
            .is_some_and(|op| matches!(op, BinaryOp::ArithOp(_) | BinaryOp::CmpOp(_))),
        SyntaxKind::PREFIX_EXPR => ast::PrefixExpr::cast(parent.clone())
            .and_then(|prefix| prefix.op_kind())
            .is_some_and(|op| matches!(op, ast::UnaryOp::Neg | ast::UnaryOp::Not)),
        // Its arguments, which the argument list let through.
        SyntaxKind::METHOD_CALL_EXPR | SyntaxKind::CALL_EXPR
            if child.kind() == SyntaxKind::ARG_LIST =>
        {
            true
        }
        SyntaxKind::METHOD_CALL_EXPR => {
            // The value the method is called on.
            ast::MethodCallExpr::cast(parent.clone()).is_some_and(|call| {
                call.receiver()
                    .is_some_and(|receiver| receiver.syntax() == child)
                    && value_method(&call)
            })
        }
        SyntaxKind::ARG_LIST => match parent.parent() {
            Some(call) if ast::MethodCallExpr::can_cast(call.kind()) => {
                ast::MethodCallExpr::cast(call).is_some_and(|call| value_method(&call))
            }
            Some(call) if ast::CallExpr::can_cast(call.kind()) => ast::CallExpr::cast(call)
                .and_then(|call| call.expr())
                .is_some_and(|path| DURATIONS.contains(&last_segment(&plain(path.syntax())))),
            _ => false,
        },
        _ => false,
    }
}

fn value_method(call: &ast::MethodCallExpr) -> bool {
    call.name_ref()
        .is_some_and(|name| VALUE_METHODS.contains(&name.text().as_str()))
}

/// Whether `name` has one binding in `scope`, is read at least once, and is read only as an output
/// macro's argument.
fn only_printed(name: &str, scope: &SyntaxNode) -> bool {
    let bindings = scope
        .descendants()
        .filter_map(ast::IdentPat::cast)
        .filter(|pattern| pattern.name().is_some_and(|own| own.text() == name))
        .count();
    if bindings != 1
        || scope
            .descendants()
            .filter_map(ast::NameRef::cast)
            .any(|read| read.text() == name)
    {
        return false;
    }
    let mut reads = 0;
    for call in scope.descendants().filter_map(ast::MacroCall::cast) {
        let Some(tree) = call.token_tree() else {
            continue;
        };
        let found = mentions(name, &tree);
        if found.is_empty() {
            continue;
        }
        if !is_output(&call)
            || in_a_closure(call.syntax(), scope)
            || !found.iter().all(|token| plain_argument(token, &tree))
        {
            return false;
        }
        reads += found.len();
    }
    reads > 0
}

/// `info!`, `println!`, `tracing::warn!`: the macros whose statement only writes output.
fn is_output(call: &ast::MacroCall) -> bool {
    call.path()
        .and_then(|path| path.segment())
        .is_some_and(|segment| {
            OUTPUT_MACROS.contains(&segment.syntax().text().to_string().as_str())
        })
}

/// Whether a closure between `node` and `scope` holds it.
fn in_a_closure(node: &SyntaxNode, scope: &SyntaxNode) -> bool {
    node.ancestors()
        .take_while(|ancestor| ancestor != scope)
        .any(|ancestor| ast::ClosureExpr::can_cast(ancestor.kind()))
}

/// The tokens of a macro's arguments that name `name`: the name itself, or a string with
/// `{name}` or `{name:...}` in it, as a format string captures a variable.
fn mentions(name: &str, tree: &ast::TokenTree) -> Vec<SyntaxToken> {
    let captured = [format!("{{{name}}}"), format!("{{{name}:")];
    tree.syntax()
        .descendants_with_tokens()
        .filter_map(NodeOrToken::into_token)
        .filter(|token| match token.kind() {
            SyntaxKind::IDENT => token.text() == name,
            SyntaxKind::STRING => captured
                .iter()
                .any(|capture| token.text().contains(capture.as_str())),
            _ => false,
        })
        .collect()
}

/// Whether `token` is one of the macro's own arguments, as a whole: in its own tokens, not another
/// macro's or a group's; a string, or the name with `,` `(` `=` `%` `?` or `&` before it and `,`,
/// `=` or the end after it, a method of [`VALUE_METHODS`] called on it in between.
fn plain_argument(token: &SyntaxToken, tree: &ast::TokenTree) -> bool {
    if token.parent().as_ref() != Some(tree.syntax()) {
        return false;
    }
    if token.kind() == SyntaxKind::STRING {
        return true;
    }
    let before = previous(token);
    if !before.as_ref().is_some_and(|before| {
        matches!(
            before.kind(),
            SyntaxKind::COMMA
                | SyntaxKind::L_PAREN
                | SyntaxKind::L_BRACK
                | SyntaxKind::L_CURLY
                | SyntaxKind::EQ
                | SyntaxKind::PERCENT
                | SyntaxKind::QUESTION
                | SyntaxKind::AMP
        )
    }) {
        return false;
    }
    let mut after = next(&NodeOrToken::Token(token.clone()));
    loop {
        match after {
            Some(NodeOrToken::Token(ref here)) if here.kind() == SyntaxKind::DOT => {
                let method = next(&NodeOrToken::Token(here.clone()));
                let Some(NodeOrToken::Token(method)) = method else {
                    return false;
                };
                if method.kind() != SyntaxKind::IDENT || !VALUE_METHODS.contains(&method.text()) {
                    return false;
                }
                let Some(NodeOrToken::Node(arguments)) = next(&NodeOrToken::Token(method)) else {
                    return false;
                };
                if arguments.text() != "()" {
                    return false;
                }
                after = next(&NodeOrToken::Node(arguments));
            }
            Some(NodeOrToken::Token(ref here)) => {
                return matches!(
                    here.kind(),
                    SyntaxKind::COMMA
                        | SyntaxKind::EQ
                        | SyntaxKind::R_PAREN
                        | SyntaxKind::R_BRACK
                        | SyntaxKind::R_CURLY
                );
            }
            _ => return false,
        }
    }
}

/// The token before `token` in its group, past whitespace and comments.
fn previous(token: &SyntaxToken) -> Option<SyntaxToken> {
    let mut here = token.prev_sibling_or_token();
    while let Some(element) = here {
        match element {
            NodeOrToken::Token(ref before) if before.kind().is_trivia() => {
                here = before.prev_sibling_or_token();
            }
            NodeOrToken::Token(before) => return Some(before),
            NodeOrToken::Node(_) => return None,
        }
    }
    None
}

/// What comes after `element` in its group, past whitespace and comments.
fn next(element: &ra_ap_syntax::SyntaxElement) -> Option<ra_ap_syntax::SyntaxElement> {
    let mut here = element.next_sibling_or_token();
    while let Some(after) = here {
        match after {
            NodeOrToken::Token(ref token) if token.kind().is_trivia() => {
                here = token.next_sibling_or_token();
            }
            _ => return Some(after),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each function's mutants that change a value, `replacement (why it's skipped)`. Each call has
    /// a directory of its own: the tests run at once.
    fn mutants(source: &str) -> BTreeMap<String, Vec<String>> {
        static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("mutastic-logged-{}-{call}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let found = find(&dir, "sample", &[PathBuf::from("src/lib.rs")], ALL);
        std::fs::remove_dir_all(&dir).ok();
        let mut by_function: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for mutant in found
            .mutants
            .iter()
            .filter(|mutant| VALUES.contains(&mutant.operator))
        {
            let why = mutant
                .skipped
                .as_ref()
                .map(|why| format!(" ({why})"))
                .unwrap_or_default();
            by_function
                .entry(found.functions[mutant.function].path.clone())
                .or_default()
                .push(format!("{}{why}", mutant.replacement));
        }
        by_function
    }

    /// Every value mutant of each function named `only_…` is arid; none of those named `kept_…`
    /// is, and each has one.
    fn assert_only_and_kept(source: &str) {
        let found = mutants(source);
        let arid = format!(" ({ONLY_FEEDS_A_LOG_LINE})");
        let mut only = 0;
        let mut kept = 0;
        for (path, changes) in &found {
            let function = path.rsplit("::").next().unwrap_or(path);
            if function == "record" {
                continue;
            }
            if function.starts_with("only_") {
                only += 1;
                assert!(
                    changes.iter().all(|change| change.ends_with(&arid)),
                    "{function}: {changes:?}"
                );
            } else {
                kept += 1;
                assert!(
                    changes.iter().all(|change| !change.ends_with(&arid)),
                    "{function}: {changes:?}"
                );
                assert!(
                    changes.iter().any(|change| !change.contains(" (")),
                    "{function}: {changes:?}"
                );
            }
        }
        assert!(only > 0 && kept > 0, "{found:?}");
    }

    #[test]
    fn a_let_only_printed_is_arid() {
        assert_only_and_kept(
            r#"
use std::time::{Duration, Instant};

pub fn only_captured(start: Instant, base: u128) {
    let took = start.elapsed().as_millis() - base;
    log::info!("took {took} ms");
}

pub fn only_an_argument(a: i64, b: i64) {
    let d = a - b;
    println!("{}", d);
}

pub fn only_formatted(a: i64, b: i64) {
    let d = a * 2 + b;
    eprintln!("{d:>8}");
}

pub fn only_read_twice(a: u64) {
    let left = a.saturating_sub(1) * 3;
    tracing::info!(left, "left");
    tracing::debug!(?left);
    tracing::warn!(left = left, "again");
}

pub fn only_through_a_method(a: f64, b: f64) {
    let ratio = (a / b).abs();
    println!("{}", ratio.round());
}

pub fn only_a_duration(a: u64) {
    let wait = Duration::from_millis(a * 1000);
    log::debug!("{:?}", wait);
}

pub fn only_in_a_branch(a: i32, verbose: bool) {
    let next = a + 1;
    if verbose {
        println!("next {next}");
    }
}

pub fn kept_returned(a: i64, b: i64) -> i64 {
    let d = a - b;
    println!("{d}");
    d
}

pub fn kept_compared(a: i64, b: i64) -> bool {
    let d = a - b;
    log::info!("{d}");
    d > 10
}

pub fn kept_passed_on(a: i64, out: &mut Vec<i64>) {
    let d = a - 1;
    println!("{d}");
    out.push(d);
}

pub fn kept_formatted(a: i64) -> String {
    let d = a - 1;
    println!("{d}");
    format!("{d}")
}

pub fn kept_written(a: i64, out: &mut String) {
    use std::fmt::Write;
    let d = a - 1;
    write!(out, "{d}").ok();
}

pub fn kept_asserted(a: i64) {
    let d = a - 1;
    assert!(d >= 0, "{d}");
}

pub fn kept_in_a_loop(items: &[i64], limit: i64) {
    for item in items {
        let d = item * 2;
        println!("{d}");
        if d > limit {
            break;
        }
    }
}

pub fn kept_shadowed(a: i64) -> i64 {
    let d = a - 1;
    println!("{d}");
    let d = 7;
    d
}

pub fn kept_in_a_closure(a: i64) -> impl Fn() {
    let d = a - 1;
    move || println!("{d}")
}

pub fn kept_mut(a: i64) {
    let mut d = a - 1;
    d += 1;
    println!("{d}");
}

pub fn kept_through_a_call(a: i64) {
    let d = record(a - 1);
    println!("{d}");
}

pub fn kept_through_a_branch(a: i64, up: bool) {
    let d = if a > 0 { record(a) } else { 0 };
    println!("{d}");
    let _ = up;
}

pub fn kept_inside_another_macro(a: i64) {
    let d = a - 1;
    println!("{}", stringify!(d));
}

pub fn kept_as_a_call_argument(a: i64) {
    let d = a - 1;
    println!("{}", record(d));
}

fn record(value: i64) -> i64 {
    value
}
"#,
        );
    }

    /// A value in a macro's tokens has no mutant at all; the `let`'s is the one this adds.
    #[test]
    fn a_value_in_the_macro_itself_has_no_mutant() {
        let found = mutants("pub fn f(a: i64, b: i64) {\n    println!(\"{}\", a - b);\n}\n");
        assert!(found.is_empty(), "{found:?}");
    }
}
