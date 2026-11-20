//! Thread settings that change only how a thread looks or what it costs, not whether, when or how
//! often work runs, are arid, as in the Java edition:
//!
//! ```text
//! thread::Builder::new().name("squarer".into())        // only names the thread
//! thread::Builder::new().stack_size(64 * 1024)         // only sizes its stack, while above zero
//! runtime::Builder::new_multi_thread().thread_name("w") // tokio's, and thread_name_fn's body
//! runtime::Builder::new_multi_thread().thread_stack_size(1 << 20)
//! runtime::Builder::new_multi_thread().thread_keep_alive(Duration::from_secs(5 * 60))
//! ```
//!
//! The call is known by the chain it's on, which visibly starts at `thread::Builder::new()` (or a
//! `Builder::new()` that the file imports from a `thread` module) or at a tokio runtime builder's
//! `Builder::new_multi_thread()` or `new_current_thread()`: any other `.name(..)` is left alone.
//! Only mutants that change a value in the call's argument are skipped: a stack size or keep-alive
//! only while the source shows it stays above zero. A crate that reads names
//! (`thread::current().name()`) keeps its name mutants: there, names mean something.

use super::*;
use ra_ap_syntax::ast::HasArgList;

use crate::arid::{KEEP_ALIVE, NAMES, STACK};

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

/// What a builder's call sets.
#[derive(Clone, Copy)]
enum Setting {
    Name,
    Stack,
    KeepAlive,
}

/// Why the mutant `operator` makes at `node`, replacing it with `replacement`, shouldn't run, if
/// it only changes a thread's name, its stack or how long idle threads live.
pub(super) fn reason(
    operator: &str,
    node: &SyntaxNode,
    replacement: &str,
    reads_names: bool,
) -> Option<String> {
    if operator == OMIT_CALL {
        // The step itself left out: the thread has no name, or the default stack or keep-alive.
        let setting = setting(&ast::MethodCallExpr::cast(node.clone())?)?;
        return arid(setting, matches!(setting, Setting::Name) && reads_names);
    }
    if !VALUES.contains(&operator) {
        return None;
    }
    let (setting, argument) = setting_of(node)?;
    let counted = match setting {
        Setting::Name => reads_names,
        Setting::Stack => !spliced(&argument, node, replacement).is_some_and(|size| size > 0),
        Setting::KeepAlive => !duration(&argument, node, replacement).is_some_and(|time| time > 0),
    };
    arid(setting, counted)
}

fn arid(setting: Setting, counted: bool) -> Option<String> {
    (!counted).then(|| {
        match setting {
            Setting::Name => NAMES,
            Setting::Stack => STACK,
            Setting::KeepAlive => KEEP_ALIVE,
        }
        .to_string()
    })
}

/// Whether the crate reads a thread's name: `thread::current().name()`, `handle.thread().name()`.
/// A name in a macro's tokens, a log line's, isn't seen.
pub(super) fn reads_names(tree: &SourceFile) -> bool {
    tree.syntax()
        .descendants()
        .filter_map(ast::MethodCallExpr::cast)
        .filter(|call| call.name_ref().is_some_and(|name| name.text() == "name"))
        .filter(|call| {
            call.arg_list()
                .is_some_and(|args| args.args().next().is_none())
        })
        .filter_map(|call| call.receiver())
        .any(|receiver| match receiver {
            ast::Expr::CallExpr(call) => call
                .expr()
                .is_some_and(|path| last_segment(&plain(path.syntax())) == "current"),
            ast::Expr::MethodCallExpr(call) => {
                call.name_ref().is_some_and(|name| name.text() == "thread")
            }
            _ => false,
        })
}

/// The setting whose argument `node` is in, and the argument, if it's a thread builder's.
fn setting_of(node: &SyntaxNode) -> Option<(Setting, ast::Expr)> {
    node.ancestors()
        .filter_map(ast::ArgList::cast)
        .find_map(|list| {
            let call = ast::MethodCallExpr::cast(list.syntax().parent()?)?;
            Some((setting(&call)?, list.args().next()?))
        })
}

/// What `call` sets, if it's a step of a thread builder's chain that sets a name, a stack size or
/// a keep-alive.
fn setting(call: &ast::MethodCallExpr) -> Option<Setting> {
    Some(
        match (call.name_ref()?.text().as_str(), builder_of(call)?) {
            ("name", Builder::Std) => Setting::Name,
            ("stack_size", Builder::Std) => Setting::Stack,
            ("thread_name" | "thread_name_fn", Builder::Tokio) => Setting::Name,
            ("thread_stack_size", Builder::Tokio) => Setting::Stack,
            ("thread_keep_alive", Builder::Tokio) => Setting::KeepAlive,
            _ => return None,
        },
    )
}

#[derive(PartialEq, Eq)]
enum Builder {
    Std,
    Tokio,
}

/// Which builder the chain `call` is on starts at, if it visibly starts at one.
fn builder_of(call: &ast::MethodCallExpr) -> Option<Builder> {
    let mut receiver = call.receiver()?;
    loop {
        receiver = match receiver {
            ast::Expr::MethodCallExpr(inner) => inner.receiver()?,
            ast::Expr::ParenExpr(inner) => inner.expr()?,
            ast::Expr::CallExpr(start) => return started_by(&start),
            _ => return None,
        };
    }
}

/// `thread::Builder::new()`, or `Builder::new()` imported from a `thread` module; tokio's
/// `Builder::new_multi_thread()` or `new_current_thread()`, by whatever path.
fn started_by(start: &ast::CallExpr) -> Option<Builder> {
    let path = plain(start.expr()?.syntax()).replace(' ', "");
    if path.ends_with("Builder::new_multi_thread") || path.ends_with("Builder::new_current_thread")
    {
        return Some(Builder::Tokio);
    }
    if path.ends_with("thread::Builder::new") || path == "thread::Builder::new" {
        return Some(Builder::Std);
    }
    (path == "Builder::new" && imports_thread_builder(start.syntax())).then_some(Builder::Std)
}

/// Whether the file has `use std::thread::Builder;` or a `use` naming `Builder` in a `thread`
/// module: `use std::thread::{self, Builder};`.
fn imports_thread_builder(node: &SyntaxNode) -> bool {
    let Some(root) = node.ancestors().last() else {
        return false;
    };
    root.descendants()
        .filter_map(ast::Use::cast)
        .map(|item| plain(item.syntax()).replace(' ', ""))
        .any(|item| {
            item.contains("thread::Builder")
                || (item.contains("thread::{") && item.contains("Builder"))
        })
}

/// A keep-alive's `Duration::from_secs(..)` (or `from_millis`, `from_micros`, `from_nanos`), its
/// number worked out with the mutant in.
fn duration(argument: &ast::Expr, node: &SyntaxNode, replacement: &str) -> Option<i128> {
    let ast::Expr::CallExpr(call) = argument else {
        return None;
    };
    let unit = last_segment(&plain(call.expr()?.syntax())).to_string();
    if !matches!(
        unit.as_str(),
        "from_secs" | "from_millis" | "from_micros" | "from_nanos"
    ) {
        return None;
    }
    let mut args = call.arg_list()?.args();
    let (Some(number), None) = (args.next(), args.next()) else {
        return None;
    };
    spliced(&number, node, replacement)
}

/// The value of `argument`, with the mutant's `replacement` written over `node`, if it's whole
/// numbers worked out with `+`, `-`, `*`, `/`, `%` and shifts.
fn spliced(argument: &ast::Expr, node: &SyntaxNode, replacement: &str) -> Option<i128> {
    let whole = argument.syntax().text_range();
    let site = node.text_range();
    if !whole.contains_range(site) {
        return None;
    }
    let text = argument.syntax().text().to_string();
    let start = usize::from(site.start() - whole.start());
    let end = usize::from(site.end() - whole.start());
    let changed = format!("{}{replacement}{}", &text[..start], &text[end..]);
    let file = SourceFile::parse(&format!("const _: u128 = {changed};"), Edition::CURRENT).tree();
    let body = file
        .syntax()
        .descendants()
        .find_map(ast::Const::cast)?
        .body()?;
    value(&body)
}

fn value(expr: &ast::Expr) -> Option<i128> {
    match expr {
        ast::Expr::Literal(literal) => {
            integer_literal(&literal.syntax().text().to_string()).map(|(number, _)| number)
        }
        ast::Expr::ParenExpr(inner) => value(&inner.expr()?),
        ast::Expr::CastExpr(cast) => value(&cast.expr()?),
        ast::Expr::PrefixExpr(prefix) if prefix.op_kind() == Some(ast::UnaryOp::Neg) => {
            value(&prefix.expr()?)?.checked_neg()
        }
        ast::Expr::BinExpr(binary) => {
            let (a, b) = (value(&binary.lhs()?)?, value(&binary.rhs()?)?);
            match binary.op_kind()? {
                BinaryOp::ArithOp(ArithOp::Add) => a.checked_add(b),
                BinaryOp::ArithOp(ArithOp::Sub) => a.checked_sub(b),
                BinaryOp::ArithOp(ArithOp::Mul) => a.checked_mul(b),
                BinaryOp::ArithOp(ArithOp::Div) => a.checked_div(b),
                BinaryOp::ArithOp(ArithOp::Rem) => a.checked_rem(b),
                BinaryOp::ArithOp(ArithOp::Shl) => a.checked_shl(u32::try_from(b).ok()?),
                BinaryOp::ArithOp(ArithOp::Shr) => a.checked_shr(u32::try_from(b).ok()?),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each function's mutants, `replacement (why it's skipped)`. Each call has a directory of its
    /// own: the tests run at once.
    fn mutants(source: &str) -> BTreeMap<String, Vec<String>> {
        static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("mutastic-threads-{}-{call}", std::process::id()));
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
            by_function
                .entry(found.functions[mutant.function].path.clone())
                .or_default()
                .push(format!("{}{why}", mutant.replacement));
        }
        by_function
    }

    const SOURCE: &str = r#"
use std::thread;
use std::time::Duration;

pub fn named() -> thread::JoinHandle<u32> {
    thread::Builder::new()
        .name("squarer".to_string())
        .stack_size(64 * 1024)
        .spawn(|| 7)
        .expect("spawn")
}

pub fn fire_and_forget(flag: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    let _ = thread::Builder::new()
        .name(String::from("background"))
        .spawn(move || flag.store(true, std::sync::atomic::Ordering::SeqCst));
}

pub fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("tokio-worker")
        .thread_stack_size(3 * 1024 * 1024)
        .thread_keep_alive(Duration::from_millis(500 + 500))
        .build()
        .unwrap()
}

pub struct Person { name: String }

impl Person {
    pub fn name(self, name: &str) -> Self { Person { name: format!("{}{name}", self.name) } }
}

pub fn not_a_thread() -> Person {
    Person { name: String::new() }.name("ada")
}
"#;

    #[test]
    fn a_thread_builders_name_stack_and_keep_alive_are_arid() {
        let found = mutants(SOURCE);
        let named = &found["named"];
        assert!(
            named.contains(&"\"\" (arid: only names a thread)".to_string()),
            "{named:?}"
        );
        for kept in [
            "(64) + (1024) (arid: only sizes a thread's stack)",
            "65 (arid: only sizes a thread's stack)",
            "thread::Builder::new() (arid: only names a thread)",
            "(64) - (1024)",
        ] {
            assert!(named.contains(&kept.to_string()), "{kept}: {named:?}");
        }
        let runtime = &found["runtime"];
        assert!(
            runtime.contains(&"\"\" (arid: only names a thread)".to_string()),
            "{runtime:?}"
        );
        assert!(
            runtime.contains(&"1".to_string()),
            "worker_threads stays counted: {runtime:?}"
        );
        assert!(
            runtime.contains(&"3".to_string()),
            "worker_threads stays counted: {runtime:?}"
        );
        assert!(
            runtime.contains(&"(500) - (500)".to_string()),
            "a keep-alive of zero stays counted: {runtime:?}"
        );
        assert!(
            runtime.contains(
                &"(500) * (500) (arid: only sets how long idle threads live)".to_string()
            ),
            "{runtime:?}"
        );
        assert!(
            runtime
                .iter()
                .any(|each| each.ends_with("(arid: only sizes a thread's stack)")),
            "{runtime:?}"
        );
    }

    /// A spawn and its result decide whether the work runs: they're still made, and run.
    #[test]
    fn what_decides_whether_work_runs_still_runs() {
        let found = mutants(SOURCE);
        let forget = &found["fire_and_forget"];
        assert!(
            forget.contains(&"\"\" (arid: only names a thread)".to_string()),
            "{forget:?}"
        );
        assert!(
            forget
                .iter()
                .any(|each| !each.contains("arid") && each != "\"\""),
            "{forget:?}"
        );
        let other = &found["not_a_thread"];
        assert!(other.contains(&"\"\"".to_string()), "{other:?}");
    }

    #[test]
    fn a_crate_that_reads_names_keeps_them() {
        let source = format!(
            "{SOURCE}\npub fn on_worker() -> bool {{ thread::current().name() == Some(\"squarer\") }}\n"
        );
        let found = mutants(&source);
        let named = &found["named"];
        assert!(named.contains(&"\"\"".to_string()), "{named:?}");
        assert!(
            named
                .iter()
                .all(|each| !each.ends_with("(arid: only names a thread)")),
            "{named:?}"
        );
        assert!(
            named
                .iter()
                .any(|each| each.ends_with("(arid: only sizes a thread's stack)")),
            "{named:?}"
        );
    }

    #[test]
    fn a_builder_imported_from_a_thread_module_is_known() {
        let imported = "use std::thread::{self, Builder};\n\
            pub fn named() -> thread::JoinHandle<u32> { Builder::new().name(\"w\".into()).spawn(|| 1).unwrap() }\n";
        assert!(
            mutants(imported)["named"].contains(&"\"\" (arid: only names a thread)".to_string())
        );
        let other = "pub struct Builder;\n\
            impl Builder { pub fn new() -> Self { Builder } pub fn name(self, _: &str) -> u32 { 1 } }\n\
            pub fn named() -> u32 { Builder::new().name(\"w\") }\n";
        assert!(mutants(other)["named"].contains(&"\"\"".to_string()));
    }
}
