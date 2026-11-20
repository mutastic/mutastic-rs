//! Arid sites the Java edition skips too, made so they're counted, kept by id, audited and brought
//! back by `mutate:`, and skipped:
//!
//! ```text
//! thread::sleep(pause);                                  // only waits
//! thread::sleep(Duration::from_millis(attempt * 100));  // only sets how long to wait
//! Vec::with_capacity(orders.len() * 2)                   // only sizes a collection
//! Err("amount must be positive")                         // only an error's message
//! order.expect("an order to ship")                       // only a failed check's message
//! let mut count = 0;                                     // a variable's first value
//! impl fmt::Debug for Order { .. }                       // a debug representation
//! impl Hash for Order { .. }                             // any hash consistent with `Eq`
//! ```
//!
//! A time that goes anywhere but a sleep (a timeout, a deadline, a backoff kept in a variable)
//! stays counted: it decides whether the code gives up, which a test can see. So does a `Display`
//! impl, which is often what a person or a program reads.

use super::*;
use ra_ap_syntax::ast::HasArgList;

use crate::arid::{
    DEBUG, ERROR_MESSAGE, FAILED_CHECK_MESSAGE, FIRST_VALUE, HASH, ONLY_SIZES, ONLY_WAITS,
    WAIT_TIME,
};

/// The operators that change a number worked out, or given, by the code.
const NUMBERS: &[&str] = &[
    CONSTANT_VALUE,
    MATH,
    ARITHMETIC_REPLACEMENT,
    OTHER_ARITHMETIC,
    KEEP_OPERAND,
    REMOVE_NEGATION,
    BITWISE,
];

/// Calls whose number is only the room a collection starts with or makes.
const CAPACITY_CALLS: &[&str] = &[
    "with_capacity",
    "with_capacity_and_hasher",
    "reserve",
    "reserve_exact",
];

/// `Duration`'s constructors from a number.
const DURATIONS: &[&str] = &["from_secs", "from_millis", "from_micros", "from_nanos"];

/// Calls whose text only tells a person what went wrong.
const ERROR_CALLS: &[&str] = &["Err", "context", "with_context", "wrap_err"];

/// What makes a `String` (or a `Box<dyn Error>`) of a literal, and nothing else.
const TEXT_CONVERSIONS: &[&str] = &["into", "to_string", "to_owned"];

/// Why the mutant `operator` makes at `node` is arid, if it is.
pub(super) fn reason(operator: &str, node: &SyntaxNode) -> Option<String> {
    representation(node)
        .or_else(|| {
            (operator == REMOVE_VOID_CALL || operator == REMOVE_DISCARDED_CALL)
                .then(|| sleeps(node))
                .flatten()
        })
        .or_else(|| {
            (operator == STRING_CONSTANT)
                .then(|| message(node))
                .flatten()
        })
        .or_else(|| {
            NUMBERS
                .contains(&operator)
                .then(|| number(operator, node))
                .flatten()
        })
        .map(str::to_string)
}

/// Inside `impl Debug for T`'s `fmt`, or `impl Hash for T`'s `hash`.
fn representation(node: &SyntaxNode) -> Option<&'static str> {
    let function = node.ancestors().find_map(ast::Fn::cast)?;
    let name = function.name()?.text().to_string();
    let implementation = ast::Impl::cast(function.syntax().parent()?.parent()?)?;
    let tr = plain(implementation.trait_()?.syntax()).replace(' ', "");
    match (last_segment(&tr), name.as_str()) {
        ("Debug", "fmt") => Some(DEBUG),
        ("Hash", "hash") => Some(HASH),
        _ => None,
    }
}

/// `thread::sleep(d);` or `tokio::time::sleep(d).await;` removed.
fn sleeps(statement: &SyntaxNode) -> Option<&'static str> {
    let expr = ast::ExprStmt::cast(statement.clone())?.expr()?;
    let (call, awaited) = match expr {
        ast::Expr::AwaitExpr(awaited) => (awaited.expr()?, true),
        other => (other, false),
    };
    let ast::Expr::CallExpr(call) = call else {
        return None;
    };
    let path = plain(call.expr()?.syntax()).replace(' ', "");
    let sleep = last_segment(&path) == "sleep" && (awaited || path.ends_with("thread::sleep"));
    sleep.then_some(ONLY_WAITS)
}

/// A literal that only says what went wrong: an `Err`'s or a `context`'s, or an `expect`'s.
fn message(literal: &SyntaxNode) -> Option<&'static str> {
    let mut given = literal.clone();
    // `"..".into()`, `"..".to_string()`, `String::from("..")`: still only the text.
    loop {
        let parent = given.parent()?;
        if let Some(call) = ast::MethodCallExpr::cast(parent.clone()) {
            let converts = call.receiver().is_some_and(|r| r.syntax() == &given)
                && call
                    .arg_list()
                    .is_some_and(|list| list.args().next().is_none())
                && call
                    .name_ref()
                    .is_some_and(|name| TEXT_CONVERSIONS.contains(&name.text().as_str()));
            if !converts {
                return None;
            }
            given = parent;
            continue;
        }
        if ast::ArgList::can_cast(parent.kind()) {
            let call = parent.parent()?;
            let name = callee(&call)?;
            if name == "from" && conversion_from(&call) {
                given = call;
                continue;
            }
            return if ERROR_CALLS.contains(&name.as_str()) {
                Some(ERROR_MESSAGE)
            } else if name == "expect" {
                Some(FAILED_CHECK_MESSAGE)
            } else {
                None
            };
        }
        return None;
    }
}

/// `String::from(..)` or `Box::from(..)`.
fn conversion_from(call: &SyntaxNode) -> bool {
    let Some(ast::Expr::PathExpr(path)) = ast::CallExpr::cast(call.clone()).and_then(|c| c.expr())
    else {
        return false;
    };
    let path = plain(path.syntax()).replace(' ', "");
    path == "String::from" || path == "Box::from"
}

/// A number that only sizes a collection or says how long to sleep, or a variable's first 0 or 1.
fn number(operator: &str, node: &SyntaxNode) -> Option<&'static str> {
    if operator == CONSTANT_VALUE
        && node
            .parent()
            .is_some_and(|p| p.kind() == SyntaxKind::LET_STMT)
        && integer_literal(&plain(node).replace(' ', "")).is_some_and(|(v, _)| matches!(v, 0 | 1))
    {
        return Some(FIRST_VALUE);
    }
    let (call, position) = taken_by(node)?;
    let name = callee(&call)?;
    if CAPACITY_CALLS.contains(&name.as_str())
        && (name != "with_capacity_and_hasher" || position == 0)
    {
        return Some(ONLY_SIZES);
    }
    if is_sleep(&call) {
        return Some(WAIT_TIME);
    }
    let duration = DURATIONS.contains(&name.as_str()) || is_duration_new(&call);
    if duration {
        let (outer, _) = taken_by(&call)?;
        return is_sleep(&outer).then_some(WAIT_TIME);
    }
    None
}

/// The call `node`'s value goes straight into as an argument, through what only works it out
/// (parentheses, a cast, arithmetic, a minus), and which argument it is.
fn taken_by(node: &SyntaxNode) -> Option<(SyntaxNode, usize)> {
    let mut value = node.clone();
    loop {
        let parent = value.parent()?;
        match parent.kind() {
            SyntaxKind::PAREN_EXPR | SyntaxKind::CAST_EXPR | SyntaxKind::PREFIX_EXPR => {}
            SyntaxKind::BIN_EXPR => {
                let arithmetic = ast::BinExpr::cast(parent.clone())
                    .and_then(|b| b.op_kind())
                    .is_some_and(|op| matches!(op, BinaryOp::ArithOp(_)));
                if !arithmetic {
                    return None;
                }
            }
            SyntaxKind::ARG_LIST => {
                let list = ast::ArgList::cast(parent.clone())?;
                let position = list.args().position(|arg| arg.syntax() == &value)?;
                return Some((parent.parent()?, position));
            }
            _ => return None,
        }
        value = parent;
    }
}

/// `thread::sleep(..)`, `tokio::time::sleep(..)`, `sleep(..)`: a function called `sleep`.
fn is_sleep(call: &SyntaxNode) -> bool {
    ast::CallExpr::can_cast(call.kind()) && callee(call).is_some_and(|name| name == "sleep")
}

/// `Duration::new(secs, nanos)`.
fn is_duration_new(call: &SyntaxNode) -> bool {
    ast::CallExpr::cast(call.clone())
        .and_then(|c| c.expr())
        .is_some_and(|path| {
            plain(path.syntax())
                .replace(' ', "")
                .ends_with("Duration::new")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Each function's mutants, `site -> replacement (why it's skipped)`, operators by short name.
    fn mutants(source: &str) -> BTreeMap<String, Vec<String>> {
        static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("mutastic-arid-sites-{}-{call}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let found = find(&dir, "sample", &[PathBuf::from("src/lib.rs")], ALL);
        std::fs::remove_dir_all(&dir).ok();
        let mut by_function: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for mutant in &found.mutants {
            let why = mutant
                .skipped
                .as_ref()
                .map(|why| format!(" ({why})"))
                .unwrap_or_default();
            let site = source[mutant.site.clone()].replace('\n', " ");
            by_function
                .entry(found.functions[mutant.function].path.clone())
                .or_default()
                .push(format!("{site} -> {}{why}", mutant.replacement));
        }
        by_function
    }

    fn of<'m>(found: &'m BTreeMap<String, Vec<String>>, function: &str) -> &'m [String] {
        found.get(function).map_or(&[], Vec::as_slice)
    }

    #[test]
    fn a_pure_sleep_is_arid_and_a_timeout_is_counted() {
        let found = mutants(
            r#"
use std::thread;
use std::time::Duration;

pub fn pause(attempt: u64) {
    thread::sleep(Duration::from_millis(attempt * 100));
}

pub async fn later(pause: Duration) {
    tokio::time::sleep(pause).await;
}

pub fn timeout() -> Duration {
    let limit = Duration::from_secs(30);
    limit
}
"#,
        );
        assert_eq!(
            of(&found, "pause")[2..],
            [
                "thread::sleep(Duration::from_millis(attempt * 100)); ->  (arid: only waits)",
                "attempt * 100 -> (attempt) / (100) (arid: only sets how long to wait)",
                "attempt * 100 -> (attempt) + (100) (arid: only sets how long to wait)",
                "attempt * 100 -> (attempt) - (100) (arid: only sets how long to wait)",
                "attempt * 100 -> (attempt) % (100) (arid: only sets how long to wait)",
                "attempt * 100 -> attempt (arid: only sets how long to wait)",
                "100 -> 101 (arid: only sets how long to wait)",
                "100 -> 99 (arid: only sets how long to wait)",
                "100 -> 0 (arid: only sets how long to wait)",
                "100 -> 1 (arid: only sets how long to wait)",
            ],
            "{found:#?}"
        );
        assert_eq!(
            of(&found, "later")[2..],
            ["tokio::time::sleep(pause).await; ->  (arid: only waits)"],
            "{found:#?}"
        );
        assert!(
            of(&found, "timeout").iter().any(|m| m == "30 -> 0"),
            "a timeout's number is counted: {found:#?}"
        );
        assert!(
            of(&found, "timeout").iter().all(|m| !m.contains("arid")),
            "{found:#?}"
        );
    }

    #[test]
    fn a_capacity_is_arid_and_a_bound_is_counted() {
        let found = mutants(
            r#"
pub fn names(orders: &[u32]) -> Vec<String> {
    let mut names = Vec::with_capacity(orders.len() * 2);
    names.reserve(16);
    names
}

pub fn bound(orders: &[u32]) -> usize {
    orders.len() * 2
}
"#,
        );
        let names = of(&found, "names");
        assert!(
            names.iter().any(|m| m
                == "orders.len() * 2 -> (orders.len()) / (2) (arid: only sizes a collection's first allocation)"),
            "{found:#?}"
        );
        assert!(
            names
                .iter()
                .any(|m| m == "16 -> 17 (arid: only sizes a collection's first allocation)"),
            "{found:#?}"
        );
        assert!(
            of(&found, "bound").iter().all(|m| !m.contains("arid")),
            "{found:#?}"
        );
    }

    #[test]
    fn an_errors_text_and_a_first_zero_or_one_are_arid() {
        let found = mutants(
            r#"
pub fn checked(order: Option<u32>, amount: i64) -> Result<u32, String> {
    let mut count = 0;
    let mut start = 2;
    if amount < 0 {
        return Err("amount must be positive".into());
    }
    count += 1;
    start += count;
    let shown = "an order";
    Ok(order.expect("an order to ship") + count + start + shown.len() as u32)
}
"#,
        );
        let checked = of(&found, "checked");
        for arid in [
            "0 -> 1 (arid: a variable's first value of 0 or 1)",
            "\"amount must be positive\" -> \"\" (arid: only an error's message)",
            "\"an order to ship\" -> \"\" (arid: only a failed check's message)",
        ] {
            assert!(checked.iter().any(|m| m == arid), "{arid}: {found:#?}");
        }
        for counted in ["2 -> 3", "\"an order\" -> \"\""] {
            assert!(
                checked.iter().any(|m| m == counted),
                "{counted}: {found:#?}"
            );
        }
    }

    #[test]
    fn a_debug_or_hash_impl_is_arid_and_display_is_counted() {
        let found = mutants(
            r#"
use std::fmt;
use std::hash::{Hash, Hasher};

pub struct Order { id: u32 }

impl fmt::Debug for Order {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.id > 0 { f.write_str("Order") } else { f.write_str("none") }
    }
}

impl fmt::Display for Order {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.id > 0 { f.write_str("Order") } else { f.write_str("none") }
    }
}

impl Hash for Order {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Order {
    pub fn new(id: u32) -> Option<Self> {
        Some(Order { id })
    }
}
"#,
        );
        let debug = of(&found, "<Order as fmt::Debug>::fmt");
        assert!(!debug.is_empty(), "{found:#?}");
        assert!(
            debug.iter().all(
                |m| m.ends_with("(arid: `Debug::fmt` is a debug representation)")
                    || m.contains("(equivalent: ")
            ),
            "{found:#?}"
        );
        assert!(debug.iter().any(|m| m.contains("-> Ok(())")), "{found:#?}");
        let display = of(&found, "<Order as fmt::Display>::fmt");
        assert!(
            display.iter().any(|m| m.contains("-> Ok(())"))
                && display.iter().all(|m| !m.contains("arid")),
            "{found:#?}"
        );
        let hash = of(&found, "<Order as Hash>::hash");
        assert!(!hash.is_empty(), "{found:#?}");
        assert!(
            hash.iter().all(
                |m| m.ends_with("(arid: `Hash::hash` may give any value consistent with `Eq`)")
            ),
            "{found:#?}"
        );
        assert!(
            of(&found, "Order::new")
                .iter()
                .any(|m| m.ends_with("-> None")),
            "{found:#?}"
        );
    }
}
