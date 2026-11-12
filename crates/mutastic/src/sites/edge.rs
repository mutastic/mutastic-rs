//! A boundary where both sides give the same value at the edge, as the Java edition skips it: the
//! `if` the comparison decides does nothing different there, as its two sides are equal.
//!
//! ```text
//! if x > max { max = x }        // at x == max, storing x leaves max as it was
//! if a < b { a } else { b }     // at a == b, either is the same
//! if fee < 10 { 10 } else { fee } // at fee == 10, both are 10
//! ```
//!
//! The rule acts only on what the source proves. The comparison is the `if`'s whole condition, and
//! its branch is one thing: an assignment of one side to the other, with no `else`, or one side in
//! each branch. Each side is an integer literal or a whole number of a type the source declares, so
//! never a float, whose `-0.0` equals `0.0` but isn't the same value. Nothing runs between the
//! comparison and the branch, so a `mut` binding is as good as any.

use super::guarded::{is_integer, unparenthesized, Proof, Structs};
use super::nonzero::untouched_by_macros;
use super::*;

const REASON: &str = "equivalent: both sides give the same value at the edge";

/// Why the boundary `change` makes to the comparison at `node` is equivalent, if it is.
pub(super) fn same_at_edge(
    node: &SyntaxNode,
    change: &Change,
    module: &str,
    structs: &Structs,
) -> Option<String> {
    if !matches!(change, Change::Boundary { .. }) {
        return None;
    }
    let expr = ast::BinExpr::cast(node.clone())?;
    let Some(BinaryOp::CmpOp(CmpOp::Ord { .. })) = expr.op_kind() else {
        return None;
    };
    let (lhs, rhs) = (unparenthesized(expr.lhs()?), unparenthesized(expr.rhs()?));
    let sides = [plain(lhs.syntax()), plain(rhs.syntax())];
    let branch = decided_by(&expr)?;
    let then = only_expression(&branch.then_branch()?)?;
    let same = match branch.else_branch() {
        None => assigns_one_to_the_other(&then, &sides),
        Some(ast::ElseBranch::Block(other)) => {
            picks_a_side(&then, &only_expression(&other)?, &sides)
        }
        Some(ast::ElseBranch::IfExpr(_)) => false,
    };
    if !same {
        return None;
    }
    let function = expr.syntax().ancestors().find_map(ast::Fn::cast)?;
    if !untouched_by_macros(function.syntax()) {
        return None;
    }
    let proved = Proof {
        function: &function,
        module,
        structs,
        allow_mut: true,
    };
    let literals = [&lhs, &rhs]
        .into_iter()
        .filter(|side| is_integer(side))
        .count();
    let all_proved = [&lhs, &rhs]
        .into_iter()
        .all(|side| is_integer(side) || proved.whole_number(side));
    (literals < 2 && all_proved).then(|| REASON.to_string())
}

/// The `if` whose whole condition `compared` is, parentheses aside.
fn decided_by(compared: &ast::BinExpr) -> Option<ast::IfExpr> {
    let mut condition = compared.syntax().clone();
    while let Some(paren) = condition
        .parent()
        .filter(|p| ast::ParenExpr::can_cast(p.kind()))
    {
        condition = paren;
    }
    let branch = ast::IfExpr::cast(condition.parent()?)?;
    (branch.condition()?.syntax() == &condition).then_some(branch)
}

/// The one thing `block` does: its tail, or its one statement, with nothing else.
fn only_expression(block: &ast::BlockExpr) -> Option<ast::Expr> {
    let list = block.stmt_list()?;
    let mut statements = list.statements();
    let first = statements.next();
    if statements.next().is_some() {
        return None;
    }
    match (first, list.tail_expr()) {
        (None, Some(tail)) => Some(unparenthesized(tail)),
        (Some(ast::Stmt::ExprStmt(statement)), None) => statement.expr().map(unparenthesized),
        _ => None,
    }
}

/// `max = x`, where the two are the comparison's sides: at the edge it stores what's there.
fn assigns_one_to_the_other(expr: &ast::Expr, sides: &[String; 2]) -> bool {
    let ast::Expr::BinExpr(assignment) = expr else {
        return false;
    };
    let Some(BinaryOp::Assignment { op: None }) = assignment.op_kind() else {
        return false;
    };
    let (Some(target), Some(value)) = (assignment.lhs(), assignment.rhs()) else {
        return false;
    };
    let target = unparenthesized(target);
    !is_integer(&target) && the_sides(&target, &unparenthesized(value), sides)
}

/// One side in each branch: at the edge, either gives the same value.
fn picks_a_side(then: &ast::Expr, other: &ast::Expr, sides: &[String; 2]) -> bool {
    the_sides(then, other, sides)
}

/// Whether `one` and `other` are the two sides, in either order.
fn the_sides(one: &ast::Expr, other: &ast::Expr, sides: &[String; 2]) -> bool {
    let pair = [plain(one.syntax()), plain(other.syntax())];
    sides[0] != sides[1] && (pair == *sides || (pair[0] == sides[1] && pair[1] == sides[0]))
}
