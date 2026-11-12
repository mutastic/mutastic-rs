//! A condition the build profile decides: `cfg!(debug_assertions)`. mutastic builds every mutant
//! with debug assertions on, as the test profile does, whatever the project's own profiles say, so
//! under mutastic the condition is always true, and `!cfg!(debug_assertions)` always false. A
//! mutant that forces the condition to the value it already has can't change what the tests see,
//! and is skipped as equivalent; the one that forces it the other way runs.
//!
//! Only the condition written as exactly that, wherever a condition's mutant forces it (alone, under
//! `!`, in parentheses, or as one side of `&&`): for `cfg!(all(…))`, another `cfg!`, a `cfg!` of
//! another path, or a name it was stored in, the mutants run.

use super::*;

/// Why a mutant is equivalent, when it forces `cfg!(debug_assertions)` to the value mutastic's
/// build already gives it.
pub(super) fn decided_by_profile(node: &SyntaxNode, change: &Change) -> Option<String> {
    let Change::Always { truth, .. } = change else {
        return None;
    };
    let expr = ast::Expr::cast(node.clone())?;
    let on = holds_under_mutastic(&expr)?;
    (*truth == on).then(|| {
        format!(
            "equivalent: differs only when debug assertions are off. That can't happen because \
             mutastic builds with them on, as the test profile does, so the condition is always \
             {on}."
        )
    })
}

/// What a condition is under mutastic's build, when it's `cfg!(debug_assertions)`, negated any
/// number of times, in any parentheses; none for anything else.
fn holds_under_mutastic(expr: &ast::Expr) -> Option<bool> {
    match expr {
        ast::Expr::ParenExpr(inner) => holds_under_mutastic(&inner.expr()?),
        ast::Expr::PrefixExpr(prefix) if prefix.op_kind() == Some(ast::UnaryOp::Not) => {
            holds_under_mutastic(&prefix.expr()?).map(|on| !on)
        }
        ast::Expr::MacroExpr(call) => is_debug_assertions(call).then_some(true),
        _ => None,
    }
}

/// `cfg!(debug_assertions)`, as std's macro: a path of the one segment `cfg`, given exactly that.
fn is_debug_assertions(call: &ast::MacroExpr) -> bool {
    let Some(call) = call.macro_call() else {
        return false;
    };
    let named_cfg = call
        .path()
        .is_some_and(|path| path.qualifier().is_none() && path.syntax().text() == "cfg");
    let given = call.token_tree().map(|tokens| {
        tokens
            .syntax()
            .text()
            .to_string()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
    });
    named_cfg && given.as_deref() == Some("(debug_assertions)")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each function's skipped mutants, by replacement.
    fn skipped(source: &str) -> BTreeMap<String, Vec<String>> {
        let dir = std::env::temp_dir().join(format!("mutastic-profile-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let found = find(&dir, "sample", &[PathBuf::from("src/lib.rs")], STRONGER);
        std::fs::remove_dir_all(&dir).ok();
        let mut skipped: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for mutant in &found.mutants {
            if mutant.skipped.is_some() {
                skipped
                    .entry(found.functions[mutant.function].path.clone())
                    .or_default()
                    .push(mutant.replacement.clone());
            }
        }
        skipped
    }

    #[test]
    fn a_condition_the_profile_decides_is_skipped_only_where_its_value_is_kept() {
        let source = "pub fn checked(x: i32) -> i32 { if cfg!(debug_assertions) { x + 1 } else { x } }\n\
             pub fn unchecked(x: i32) -> i32 { if !cfg!(debug_assertions) { x + 1 } else { x } }\n\
             pub fn wrapped(x: i32) -> i32 { if (cfg!( debug_assertions )) { x + 1 } else { x } }\n\
             pub fn combined(x: i32) -> i32 { if cfg!(all(debug_assertions, unix)) { x + 1 } else { x } }\n\
             pub fn other(x: i32) -> i32 { if cfg!(test) { x + 1 } else { x } }\n\
             pub fn and_more(x: i32) -> i32 { if cfg!(debug_assertions) && x > 0 { x + 1 } else { x } }\n\
             pub fn own(x: i32) -> i32 { if my::cfg!(debug_assertions) { x + 1 } else { x } }\n";
        let skipped = skipped(source);
        assert_eq!(skipped.get("checked"), Some(&vec!["true".to_string()]));
        // Under `!`, the mutants change the `cfg!` itself, and forcing that true keeps it.
        assert_eq!(skipped.get("unchecked"), Some(&vec!["true".to_string()]));
        assert_eq!(skipped.get("wrapped"), Some(&vec!["true".to_string()]));
        // Forced to the value it has, as one side of `&&`, too: the rest of the condition is untouched.
        assert_eq!(skipped.get("and_more"), Some(&vec!["true".to_string()]));
        for kept in ["combined", "other", "own"] {
            assert_eq!(skipped.get(kept), None, "{kept}: {skipped:?}");
        }
    }
}
