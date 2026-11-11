//! A comparison inside code that runs only when its two sides differ: `ver.major > cmp.major` in
//! `if ver.major != cmp.major { … }`. The sides can't be equal there, so a mutant of the comparison
//! that differs from it only when they are (`>` to `>=`) is equivalent, and is skipped.
//!
//! The rule acts only on what the source proves. The guard is a `!=` on the same two sides that
//! holds wherever the comparison runs: the `if` around it, the `else` of an `if a == b`, an
//! `if a == b` before it that returns, or the left of an `&&`. Each side is an integer literal or a
//! whole number read from a binding that can't change: a parameter, a `let` or a `Some(x)`, bound
//! once in the function, without `mut`, never given to a macro, or a field of one, of a struct
//! declared in the crate. A float, a type that's inferred or generic, a binding that's `mut` or
//! bound twice, and the mutants run.

use super::nonzero::untouched_by_macros;
use super::nonzero::{generic_named, leaves_of, scope_of, simple_name, single_name, Leaf};
use super::*;
use ra_ap_syntax::ast::{HasGenericArgs, HasGenericParams};

/// The crate's structs with named fields and no generics, by module and name: each field's
/// declared type.
pub(super) type Structs = BTreeMap<(String, String), Option<Vec<(String, ast::Type)>>>;

/// Records the structs a file declares outside any function, by the module they're in. A name
/// declared twice in a module (under two `cfg`s, say) is recorded as unknown.
pub(super) fn index(module: &str, tree: &SourceFile, structs: &mut Structs) {
    for declared in tree.syntax().descendants().filter_map(ast::Struct::cast) {
        let in_items = declared.syntax().ancestors().skip(1).all(|node| {
            matches!(
                node.kind(),
                SyntaxKind::ITEM_LIST | SyntaxKind::MODULE | SyntaxKind::SOURCE_FILE
            )
        });
        let Some(name) = declared.name() else {
            continue;
        };
        if !in_items {
            continue;
        }
        let key = (
            module_at(module, declared.syntax()),
            name.text().to_string(),
        );
        let fields = match declared.field_list() {
            Some(ast::FieldList::RecordFieldList(fields))
                if declared.generic_param_list().is_none()
                    && untouched_by_macros(declared.syntax()) =>
            {
                fields
                    .fields()
                    .map(|field| Some((field.name()?.text().to_string(), field.ty()?)))
                    .collect::<Option<Vec<_>>>()
            }
            _ => None,
        };
        structs
            .entry(key)
            .and_modify(|known| *known = None)
            .or_insert(fields);
    }
}

/// Where one side of a comparison stands against the other, once they're known to differ.
#[derive(Clone, Copy)]
enum Side {
    Less,
    Greater,
}

/// Whether `a op b` holds where `a` is on this side of `b`; none for an operator that isn't a
/// comparison.
fn holds(op: &str, side: Side) -> Option<bool> {
    let less = matches!(side, Side::Less);
    Some(match op {
        "==" => false,
        "!=" => true,
        "<" | "<=" => less,
        ">" | ">=" => !less,
        _ => return None,
    })
}

/// Why a comparison's mutant is equivalent, when it differs from the comparison only where its two
/// sides are equal, and a guard around it proves they aren't.
pub(super) fn never_equal(
    node: &SyntaxNode,
    change: &Change,
    module: &str,
    structs: &Structs,
) -> Option<String> {
    let expr = ast::BinExpr::cast(node.clone())?;
    let Some(BinaryOp::CmpOp(_)) = expr.op_kind() else {
        return None;
    };
    let before = expr.op_token()?.text().to_string();
    let after: Box<dyn Fn(Side) -> bool> = match change {
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
            let after = after.clone();
            holds(&after, Side::Less)?;
            Box::new(move |side| holds(&after, side).unwrap_or(false))
        }
        _ => return None,
    };
    let agrees = [Side::Less, Side::Greater]
        .into_iter()
        .all(|side| holds(&before, side) == Some(after(side)));
    if !agrees {
        return None;
    }
    let (lhs, rhs) = (unparenthesized(expr.lhs()?), unparenthesized(expr.rhs()?));
    let sides = [plain(lhs.syntax()), plain(rhs.syntax())];
    let guard = guard(&expr, &sides)?;
    let function = expr.syntax().ancestors().find_map(ast::Fn::cast)?;
    if !untouched_by_macros(function.syntax()) {
        return None;
    }
    let proved = Proof {
        function: &function,
        module,
        structs,
    };
    let literals = [&lhs, &rhs]
        .into_iter()
        .filter(|side| is_integer(side))
        .count();
    let all_proved = [&lhs, &rhs]
        .into_iter()
        .all(|side| is_integer(side) || proved.whole_number(side));
    (literals < 2 && all_proved).then(|| {
        format!(
            "equivalent: differs only when {} is {}. That can't happen because {guard} rules it out.",
            sides[0], sides[1]
        )
    })
}

fn unparenthesized(expr: ast::Expr) -> ast::Expr {
    match &expr {
        ast::Expr::ParenExpr(paren) => match paren.expr() {
            Some(inner) => unparenthesized(inner),
            None => expr,
        },
        _ => expr,
    }
}

fn is_integer(expr: &ast::Expr) -> bool {
    matches!(expr, ast::Expr::Literal(_)) && integer_literal(&plain(expr.syntax())).is_some()
}

/// What makes the comparison's sides differ wherever it runs, in words: `the if a != b around
/// it`. Looks out from the comparison to its function.
fn guard(compared: &ast::BinExpr, sides: &[String; 2]) -> Option<String> {
    let mut child = compared.syntax().clone();
    for parent in compared.syntax().ancestors().skip(1) {
        if ast::Fn::can_cast(parent.kind()) {
            return None;
        }
        if let Some(found) = guard_at(&parent, &child, sides) {
            return Some(found);
        }
        child = parent;
    }
    None
}

/// The guard `parent` makes for `child`, its child, if any.
fn guard_at(parent: &SyntaxNode, child: &SyntaxNode, sides: &[String; 2]) -> Option<String> {
    if let Some(both) = ast::BinExpr::cast(parent.clone()) {
        let left = both.lhs()?;
        let entered_when = match both.op_kind()? {
            BinaryOp::LogicOp(LogicOp::And) => true,
            BinaryOp::LogicOp(LogicOp::Or) => false,
            _ => return None,
        };
        if both.rhs()?.syntax() != child {
            return None;
        }
        let differ = implied(&left, entered_when, sides)?;
        return Some(format!("the {} before it", plain(differ.syntax())));
    }
    if let Some(branch) = ast::IfExpr::cast(parent.clone()) {
        let condition = branch.condition()?;
        let in_then = branch.then_branch()?.syntax() == child;
        let in_else = match branch.else_branch() {
            Some(ast::ElseBranch::Block(block)) => block.syntax() == child,
            Some(ast::ElseBranch::IfExpr(elif)) => elif.syntax() == child,
            None => false,
        };
        if !(in_then || in_else) {
            return None;
        }
        let differ = implied(&condition, in_then, sides)?;
        return Some(described(&condition, &differ, "around"));
    }
    let statements = ast::StmtList::cast(parent.clone())?;
    statements
        .statements()
        .take_while(|statement| statement.syntax() != child)
        .filter_map(|statement| match statement {
            ast::Stmt::ExprStmt(statement) => match statement.expr()? {
                ast::Expr::IfExpr(branch) => Some(branch),
                _ => None,
            },
            _ => None,
        })
        .find_map(|branch| {
            let condition = branch.condition()?;
            let then_leaves = branch.then_branch().is_some_and(|block| leaves(&block));
            let else_leaves = match branch.else_branch() {
                Some(ast::ElseBranch::Block(block)) => leaves(&block),
                _ => false,
            };
            let differ = if then_leaves {
                implied(&condition, false, sides)
            } else if else_leaves {
                implied(&condition, true, sides)
            } else {
                None
            }?;
            Some(described(&condition, &differ, "before"))
        })
}

/// `the if a != b around it`, or `the a != b in the if around it` when the `if` tests more.
fn described(condition: &ast::Expr, differ: &ast::BinExpr, place: &str) -> String {
    let differ = plain(differ.syntax());
    if plain(unparenthesized(condition.clone()).syntax()) == differ {
        format!("the if {differ} {place} it")
    } else {
        format!("the {differ} in the if {place} it")
    }
}

/// Whether control never reaches the end of `block`: its last step returns, breaks, continues
/// or panics.
fn leaves(block: &ast::BlockExpr) -> bool {
    let Some(list) = block.stmt_list() else {
        return false;
    };
    let last = list
        .tail_expr()
        .or_else(|| match list.statements().last()? {
            ast::Stmt::ExprStmt(statement) => statement.expr(),
            _ => None,
        });
    match last {
        Some(ast::Expr::ReturnExpr(_) | ast::Expr::BreakExpr(_) | ast::Expr::ContinueExpr(_)) => {
            true
        }
        Some(ast::Expr::MacroExpr(call)) => diverges(&call),
        _ => false,
    }
}

/// The comparison of the two sides with `!=` (when `condition` is `holding`) or `==` (when it
/// isn't) that `condition` being `holding` proves: `a != b && c` holding proves `a != b`.
fn implied(condition: &ast::Expr, holding: bool, sides: &[String; 2]) -> Option<ast::BinExpr> {
    match unparenthesized(condition.clone()) {
        ast::Expr::PrefixExpr(not) if not.op_kind() == Some(ast::UnaryOp::Not) => {
            implied(&not.expr()?, !holding, sides)
        }
        ast::Expr::BinExpr(both) => {
            match both.op_kind()? {
                BinaryOp::LogicOp(LogicOp::And) if holding => implied(&both.lhs()?, true, sides)
                    .or_else(|| implied(&both.rhs()?, true, sides)),
                BinaryOp::LogicOp(LogicOp::Or) if !holding => implied(&both.lhs()?, false, sides)
                    .or_else(|| implied(&both.rhs()?, false, sides)),
                BinaryOp::CmpOp(CmpOp::Eq { negated }) if negated == holding => {
                    let tested = [
                        plain(unparenthesized(both.lhs()?).syntax()),
                        plain(unparenthesized(both.rhs()?).syntax()),
                    ];
                    let same = tested == *sides || (tested[0] == sides[1] && tested[1] == sides[0]);
                    same.then_some(both)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// What the source says about the values a function reads.
struct Proof<'a> {
    function: &'a ast::Fn,
    module: &'a str,
    structs: &'a Structs,
}

/// A type as written, where it's written, and the module that is.
struct Typed {
    ty: ast::Type,
    at: SyntaxNode,
    module: String,
}

impl Proof<'_> {
    /// Whether `value` is a whole number that can't change while the function runs.
    fn whole_number(&self, value: &ast::Expr) -> bool {
        self.typed(value, 4)
            .and_then(|typed| {
                let name = single_name(&shared(typed.ty)?)?;
                (WHOLE_NUMBERS.contains(&name.as_str()) && !generic_named(&typed.at, &name))
                    .then_some(())
            })
            .is_some()
    }

    /// The declared type of `value`: a binding that can't change, `self`, or a field of either.
    /// `depth` bounds how many `let`s it follows.
    fn typed(&self, value: &ast::Expr, depth: u8) -> Option<Typed> {
        match unparenthesized(value.clone()) {
            ast::Expr::FieldExpr(field) => {
                let owner = self.typed(&field.expr()?, depth)?;
                let name = single_name(&shared(owner.ty)?)?;
                let (module, fields) = self.declared(&name, &owner.at, &owner.module)?;
                let wanted = field.name_ref()?.text().to_string();
                let (_, ty) = fields.iter().find(|(name, _)| *name == wanted)?;
                Some(Typed {
                    ty: ty.clone(),
                    at: ty.syntax().clone(),
                    module,
                })
            }
            ast::Expr::PathExpr(path) if plain(path.syntax()) == "self" => {
                let receiver = self.function.param_list()?.self_param()?;
                if receiver.mut_token().is_some() || receiver.ty().is_some() {
                    return None;
                }
                let imp = self
                    .function
                    .syntax()
                    .parent()?
                    .parent()
                    .and_then(ast::Impl::cast)?;
                if imp.generic_param_list().is_some() {
                    return None;
                }
                Some(Typed {
                    ty: imp.self_ty()?,
                    at: imp.syntax().clone(),
                    module: module_at(self.module, imp.syntax()),
                })
            }
            path @ ast::Expr::PathExpr(_) => {
                let name = simple_name(&path)?;
                self.bound(&name, path.syntax(), depth)
            }
            _ => None,
        }
    }

    /// The type of the one binding of `name` in the function, which `used` reads: a parameter, a
    /// `let`, or the `x` of a `Some(x)`, without `mut` or `ref`, and never given to a macro.
    fn bound(&self, name: &str, used: &SyntaxNode, depth: u8) -> Option<Typed> {
        let function = self.function.syntax();
        let given_to_a_macro = function
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::MACRO_CALL)
            .flat_map(|call| call.descendants_with_tokens())
            .filter_map(|element| element.into_token())
            .any(|token| token.kind() == SyntaxKind::IDENT && token.text() == name);
        if given_to_a_macro || depth == 0 {
            return None;
        }
        let mut bindings = function
            .descendants()
            .filter_map(ast::IdentPat::cast)
            .filter(|pat| pat.name().is_some_and(|n| n.text() == name));
        let pat = bindings.next()?;
        if bindings.next().is_some()
            || pat.pat().is_some()
            || pat.mut_token().is_some()
            || pat.ref_token().is_some()
        {
            return None;
        }
        let parent = pat.syntax().parent()?;
        let here = |ty: ast::Type| Typed {
            at: ty.syntax().clone(),
            module: module_at(self.module, ty.syntax()),
            ty,
        };
        if let Some(param) = ast::Param::cast(parent.clone()) {
            let in_this_function = param.syntax().parent()?.parent()? == *function;
            return in_this_function.then(|| param.ty()).flatten().map(here);
        }
        if let Some(stmt) = ast::LetStmt::cast(parent.clone()) {
            if !after_in_block(stmt.syntax(), used) || stmt.let_else().is_some() {
                return None;
            }
            return match stmt.ty() {
                Some(ty) => Some(here(ty)),
                None => self.typed(&stmt.initializer()?, depth - 1),
            };
        }
        let some = ast::TupleStructPat::cast(parent)?;
        let only_field = some.fields().count() == 1;
        if plain(some.path()?.syntax()) != "Some" || !only_field {
            return None;
        }
        let holder = some.syntax().parent()?;
        let (scrutinee, scope) = if let Some(arm) = ast::MatchArm::cast(holder.clone()) {
            let matched = arm
                .syntax()
                .parent()?
                .parent()
                .and_then(ast::MatchExpr::cast)?;
            (matched.expr()?, arm.expr()?.syntax().clone())
        } else if let Some(test) = ast::LetExpr::cast(holder.clone()) {
            let branch = test.syntax().parent().and_then(ast::IfExpr::cast)?;
            (test.expr()?, branch.then_branch()?.syntax().clone())
        } else {
            let stmt = ast::LetStmt::cast(holder)?;
            if !after_in_block(stmt.syntax(), used) {
                return None;
            }
            (stmt.initializer()?, used.clone())
        };
        if !scope.text_range().contains_range(used.text_range()) {
            return None;
        }
        let option = self.typed(&scrutinee, depth - 1)?;
        let held = optional(&shared(option.ty)?)?;
        Some(Typed {
            at: held.syntax().clone(),
            ty: held,
            module: option.module,
        })
    }

    /// The fields of the struct `name` names where `at` is, in `module`, and the module it's
    /// declared in: declared there, or imported from the crate by a `use`.
    fn declared(
        &self,
        name: &str,
        at: &SyntaxNode,
        module: &str,
    ) -> Option<(String, &Vec<(String, ast::Type)>)> {
        if generic_named(at, name) {
            return None;
        }
        let mut scope = scope_of(at);
        while let Some(items) = scope {
            let declared: Vec<ast::Item> = items
                .children()
                .filter_map(ast::Item::cast)
                .filter(|item| {
                    !matches!(item, ast::Item::Use(_) | ast::Item::Impl(_))
                        && item
                            .syntax()
                            .children()
                            .filter_map(ast::Name::cast)
                            .any(|n| n.text() == name)
                })
                .collect();
            let at_module_level = items.kind() != SyntaxKind::STMT_LIST;
            if !declared.is_empty() {
                let one_struct = declared.len() == 1 && matches!(declared[0], ast::Item::Struct(_));
                if !(one_struct && at_module_level) {
                    return None;
                }
                let fields = self.structs.get(&(module.to_string(), name.to_string()))?;
                return Some((module.to_string(), fields.as_ref()?));
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
                let (original, path) = leaf.path.split_last()?;
                let from = in_crate(path, module)?;
                let fields = self
                    .structs
                    .get(&(from.clone(), original.clone()))?
                    .as_ref()?;
                return Some((from, fields));
            }
            if leaves.iter().any(|leaf| leaf.binds.is_none()) || at_module_level {
                return None;
            }
            scope = scope_of(&items);
        }
        None
    }
}

/// The module a path in a `use` names, from the module it's written in: `crate::a`, `super`, or
/// `self::b`. None for anything outside the crate.
fn in_crate(path: &[String], here: &str) -> Option<String> {
    let mut module: Vec<&str> = here.split("::").filter(|part| !part.is_empty()).collect();
    let mut rest = path;
    match rest.first()?.as_str() {
        "crate" => {
            module.clear();
            rest = &rest[1..];
        }
        "self" => rest = &rest[1..],
        "super" => {
            while rest.first().is_some_and(|part| part == "super") {
                module.pop()?;
                rest = &rest[1..];
            }
        }
        _ => return None,
    }
    module.extend(rest.iter().map(String::as_str));
    Some(module.join("::"))
}

/// A type with its shared references taken off; none behind a `&mut`, whose value can change.
fn shared(ty: ast::Type) -> Option<ast::Type> {
    match ty {
        ast::Type::RefType(reference) if reference.mut_token().is_none() => shared(reference.ty()?),
        ast::Type::RefType(_) => None,
        other => Some(other),
    }
}

/// `u64`, from `Option<u64>`.
fn optional(ty: &ast::Type) -> Option<ast::Type> {
    let ast::Type::PathType(path) = ty else {
        return None;
    };
    let path = path.path()?;
    let segment = path.segment()?;
    let named = matches!(
        plain(path.syntax()).split('<').next()?.trim(),
        "Option" | "std::option::Option" | "core::option::Option"
    );
    let mut arguments = segment.generic_arg_list()?.generic_args();
    let (Some(ast::GenericArg::TypeArg(held)), None) = (arguments.next(), arguments.next()) else {
        return None;
    };
    named.then(|| held.ty()).flatten()
}

/// Whether `stmt` comes before `used` in a block that holds both.
fn after_in_block(stmt: &SyntaxNode, used: &SyntaxNode) -> bool {
    stmt.parent().is_some_and(|block| {
        stmt.text_range().end() <= used.text_range().start()
            && block.text_range().contains_range(used.text_range())
    })
}

/// The module `node` is in: its file's, then any `mod name { … }` around it.
fn module_at(file_module: &str, node: &SyntaxNode) -> String {
    let mut inline: Vec<String> = node
        .ancestors()
        .filter_map(ast::Module::cast)
        .filter_map(|m| m.name().map(|n| n.text().to_string()))
        .collect();
    inline.reverse();
    file_module
        .split("::")
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .chain(inline)
        .collect::<Vec<_>>()
        .join("::")
}

#[cfg(test)]
mod tests {
    use super::super::*;

    /// The mutants each function's comparisons have skipped as equivalent, by function, with
    /// their reasons.
    fn skipped(files: &[(&str, &str)]) -> BTreeMap<String, Vec<String>> {
        static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("mutastic-guarded-{}-{run}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        for (path, source) in files {
            std::fs::write(dir.join(path), source).unwrap();
        }
        let found = find(&dir, "sample", &[PathBuf::from("src/lib.rs")], STRONGER);
        std::fs::remove_dir_all(&dir).ok();
        let mut skipped: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for mutant in &found.mutants {
            if let Some(reason) = &mutant.skipped {
                skipped
                    .entry(found.functions[mutant.function].path.clone())
                    .or_default()
                    .push(format!("{} | {reason}", mutant.replacement));
            }
        }
        skipped
    }

    #[test]
    fn a_comparison_inside_a_guard_that_its_sides_differ_is_skipped_where_proved() {
        let source = "pub struct Version { major: u64, minor: u64 }\n\
             pub struct Req { major: u64, minor: Option<u64> }\n\
             pub fn greater(cmp: &Req, ver: &Version) -> bool {\n\
                 if ver.major != cmp.major { return ver.major > cmp.major; }\n\
                 match cmp.minor { None => return false, Some(minor) => {\n\
                     if ver.minor != minor { return ver.minor > minor; } } }\n\
                 false }\n\
             pub fn sign(a: i32, b: i32) -> i32 { if a == b { 0 } else if a < b { -1 } else { 1 } }\n\
             pub fn early(a: u8, b: u8) -> u8 { if a == b { return 0; } if b >= a { 1 } else { 2 } }\n\
             pub fn both(a: u32, b: u32) -> bool { a != b && a < b }\n\
             pub fn typed_let(x: u32) -> u32 { let lo: u32 = x / 2; let hi = x; if lo == hi { 1 } else if lo <= hi { 2 } else { 3 } }\n\
             pub fn literal(n: i64) -> i64 { if n != 0 { if n > 0 { 1 } else { 2 } } else { 3 } }\n\
             pub fn equal_inside(a: u16, b: u16) -> u16 { if a != b { if a == b { 1 } else { 2 } } else { 3 } }\n\
             impl Version { pub fn below(&self, other: &Version) -> u8 {\n\
                 if self.major != other.major { if self.major < other.major { 1 } else { 2 } } else { 3 } } }\n\
             pub fn reassigned(a: u32, b: u32) -> u32 { let mut c = a; if c != b { c = b; if c < b { return 1; } } 2 }\n\
             pub fn other_value(a: u32, b: u32, c: u32) -> u32 { if a != b { if a < c { 1 } else { 2 } } else { 3 } }\n\
             pub fn other_guard(a: u32, b: u32, c: u32) -> u32 { if a != c { if a < b { 1 } else { 2 } } else { 3 } }\n\
             pub fn not_dominating(a: u32, b: u32, flag: bool) -> u32 { if a != b || flag { if a < b { 1 } else { 2 } } else { 3 } }\n\
             pub fn falls_through(a: u32, b: u32, n: u32) -> u32 { let mut m = n; if a == b { m += 1; } if a < b { m } else { 2 } }\n\
             pub fn float(a: f64, b: f64) -> u32 { if a != b { if a < b { 1 } else { 2 } } else { 3 } }\n\
             pub fn inferred(v: &[u32]) -> u32 { let a = v[0]; let b = v[1]; if a != b { if a < b { 1 } else { 2 } } else { 3 } }\n\
             pub fn shadowed(a: u32, b: u32) -> u32 { if a != b { let a = b; if a < b { 1 } else { 2 } } else { 3 } }\n\
             pub fn mutable(a: &mut u32, b: &mut u32) -> u32 { if a != b { if a < b { 1 } else { 2 } } else { 3 } }\n\
             pub fn macro_given(a: u32, b: u32) -> u32 { if a != b { touch!(a); if a < b { 1 } else { 2 } } else { 3 } }\n\
             pub fn generic<T: Ord>(a: T, b: T) -> u32 { if a != b { if a < b { 1 } else { 2 } } else { 3 } }\n\
             pub fn guard_after(a: u32, b: u32) -> u32 { if a < b && a != b { 1 } else { 2 } }\n\
             pub struct Wrapped<T> { major: T }\n\
             pub fn generic_field(a: &Wrapped<u32>, b: &Wrapped<u32>) -> u32 { if a.major != b.major { if a.major < b.major { 1 } else { 2 } } else { 3 } }\n";
        let expected: BTreeMap<String, Vec<String>> = [
            (
                "greater",
                vec![
                    "(ver.major) >= (cmp.major) | equivalent: differs only when ver.major is \
                     cmp.major. That can't happen because the if ver.major != cmp.major around \
                     it rules it out.",
                    "(ver.minor) >= (minor) | equivalent: differs only when ver.minor is minor. \
                     That can't happen because the if ver.minor != minor around it rules it out.",
                ],
            ),
            (
                "sign",
                vec![
                    "(a) <= (b) | equivalent: differs only when a is b. That can't happen \
                     because the if a == b around it rules it out.",
                ],
            ),
            (
                "early",
                vec![
                    "(b) > (a) | equivalent: differs only when b is a. That can't happen because \
                     the if a == b before it rules it out.",
                ],
            ),
            (
                "both",
                vec![
                    "(a) <= (b) | equivalent: differs only when a is b. That can't happen \
                     because the a != b before it rules it out.",
                ],
            ),
            (
                "typed_let",
                vec![
                    "(lo) < (hi) | equivalent: differs only when lo is hi. That can't happen \
                     because the if lo == hi around it rules it out.",
                ],
            ),
            (
                "literal",
                vec![
                    "(n) >= (0) | equivalent: differs only when n is 0. That can't happen \
                     because the if n != 0 around it rules it out.",
                ],
            ),
            (
                "equal_inside",
                vec![
                    "false | equivalent: differs only when a is b. That can't happen because \
                     the if a != b around it rules it out.",
                ],
            ),
            (
                "Version::below",
                vec![
                    "(self.major) <= (other.major) | equivalent: differs only when self.major \
                     is other.major. That can't happen because the if self.major != \
                     other.major around it rules it out.",
                ],
            ),
        ]
        .into_iter()
        .map(|(f, r)| (f.to_string(), r.into_iter().map(str::to_string).collect()))
        .collect();
        assert_eq!(skipped(&[("src/lib.rs", source)]), expected);
    }

    #[test]
    fn a_field_s_type_is_read_from_the_struct_a_use_names() {
        let root = "mod eval;\nmod float;\nmod globbed;\n\
             pub struct Version { pub major: u64 }\n\
             pub mod inner { pub struct Version { pub major: f64 } }\n";
        let compare = "pub fn newer(a: &Version, b: &Version) -> bool {\n\
                 if a.major != b.major { return a.major > b.major; } false }\n";
        let skipped = skipped(&[
            ("src/lib.rs", root),
            (
                "src/eval.rs",
                &format!("use crate::{{Version}};\n{compare}"),
            ),
            (
                "src/float.rs",
                &format!("use super::inner::Version;\n{compare}"),
            ),
            ("src/globbed.rs", &format!("use crate::*;\n{compare}")),
        ]);
        let skipped_in: Vec<&String> = skipped.keys().collect();
        assert_eq!(skipped_in, ["eval::newer"]);
    }
}
