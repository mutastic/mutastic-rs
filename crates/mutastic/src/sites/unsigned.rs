//! A comparison of an unsigned value with 0: `n == 0` where `n` is a `usize`. The value can't be
//! below 0, so a mutant of the comparison that differs from it only below 0 is equivalent, and is
//! skipped: `n == 0` made `n <= 0`, `n > 0` made `n != 0`, `n >= 0` made always true.
//!
//! The rule acts only on what the source proves, as the `NonZero*` rule does. A value is unsigned
//! when it's a name or a field whose declared type is written `u8` to `u128` or `usize` (a
//! parameter, a `let` with its type written, or a field of a struct declared beside the code); a
//! cast `as` one of those; a call of a function declared beside the code to return one; the
//! index a `for` loop takes from `.enumerate()`; or arithmetic on those, which can't go below 0
//! either: it panics first. A type that's inferred or generic, a name a macro is given, or an item
//! of the project's own named as one of those types, and the mutants run. (A comparison of a
//! `.len()`, `.count()` or `.capacity()` with 0 isn't made at all: see `stronger`.)

use super::nonzero::{
    binding, field_type, generic_named, holds, is_zero, mirrored, scope_of, simple_name,
    unreferenced, untouched_by_macros, Sign,
};
use super::*;
use ra_ap_syntax::ast::{HasArgList, HasGenericParams, HasLoopBody};

/// The unsigned types, as written.
const UNSIGNED: &[&str] = &["u8", "u16", "u32", "u64", "u128", "usize"];

/// Why a comparison's mutant is equivalent, when it differs from the comparison only where an
/// unsigned value would be below 0.
pub(super) fn never_negative(node: &SyntaxNode, change: &Change) -> Option<String> {
    let expr = ast::BinExpr::cast(node.clone())?;
    let Some(BinaryOp::CmpOp(_)) = expr.op_kind() else {
        return None;
    };
    let before = expr.op_token()?.text().to_string();
    let (lhs, rhs) = (expr.lhs()?, expr.rhs()?);
    let (value, flipped) = match (is_zero(&lhs), is_zero(&rhs)) {
        (false, true) => (lhs, false),
        (true, false) => (rhs, true),
        _ => return None,
    };
    let facing = |op: &str| {
        if flipped {
            mirrored(op)
        } else {
            op.to_string()
        }
    };
    let before = facing(&before);
    let after: Box<dyn Fn(Sign) -> bool> = match change {
        Change::Always { truth, .. } => {
            let truth = *truth;
            Box::new(move |_| truth)
        }
        Change::Returns { value } if matches!(value.as_str(), "true" | "false") => {
            let truth = value == "true";
            Box::new(move |_| truth)
        }
        Change::Boundary { after, .. }
        | Change::Reversed { after, .. }
        | Change::Compared { after, .. }
        | Change::Swapped { after, .. } => {
            let after = facing(after);
            holds(&after, Sign::Zero)?;
            Box::new(move |sign| holds(&after, sign).unwrap_or(false))
        }
        _ => return None,
    };
    let function = expr.syntax().ancestors().find_map(ast::Fn::cast)?;
    if !untouched_by_macros(function.syntax()) {
        return None;
    }
    let why = unsigned(&function, &value)?;
    let agrees = [Sign::Zero, Sign::Positive]
        .iter()
        .all(|&sign| holds(&before, sign) == Some(after(sign)));
    agrees.then(|| {
        format!(
            "equivalent: differs only when {} is below 0. That can't happen because {why}.",
            plain(value.syntax())
        )
    })
}

/// Why `value` can't be below 0, in words, when the source proves it.
fn unsigned(function: &ast::Fn, value: &ast::Expr) -> Option<String> {
    match value {
        ast::Expr::ParenExpr(inner) => unsigned(function, &inner.expr()?),
        ast::Expr::PathExpr(_) => {
            let name = simple_name(value)?;
            if let Some(ty) = binding(function, &name, value.syntax()) {
                let ty = unsigned_type(&ty, value.syntax())?;
                return Some(format!("`{name}` is a `{ty}`"));
            }
            enumerated(function, &name, value.syntax())
                .then(|| format!("`{name}` is an index `.enumerate()` counts from 0"))
        }
        ast::Expr::FieldExpr(field) => {
            let (ty, at) = field_type(function, field)?;
            let ty = unsigned_type(&ty, &at)?;
            Some(format!("`{}` is a `{ty}` field", plain(field.syntax())))
        }
        ast::Expr::CastExpr(cast) => {
            let ty = unsigned_type(&cast.ty()?, cast.syntax())?;
            Some(format!("it's cast to `{ty}`"))
        }
        ast::Expr::CallExpr(call) => {
            let ty = returned(call)?;
            Some(format!(
                "`{}` returns a `{ty}`",
                plain(call.expr()?.syntax())
            ))
        }
        ast::Expr::BinExpr(arithmetic) => {
            let Some(BinaryOp::ArithOp(op)) = arithmetic.op_kind() else {
                return None;
            };
            use ra_ap_syntax::ast::ArithOp::{Add, Div, Mul, Rem, Sub};
            if !matches!(op, Add | Sub | Mul | Div | Rem) {
                return None;
            }
            let (lhs, rhs) = (arithmetic.lhs()?, arithmetic.rhs()?);
            let left = operand(function, &lhs)?;
            let right = operand(function, &rhs)?;
            let proof = left.or(right)?;
            Some(format!(
                "{proof}, and arithmetic on it stays unsigned, or panics"
            ))
        }
        _ => None,
    }
}

/// An operand of unsigned arithmetic: proved unsigned (with why), or a plain whole literal, which
/// takes the other side's type (none for why). None for anything else.
fn operand(function: &ast::Fn, expr: &ast::Expr) -> Option<Option<String>> {
    if let ast::Expr::Literal(_) = expr {
        let text = plain(expr.syntax());
        let (value, suffix) = integer_literal(&text)?;
        return (value >= 0 && (suffix.is_empty() || UNSIGNED.contains(&suffix))).then_some(None);
    }
    unsigned(function, expr).map(Some)
}

/// The unsigned type `ty` is, as written at `at`: `u32`, or `&usize`; none when a generic or an
/// item there takes the name.
fn unsigned_type(ty: &ast::Type, at: &SyntaxNode) -> Option<String> {
    let ty = unreferenced(ty.clone())?;
    let name = plain(ty.syntax());
    (UNSIGNED.contains(&name.as_str()) && !generic_named(at, &name) && !declared_around(&name, at))
        .then_some(name)
}

/// Whether an item around `at` is called `name`, so a type written that way could be its.
fn declared_around(name: &str, at: &SyntaxNode) -> bool {
    std::iter::successors(scope_of(at), scope_of).any(|items| {
        items.children().filter_map(ast::Item::cast).any(|item| {
            !matches!(item, ast::Item::Impl(_))
                && item
                    .syntax()
                    .children()
                    .filter_map(ast::Name::cast)
                    .any(|n| n.text() == name)
        })
    })
}

/// The unsigned type a call returns, when it calls by one plain name a function declared beside
/// it, once, with that return type written and no generics: `count(xs)` of `fn count(…) -> usize`.
fn returned(call: &ast::CallExpr) -> Option<String> {
    let name = simple_name(&call.expr()?)?;
    let scope = scope_of(call.syntax().ancestors().find_map(ast::Fn::cast)?.syntax())?;
    let mut functions = scope
        .children()
        .filter_map(ast::Fn::cast)
        .filter(|f| f.name().is_some_and(|n| n.text() == name));
    let declared = functions.next()?;
    if functions.next().is_some()
        || declared.generic_param_list().is_some()
        || !untouched_by_macros(declared.syntax())
    {
        return None;
    }
    unsigned_type(&declared.ret_type()?.ty()?, declared.syntax())
}

/// Whether `name` is bound, once in the function, as the first of a pair a `for` loop takes from a
/// call of `.enumerate()` with nothing given it, and `used` is in that loop's body.
fn enumerated(function: &ast::Fn, name: &str, used: &SyntaxNode) -> bool {
    let mut bindings = function
        .syntax()
        .descendants()
        .filter_map(ast::IdentPat::cast)
        .filter(|pat| pat.name().is_some_and(|n| n.text() == name));
    let (Some(pat), None) = (bindings.next(), bindings.next()) else {
        return false;
    };
    let Some(pair) = pat.syntax().parent().and_then(ast::TuplePat::cast) else {
        return false;
    };
    let first = pair
        .fields()
        .next()
        .is_some_and(|f| f.syntax() == pat.syntax());
    let Some(for_loop) = pair.syntax().parent().and_then(ast::ForExpr::cast) else {
        return false;
    };
    let from_enumerate = matches!(
        for_loop.iterable(),
        Some(ast::Expr::MethodCallExpr(call))
            if call.name_ref().is_some_and(|n| n.text() == "enumerate")
                && call.arg_list().is_some_and(|args| args.args().next().is_none())
    );
    let in_body = for_loop
        .loop_body()
        .is_some_and(|body| body.syntax().text_range().contains_range(used.text_range()));
    first && from_enumerate && in_body
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each function's skipped mutants, by replacement.
    fn skipped(source: &str) -> BTreeMap<String, Vec<String>> {
        let dir = std::env::temp_dir().join(format!("mutastic-unsigned-{}", std::process::id()));
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
    fn a_zero_check_on_an_unsigned_value_is_skipped_where_the_source_proves_it() {
        let source = "pub struct Stock { count: u32 }\n\
             fn total(xs: &[u32]) -> usize { xs.len() }\n\
             pub fn param(n: usize) -> bool { n == 0 }\n\
             pub fn typed_let(x: i64) -> bool { let n: u64 = x.unsigned_abs(); n > 0 }\n\
             pub fn cast(x: i32) -> bool { (x as u8) == 0 }\n\
             pub fn field(s: &Stock) -> bool { s.count == 0 }\n\
             pub fn called(xs: &[u32]) -> bool { total(xs) == 0 }\n\
             pub fn sum(a: u32, b: u32) -> bool { a + b * 2 == 0 }\n\
             pub fn indexed(xs: &[i32]) -> i32 { let mut t = 0; for (i, x) in xs.iter().enumerate() { if i == 0 { t += x; } } t }\n\
             pub fn signed(n: i32) -> bool { n == 0 }\n\
             pub fn inferred(x: i32) -> bool { let n = x as u32; n == 0 }\n\
             pub fn generic<usize: Copy>(n: usize) -> bool { n == 0 }\n\
             pub fn own_count(s: &Shelf) -> bool { s.total() == 0 }\n\
             pub fn minus(a: u32, b: i32) -> bool { a as i32 - b == 0 }\n\
             mod shadow { pub struct u32; pub fn f(n: u32) -> bool { n == 0 } }\n";
        let skipped = skipped(source);
        for proved in [
            "param",
            "typed_let",
            "cast",
            "field",
            "called",
            "sum",
            "indexed",
        ] {
            assert!(skipped.contains_key(proved), "{proved}: {skipped:?}");
        }
        for kept in [
            "signed",
            "inferred",
            "generic",
            "own_count",
            "minus",
            "shadow::f",
        ] {
            assert_eq!(skipped.get(kept), None, "{kept}: {skipped:?}");
        }
        // `n == 0`: made `<= 0` and `>= 0`'s twin are equivalent; `!= 0`, `< 0` and forced false
        // differ for a positive or a zero, and run.
        let param = &skipped["param"];
        assert!(param.iter().any(|r| r.contains("<=")), "{param:?}");
        assert!(!param.iter().any(|r| r == "false"), "{param:?}");
    }
}
