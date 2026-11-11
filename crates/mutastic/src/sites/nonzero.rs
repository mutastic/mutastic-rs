//! A comparison of a `NonZero*` value with 0: `parts.get() == 0` where `parts` is a `NonZeroU32`.
//! The value can't be 0, so a mutant of the comparison that differs from it only when the value is
//! 0 is equivalent, and is skipped.
//!
//! The rule acts only on what the source proves. The value is read with `.get()` from a name or a
//! field whose declared type is visibly std's (`NonZeroU32`, `NonZero<u32>`, imported from
//! `std::num` or `core::num` in a scope the code is in): a parameter, a `let` with its type
//! written, or a field of a struct declared beside the code, with no other binding of that name in
//! the function and no macro that could make one. An inferred or generic type, a type from
//! somewhere else, or a name a macro is given, and the mutants run.

use super::*;
use ra_ap_syntax::ast::{HasArgList, HasGenericArgs, HasGenericParams};

/// Where a whole number stands against 0: all a comparison with 0 can tell.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sign {
    Negative,
    Zero,
    Positive,
}

/// What the source proves about the value a comparison reads.
struct NonZero {
    /// `parts`, or `self.parts`: what `.get()` is called on.
    holder: String,
    /// `NonZeroU32`, or `NonZero<u32>`, as written.
    ty: String,
    signed: bool,
}

/// Why a comparison's mutant is equivalent, when it differs from the comparison only where a
/// `NonZero*` value would be 0.
pub(super) fn never_zero(node: &SyntaxNode, change: &Change) -> Option<String> {
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
    let proof = nonzero(&value)?;
    let reachable: &[Sign] = if proof.signed {
        &[Sign::Negative, Sign::Positive]
    } else {
        &[Sign::Positive]
    };
    let agrees = reachable
        .iter()
        .all(|&sign| holds(&before, sign) == Some(after(sign)));
    agrees.then(|| {
        format!(
            "equivalent: differs only when {holder}.get() is 0. That can't happen because \
             {holder} is a {ty}, which can't hold 0.",
            holder = proof.holder,
            ty = proof.ty,
        )
    })
}

/// Whether `value op 0` holds for a value of this sign; none for an operator that isn't a
/// comparison.
fn holds(op: &str, sign: Sign) -> Option<bool> {
    Some(match op {
        "==" => sign == Sign::Zero,
        "!=" => sign != Sign::Zero,
        "<" => sign == Sign::Negative,
        "<=" => sign != Sign::Positive,
        ">" => sign == Sign::Positive,
        ">=" => sign != Sign::Negative,
        _ => return None,
    })
}

/// The operator that says the same with its sides swapped: `0 < x` is `x > 0`.
fn mirrored(op: &str) -> String {
    match op {
        "<" => ">",
        "<=" => ">=",
        ">" => "<",
        ">=" => "<=",
        other => other,
    }
    .to_string()
}

fn is_zero(expr: &ast::Expr) -> bool {
    matches!(expr, ast::Expr::Literal(_))
        && integer_literal(&plain(expr.syntax())).is_some_and(|(value, _)| value == 0)
}

/// `holder.get()`, where the holder is provably a `NonZero*`.
fn nonzero(value: &ast::Expr) -> Option<NonZero> {
    let ast::Expr::MethodCallExpr(call) = value else {
        return None;
    };
    let plain_get = call.name_ref()?.text() == "get"
        && call.generic_arg_list().is_none()
        && call.arg_list()?.args().next().is_none();
    if !plain_get {
        return None;
    }
    let holder = call.receiver()?;
    let function = call.syntax().ancestors().find_map(ast::Fn::cast)?;
    if !untouched_by_macros(function.syntax()) {
        return None;
    }
    let (ty, at) = match &holder {
        ast::Expr::PathExpr(_) => {
            let binding = binding(&function, &simple_name(&holder)?, call.syntax())?;
            let at = binding.syntax().clone();
            (binding, at)
        }
        ast::Expr::FieldExpr(field) => field_type(&function, field)?,
        _ => return None,
    };
    let (ty, signed) = nonzero_type(&ty, &at)?;
    Some(NonZero {
        holder: plain(holder.syntax()),
        ty,
        signed,
    })
}

/// `parts`: a path of one plain name, not `self`.
pub(super) fn simple_name(expr: &ast::Expr) -> Option<String> {
    let ast::Expr::PathExpr(path) = expr else {
        return None;
    };
    let path = path.path()?;
    if path.qualifier().is_some() || path.segment()?.generic_arg_list().is_some() {
        return None;
    }
    let name = path.segment()?.name_ref()?.text().to_string();
    (name != "self").then_some(name)
}

/// The declared type of the one binding of `name` in `function` that `used` can see: a parameter,
/// or a `let` before it in a block around it, each with its type written and the name bound
/// directly. None when the function binds the name anywhere else, or a macro is given it.
fn binding(function: &ast::Fn, name: &str, used: &SyntaxNode) -> Option<ast::Type> {
    let given_to_a_macro = function
        .syntax()
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::MACRO_CALL)
        .flat_map(|call| call.descendants_with_tokens())
        .filter_map(|element| element.into_token())
        .any(|token| token.kind() == SyntaxKind::IDENT && token.text() == name);
    if given_to_a_macro {
        return None;
    }
    let mut bindings = function
        .syntax()
        .descendants()
        .filter_map(ast::IdentPat::cast)
        .filter(|pat| pat.name().is_some_and(|n| n.text() == name));
    let pat = bindings.next()?;
    if bindings.next().is_some() || pat.pat().is_some() {
        return None;
    }
    let parent = pat.syntax().parent()?;
    if let Some(param) = ast::Param::cast(parent.clone()) {
        let in_this_function = param.syntax().parent()?.parent()? == *function.syntax();
        return in_this_function.then(|| param.ty()).flatten();
    }
    let stmt = ast::LetStmt::cast(parent)?;
    let block = stmt.syntax().parent()?;
    let in_scope = stmt.syntax().text_range().end() <= used.text_range().start()
        && block.text_range().contains_range(used.text_range());
    in_scope.then(|| stmt.ty()).flatten()
}

/// The declared type of `self.parts`, or of `plan.parts` where `plan` is bound as [`binding`]
/// says: a named field of a struct with no generics declared in the same scope as the function.
/// Gives the field's type and where it's written.
fn field_type(function: &ast::Fn, field: &ast::FieldExpr) -> Option<(ast::Type, SyntaxNode)> {
    let field_name = field.name_ref()?.text().to_string();
    let base = field.expr()?;
    let owner = if plain(base.syntax()) == "self" {
        let imp = function
            .syntax()
            .parent()?
            .parent()
            .and_then(ast::Impl::cast)?;
        imp.self_ty()?
    } else {
        let name = simple_name(&base)?;
        binding(function, &name, field.syntax())?
    };
    let owner = single_name(&unreferenced(owner)?)?;
    if generic_named(function.syntax(), &owner) {
        return None;
    }
    let scope = scope_of(function.syntax())?;
    let mut structs = scope
        .children()
        .filter_map(ast::Struct::cast)
        .filter(|s| s.name().is_some_and(|n| n.text() == owner));
    let declared = structs.next()?;
    if structs.next().is_some()
        || declared.generic_param_list().is_some()
        || !untouched_by_macros(declared.syntax())
    {
        return None;
    }
    let Some(ast::FieldList::RecordFieldList(fields)) = declared.field_list() else {
        return None;
    };
    let field = fields
        .fields()
        .find(|f| f.name().is_some_and(|n| n.text() == field_name))?;
    Some((field.ty()?, field.syntax().clone()))
}

/// Attributes that leave the code they're on as it's written: an attribute macro could rewrite a
/// type the rule reads.
const INERT_ATTRIBUTES: &[&str] = &[
    "doc",
    "allow",
    "warn",
    "deny",
    "forbid",
    "expect",
    "inline",
    "must_use",
    "cold",
    "track_caller",
    "deprecated",
    "cfg",
    "derive",
    "repr",
    "non_exhaustive",
    "rustfmt::skip",
];

/// Whether every item around a node, the node too, has only outer attributes that leave it as
/// written, a `cfg_attr`'s among them. An inner attribute can't be a macro on stable Rust.
pub(super) fn untouched_by_macros(node: &SyntaxNode) -> bool {
    node.ancestors()
        .filter_map(ast::AnyHasAttrs::cast)
        .flat_map(|item| item.attrs().collect::<Vec<_>>())
        .filter(|attr| attr.excl_token().is_none())
        .flat_map(|attr| attr.skip_cfg_attrs())
        .all(|meta| match meta {
            ast::Meta::CfgMeta(_) => true,
            meta => meta
                .path()
                .map(|path| plain(path.syntax()))
                .is_some_and(|path| {
                    INERT_ATTRIBUTES.contains(&path.as_str()) || path.starts_with("clippy::")
                }),
        })
}

/// The items a node sits among: its block's statements, its module's items, or the file's.
pub(super) fn scope_of(node: &SyntaxNode) -> Option<SyntaxNode> {
    node.ancestors().skip(1).find(|ancestor| {
        matches!(
            ancestor.kind(),
            SyntaxKind::STMT_LIST | SyntaxKind::ITEM_LIST | SyntaxKind::SOURCE_FILE
        )
    })
}

/// A type with its references taken off: `&mut Plan` is `Plan`.
pub(super) fn unreferenced(ty: ast::Type) -> Option<ast::Type> {
    match ty {
        ast::Type::RefType(reference) => unreferenced(reference.ty()?),
        other => Some(other),
    }
}

/// `Plan`: a path type of one segment with no generics.
pub(super) fn single_name(ty: &ast::Type) -> Option<String> {
    let ast::Type::PathType(path) = ty else {
        return None;
    };
    let text: String = plain(path.syntax())
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let plain_name = text.chars().all(|c| c.is_alphanumeric() || c == '_');
    plain_name.then_some(text)
}

/// The `NonZero*` a type written at `at` is, and whether it's signed: `NonZeroU32`,
/// `num::NonZeroI8`, `std::num::NonZeroUsize` or `NonZero<u64>`, each visibly std's.
fn nonzero_type(ty: &ast::Type, at: &SyntaxNode) -> Option<(String, bool)> {
    let ty = unreferenced(ty.clone())?;
    let ast::Type::PathType(path) = &ty else {
        return None;
    };
    let text: String = plain(path.syntax())
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let rooted = text.starts_with("::");
    let text = text.trim_start_matches("::");
    let (path_text, argument) = match text.split_once('<') {
        Some((path_text, rest)) => (path_text, Some(rest.strip_suffix('>')?)),
        None => (text, None),
    };
    let segments: Vec<&str> = path_text.split("::").collect();
    let last = *segments.last()?;
    let signed = match (last, argument) {
        ("NonZero", Some(number)) if WHOLE_NUMBERS.contains(&number) => {
            if generic_named(at, number) {
                return None;
            }
            number.starts_with('i')
        }
        (name, None) => {
            let number = name.strip_prefix("NonZero")?.to_lowercase();
            if !WHOLE_NUMBERS.contains(&number.as_str()) {
                return None;
            }
            number.starts_with('i')
        }
        _ => return None,
    };
    if generic_named(at, last) {
        return None;
    }
    let full: Vec<String> = match segments.as_slice() {
        [root, ..] if rooted || matches!(*root, "std" | "core") => {
            segments.iter().map(|s| s.to_string()).collect()
        }
        [first, rest @ ..] => {
            let mut full = imported(first, at)?;
            full.extend(rest.iter().map(|s| s.to_string()));
            full
        }
        [] => return None,
    };
    let from_std = matches!(
        full.as_slice(),
        [root, num, name] if matches!(root.as_str(), "std" | "core") && num == "num" && name == last
    );
    let display = match argument {
        Some(number) => format!("{last}<{number}>"),
        None => last.to_string(),
    };
    from_std.then_some((display, signed))
}

/// Whether a generic parameter around `at` is called `name`, so it isn't the type of that name.
pub(super) fn generic_named(at: &SyntaxNode, name: &str) -> bool {
    at.ancestors()
        .filter_map(|node| {
            ast::Fn::cast(node.clone())
                .and_then(|f| f.generic_param_list())
                .or_else(|| ast::Impl::cast(node.clone()).and_then(|i| i.generic_param_list()))
                .or_else(|| ast::Struct::cast(node).and_then(|s| s.generic_param_list()))
        })
        .flat_map(|list| list.generic_params().collect::<Vec<_>>())
        .any(|param| {
            param
                .syntax()
                .children()
                .filter_map(ast::Name::cast)
                .any(|n| n.text() == name)
        })
}

/// The std path `name` is imported as where `at` is: `NonZeroU32` from `use std::num::NonZeroU32`,
/// `num` from `use core::num`. Looks out from `at` through its blocks to its module. None when the
/// name is declared or imported from anywhere else there, or isn't seen at all.
fn imported(name: &str, at: &SyntaxNode) -> Option<Vec<String>> {
    let mut scope = scope_of(at);
    while let Some(items) = scope {
        let declared = items.children().any(|item| {
            ast::Item::cast(item.clone()).is_some_and(|item| {
                !matches!(item, ast::Item::Use(_) | ast::Item::Impl(_))
                    && item
                        .syntax()
                        .children()
                        .filter_map(ast::Name::cast)
                        .any(|n| n.text() == name)
            })
        });
        if declared {
            return None;
        }
        let leaves: Vec<Leaf> = items
            .children()
            .filter_map(ast::Use::cast)
            .filter_map(|item| item.use_tree())
            .flat_map(|tree| {
                let mut leaves = Vec::new();
                leaves_of(&tree, &[], &mut leaves);
                leaves
            })
            .collect();
        if let Some(leaf) = leaves
            .iter()
            .find(|leaf| leaf.binds.as_deref() == Some(name))
        {
            return from_std(&leaf.path).then(|| leaf.path.clone());
        }
        let globs: Vec<&Leaf> = leaves.iter().filter(|leaf| leaf.binds.is_none()).collect();
        let num_glob = globs
            .iter()
            .any(|leaf| from_std(&leaf.path) && leaf.path.len() == 2 && leaf.path[1] == "num");
        if num_glob && name.starts_with("NonZero") {
            let mut full = globs
                .iter()
                .find(|leaf| from_std(&leaf.path))
                .map(|leaf| leaf.path.clone())?;
            full.push(name.to_string());
            return Some(full);
        }
        if !globs.is_empty() || items.kind() != SyntaxKind::STMT_LIST {
            return None;
        }
        scope = scope_of(&items);
    }
    None
}

fn from_std(path: &[String]) -> bool {
    path.first()
        .is_some_and(|root| matches!(root.as_str(), "std" | "core"))
}

/// One name a `use` brings in: its path, and the name it's bound to (none for a glob).
pub(super) struct Leaf {
    pub(super) path: Vec<String>,
    pub(super) binds: Option<String>,
}

pub(super) fn leaves_of(tree: &ast::UseTree, prefix: &[String], out: &mut Vec<Leaf>) {
    let mut path = prefix.to_vec();
    if let Some(own) = tree.path() {
        let text: String = plain(own.syntax())
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        path.extend(
            text.split("::")
                .filter(|segment| !segment.is_empty())
                .map(str::to_string),
        );
    }
    if let Some(list) = tree.use_tree_list() {
        for inner in list.use_trees() {
            leaves_of(&inner, &path, out);
        }
        return;
    }
    if tree.star_token().is_some() {
        out.push(Leaf { path, binds: None });
        return;
    }
    if path.last().is_some_and(|last| last == "self") {
        path.pop();
    }
    let binds = match tree.rename() {
        Some(rename) => rename.name().map(|n| n.text().to_string()),
        None => path.last().cloned(),
    };
    if let Some(binds) = binds {
        out.push(Leaf {
            path,
            binds: Some(binds),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    /// The mutants each function's comparison has skipped as equivalent, by function.
    fn skipped(source: &str) -> BTreeMap<String, Vec<String>> {
        let dir = std::env::temp_dir().join(format!("mutastic-nonzero-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let found = find(&dir, "sample", &[PathBuf::from("src/lib.rs")], STRONGER);
        std::fs::remove_dir_all(&dir).ok();
        let mut skipped: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for mutant in &found.mutants {
            let entry = skipped
                .entry(found.functions[mutant.function].path.clone())
                .or_default();
            if mutant.skipped.is_some() {
                entry.push(mutant.replacement.clone());
            }
        }
        skipped.retain(|_, replacements| !replacements.is_empty());
        skipped
    }

    #[test]
    fn a_zero_check_on_a_nonzero_is_skipped_where_its_type_is_proved() {
        let source = "use std::num::{NonZero, NonZeroI32, NonZeroU32};\n\
             use core::num;\n\
             pub fn per_part(total: u32, parts: NonZeroU32) -> u32 {\n\
                 if parts.get() == 0 { return 0; } total / parts.get() }\n\
             pub fn flipped(n: &std::num::NonZeroU8) -> bool { 0 != n.get() }\n\
             pub fn typed_let(x: u16) -> bool { let n: NonZero<u16> = NonZero::new(x).unwrap(); n.get() > 0 }\n\
             pub fn through_num(n: num::NonZeroU64) -> bool { n.get() >= 0 }\n\
             pub fn signed(n: NonZeroI32) -> bool { n.get() < 0 }\n\
             pub struct Plan { parts: NonZeroU32 }\n\
             impl Plan { pub fn empty(&self) -> bool { self.parts.get() <= 0 } }\n\
             pub fn of_plan(plan: &Plan) -> bool { plan.parts.get() == 0 }\n\
             pub fn returned(parts: NonZeroU32) -> bool { parts.get() == 0 }\n\
             pub fn plain(parts: u32) -> bool { parts == 0 }\n\
             pub fn is_one(parts: NonZeroU32) -> bool { parts.get() == 1 }\n\
             pub fn shadowed(parts: NonZeroU32) -> bool { let parts = std::cell::Cell::new(0u32); parts.get() == 0 }\n\
             pub fn inferred() -> bool { let n = NonZeroU32::new(3).unwrap(); n.get() == 0 }\n\
             pub fn generic<NonZeroU32: Get>(parts: NonZeroU32) -> bool { parts.get() == 0 }\n\
             pub fn macro_given(parts: NonZeroU32) -> bool { rebind!(parts); parts.get() == 0 }\n\
             #[cfg_attr(feature = \"x\", rewrite)] pub fn attributed(parts: NonZeroU32) -> bool { parts.get() == 0 }\n\
             #[inline] #[cfg_attr(unix, allow(clippy::nonminimal_bool))] pub fn inert(parts: NonZeroU32) -> bool { parts.get() == 0 }\n\
             pub fn later(x: u32) -> bool { parts.get() == 0; let parts: NonZeroU32 = todo(); x == 1 }\n\
             mod own { pub struct NonZeroU32(u32);\n\
                 pub fn f(parts: NonZeroU32) -> bool { parts.get() == 0 } }\n\
             mod bare { pub fn f(parts: NonZeroU32) -> bool { parts.get() == 0 } }\n\
             mod renamed { use std::num::Wrapping as NonZeroU16;\n\
                 pub fn f(parts: NonZeroU16) -> bool { parts.get() == 0 } }\n\
             mod globbed { use std::num::*; use other::*;\n\
                 pub fn f(parts: NonZeroU8) -> bool { parts.get() == 0 } }\n\
             mod elsewhere { use other::*;\n\
                 pub fn f(parts: NonZeroU8) -> bool { parts.get() == 0 } }\n";
        let expected: BTreeMap<String, Vec<String>> = [
            (
                "per_part",
                vec!["false", "(parts.get()) <= (0)", "(parts.get()) < (0)"],
            ),
            // The `true` and `false` a returned comparison gets twice: remove-conditional's, and
            // return-value's.
            (
                "flipped",
                vec!["true", "true", "(0) < (n.get())", "(0) <= (n.get())"],
            ),
            (
                "typed_let",
                vec!["true", "(n.get()) >= (0)", "true", "(n.get()) != (0)"],
            ),
            (
                "through_num",
                vec!["true", "(n.get()) > (0)", "true", "(n.get()) != (0)"],
            ),
            (
                "returned",
                vec![
                    "false",
                    "false",
                    "(parts.get()) <= (0)",
                    "(parts.get()) < (0)",
                ],
            ),
            ("signed", vec!["(n.get()) <= (0)"]),
            (
                "Plan::empty",
                vec![
                    "false",
                    "(self.parts.get()) < (0)",
                    "false",
                    "(self.parts.get()) == (0)",
                ],
            ),
            (
                "of_plan",
                vec![
                    "false",
                    "false",
                    "(plan.parts.get()) <= (0)",
                    "(plan.parts.get()) < (0)",
                ],
            ),
            (
                "globbed::f",
                vec![
                    "false",
                    "false",
                    "(parts.get()) <= (0)",
                    "(parts.get()) < (0)",
                ],
            ),
            (
                "inert",
                vec![
                    "false",
                    "false",
                    "(parts.get()) <= (0)",
                    "(parts.get()) < (0)",
                ],
            ),
        ]
        .into_iter()
        .map(|(f, r)| (f.to_string(), r.into_iter().map(str::to_string).collect()))
        .collect();
        assert_eq!(skipped(source), expected);
    }

    #[test]
    fn the_reason_names_the_value_and_its_type() {
        let source = "use std::num::NonZeroU32;\n\
             pub fn per_part(total: u32, parts: NonZeroU32) -> u32 {\n\
                 if parts.get() == 0 { return 0; } total / parts.get() }\n";
        let dir = std::env::temp_dir().join(format!("mutastic-nonzero-r-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let found = find(&dir, "sample", &[PathBuf::from("src/lib.rs")], DEFAULTS);
        std::fs::remove_dir_all(&dir).ok();
        let reasons: BTreeSet<&str> = found
            .mutants
            .iter()
            .filter_map(|m| m.skipped.as_deref())
            .collect();
        assert_eq!(
            reasons,
            BTreeSet::from([
                "equivalent: differs only when parts.get() is 0. That can't happen because parts \
                 is a NonZeroU32, which can't hold 0."
            ])
        );
    }
}
