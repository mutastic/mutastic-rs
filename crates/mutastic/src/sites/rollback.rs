//! The `stronger` operators that lean on the build to withdraw what doesn't compile: their change
//! can give the expression another type than syntax shows (`a + b` to `a` when `a` is a
//! reference), so some are unviable, and the schema's rollback takes them out.

use super::operators::{counterpart, receiver_text};
use super::stronger::is_pure;
use super::*;
use ra_ap_syntax::ast::HasArgList;

/// Methods that give back what they're called on, changed: leaving the call out keeps the value.
const OMITTABLE: &[&str] = &[
    "trim",
    "trim_start",
    "trim_end",
    "trim_matches",
    "trim_start_matches",
    "trim_end_matches",
    "to_lowercase",
    "to_uppercase",
    "to_ascii_lowercase",
    "to_ascii_uppercase",
    "abs",
    "round",
    "floor",
    "ceil",
    "trunc",
];

/// The first and last of a search: `find` and `rfind`.
const INDEX_OF: &[(&str, &str)] = &[
    ("find", "rfind"),
    ("position", "rposition"),
    ("split_once", "rsplit_once"),
    ("match_indices", "rmatch_indices"),
];

impl Finder<'_> {
    /// `+` to `*`, the other arithmetic operators, and a calculation to one of its values.
    pub(super) fn arithmetic(
        &mut self,
        expr: &ast::BinExpr,
        token: &ra_ap_syntax::SyntaxToken,
        before: &str,
        operands: &[String],
        context: Context<'_>,
    ) {
        let (op, compound) = match before.strip_suffix('=') {
            Some(op) if matches!(op, "+" | "-" | "*" | "/" | "%") => (op, true),
            _ if matches!(before, "+" | "-" | "*" | "/" | "%") => (before, false),
            _ => return,
        };
        let (a, b) = (&operands[0], &operands[1]);
        let write = |after: &str| {
            if compound {
                format!("{a} {after}= ({b})")
            } else {
                format!("({a}) {after} ({b})")
            }
        };
        let suffix = if compound { "=" } else { "" };
        // `i += 1` to `i *= 1` leaves `i` as it was: a step, not a calculation.
        let steps_by_one = compound && integer_literal(b).is_some_and(|(value, _)| value == 1);
        if op == "+" && !steps_by_one {
            self.operator_change(
                ARITHMETIC_REPLACEMENT,
                expr,
                token,
                before,
                &format!("*{suffix}"),
                write("*"),
                operands,
                context,
            );
        }
        let others: &[&str] = match op {
            "+" => &["/", "%"],
            "-" => &["*", "/", "%"],
            "*" => &["+", "-", "%"],
            "/" => &["+", "-", "%"],
            // `%` to `*` is older, and made beside the operator it was first in.
            _ => &["+", "-", "/"],
        };
        for after in others {
            self.operator_change(
                OTHER_ARITHMETIC,
                expr,
                token,
                before,
                &format!("{after}{suffix}"),
                write(after),
                operands,
                context,
            );
        }
        if !compound {
            self.keep_operand(expr, context);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn operator_change(
        &mut self,
        operator: &'static str,
        expr: &ast::BinExpr,
        token: &ra_ap_syntax::SyntaxToken,
        before: &str,
        after: &str,
        replacement: String,
        operands: &[String],
        context: Context<'_>,
    ) {
        // `-` to `*` moves the operator to another level of precedence: swapping the token alone
        // would regroup the code around it, so the card claims the grouped expression.
        let multiplies = |op: &str| matches!(op, "*" | "/" | "%");
        let claimed = (!before.ends_with('=') && multiplies(before) != multiplies(after))
            .then(|| grouped(expr, after))
            .flatten();
        self.make(
            Make {
                operator,
                node: expr.syntax(),
                site: expr.syntax().text_range(),
                caret: token.text_range(),
                replacement,
                change: Change::Math {
                    before: before.into(),
                    after: after.into(),
                },
                operands: operands.to_vec(),
                form: Form::Expression,
                claimed,
            },
            context,
        );
    }

    /// `price + fee` to `price`, and to `fee`; the other is still worked out when it could do
    /// something. A constant beside a value isn't what's kept: `count + 1` becomes `count` only.
    fn keep_operand(&mut self, expr: &ast::BinExpr, context: Context<'_>) {
        let (Some(lhs), Some(rhs)) = (expr.lhs(), expr.rhs()) else {
            return;
        };
        let constant = |side: &ast::Expr| matches!(side, ast::Expr::Literal(_));
        let (a, b) = (plain(lhs.syntax()), plain(rhs.syntax()));
        let mut kept = Vec::new();
        if !constant(&lhs) {
            kept.push(if is_pure(&rhs) {
                a.clone()
            } else {
                format!("{{ let kept = {a}; let _ = {b}; kept }}")
            });
        }
        if !constant(&rhs) {
            kept.push(if is_pure(&lhs) {
                b.clone()
            } else {
                format!("{{ let _ = {a}; {b} }}")
            });
        }
        let before = plain(expr.syntax());
        for after in kept {
            self.replaced(
                KEEP_OPERAND,
                expr.syntax(),
                expr.syntax().text_range(),
                before.clone(),
                after,
                context,
            );
        }
    }

    /// `s.trim()` to `s`; `a.find(b)` to `a.rfind(b)`; `a.cmp(&b)` to `b.cmp(&a)`; a builder's
    /// step left out; the last two arguments of a method swapped.
    pub(super) fn rollback_method_call(
        &mut self,
        call: &ast::MethodCallExpr,
        name: &str,
        name_range: TextRange,
        context: Context<'_>,
    ) {
        let Some(receiver) = call.receiver() else {
            return;
        };
        let args: Vec<ast::Expr> = call
            .arg_list()
            .map(|list| list.args().collect())
            .unwrap_or_default();
        if OMITTABLE.contains(&name) || self.facts.builders.contains(name) {
            self.omit_method(call, &receiver, OMIT_CALL, context);
        }
        if let Some(after) = counterpart(INDEX_OF, name) {
            self.swap_name(
                SWAP_INDEX_OF,
                call.syntax(),
                name_range,
                name,
                after,
                context,
            );
        }
        if let ("cmp" | "partial_cmp", [other]) = (name, args.as_slice()) {
            let (theirs, borrowed) = match other {
                ast::Expr::RefExpr(inner) => match inner.expr() {
                    Some(inner) => (receiver_text(&inner), true),
                    None => return,
                },
                other => (receiver_text(other), false),
            };
            let ours = receiver_text(&receiver);
            let after = format!("{theirs}.{name}({}{ours})", if borrowed { "&" } else { "" });
            self.replaced(
                REVERSE_ORDERING,
                call.syntax(),
                name_range,
                plain(call.syntax()),
                after,
                context,
            );
        }
        if name == "reverse" && args.is_empty() && orders(&receiver) {
            self.omit_method(call, &receiver, REVERSE_ORDERING, context);
        }
        if self.facts.swappable.contains(&(name.to_string(), true)) {
            self.swap_last_arguments(call.syntax(), &args, context);
        }
    }

    /// A method call left out, its receiver going on in its place.
    pub(super) fn omit_method(
        &mut self,
        call: &ast::MethodCallExpr,
        receiver: &ast::Expr,
        operator: &'static str,
        context: Context<'_>,
    ) {
        let piece = plain_replacing(call.syntax(), &[(receiver.syntax().text_range(), "")]);
        let start = call
            .dot_token()
            .map_or(receiver.syntax().text_range().end(), |dot| {
                dot.text_range().start()
            });
        let deleted = usize::from(start)..usize::from(call.syntax().text_range().end());
        let caret = call.name_ref().map_or(call.syntax().text_range(), |name| {
            name.syntax().text_range()
        });
        self.make(
            Make {
                operator,
                node: call.syntax(),
                site: call.syntax().text_range(),
                caret,
                replacement: plain(receiver.syntax()),
                change: Change::Replaced {
                    before: piece,
                    after: String::new(),
                },
                operands: Vec::new(),
                form: form_of(call.syntax()),
                claimed: Some((deleted, String::new())),
            },
            context,
        );
    }

    /// `escape(comment)` to `comment`, for a crate function that returns its one argument's type;
    /// and the last two arguments of a crate function swapped.
    pub(super) fn rollback_function_call(&mut self, call: &ast::CallExpr, context: Context<'_>) {
        let Some(name) = callee(call.syntax()) else {
            return;
        };
        let args: Vec<ast::Expr> = call
            .arg_list()
            .map(|list| list.args().collect())
            .unwrap_or_default();
        if let (true, [arg]) = (self.facts.same_type_fns.contains(&name), args.as_slice()) {
            let caret = call
                .expr()
                .map_or(call.syntax().text_range(), |f| f.syntax().text_range());
            self.replaced(
                OMIT_CALL,
                call.syntax(),
                caret,
                plain(call.syntax()),
                plain(arg.syntax()),
                context,
            );
        }
        if self.facts.swappable.contains(&(name, false)) {
            self.swap_last_arguments(call.syntax(), &args, context);
        }
    }

    fn swap_last_arguments(&mut self, call: &SyntaxNode, args: &[ast::Expr], context: Context<'_>) {
        let [.., first, second] = args else {
            return;
        };
        let (a, b) = (plain(first.syntax()), plain(second.syntax()));
        if a == b {
            return;
        }
        let both = TextRange::new(
            first.syntax().text_range().start(),
            second.syntax().text_range().end(),
        );
        let swapped = format!("{b}, {a}");
        self.make(
            Make {
                operator: SWAP_ARGUMENTS,
                node: call,
                site: call.text_range(),
                caret: both,
                replacement: plain_replacing(call, &[(both, &swapped)]),
                change: Change::Swapped {
                    before: format!("{a}, {b}"),
                    after: swapped.clone(),
                },
                operands: Vec::new(),
                form: form_of(call),
                claimed: None,
            },
            context,
        );
    }

    /// `f()?` to `f().unwrap()`: is the error path tested?
    pub(super) fn unwrap_try(&mut self, expr: &ast::TryExpr, context: Context<'_>) {
        let (Some(inner), Some(question)) = (expr.expr(), expr.question_mark_token()) else {
            return;
        };
        let range = question.text_range();
        self.make(
            Make {
                operator: UNWRAP_TRY,
                node: expr.syntax(),
                site: expr.syntax().text_range(),
                caret: range,
                replacement: format!("{}.unwrap()", receiver_text(&inner)),
                change: Change::Swapped {
                    before: "?".into(),
                    after: ".unwrap()".into(),
                },
                operands: Vec::new(),
                form: Form::Expression,
                claimed: None,
            },
            context,
        );
    }

    /// An arm that stops matching, with a guard the switch decides: its values go on to the next
    /// arm that matches. Only before a later arm that matches anything, unguarded, so the `match`
    /// stays exhaustive.
    pub(super) fn switch_arms(&mut self, expr: &ast::MatchExpr, context: Context<'_>) {
        let arms: Vec<ast::MatchArm> = expr
            .match_arm_list()
            .into_iter()
            .flat_map(|list| list.arms())
            .collect();
        let Some(last_catch_all) = arms.iter().rposition(catches_all) else {
            return;
        };
        for arm in &arms[..last_catch_all] {
            let Some(pat) = arm.pat() else {
                continue;
            };
            // A guard on an arm that binds can change how its bindings borrow: only arms that bind
            // nothing get one.
            if arm.guard().is_some() || catches_all(arm) || binds(&pat) {
                continue;
            }
            let mut end = arm.syntax().text_range().end();
            let mut next = arm.syntax().next_sibling_or_token();
            while let Some(element) = next {
                match element.kind() {
                    SyntaxKind::WHITESPACE | SyntaxKind::COMMENT => {}
                    SyntaxKind::COMMA => {
                        end = element.text_range().end();
                        break;
                    }
                    _ => break,
                }
                next = element.next_sibling_or_token();
            }
            let removed = usize::from(arm.syntax().text_range().start())..usize::from(end);
            let pattern = plain(pat.syntax());
            self.make(
                Make {
                    operator: REMOVE_SWITCH_ARM,
                    node: arm.syntax(),
                    site: TextRange::empty(pat.syntax().text_range().end()),
                    caret: pat.syntax().text_range(),
                    replacement: format!("{pattern} if false"),
                    change: Change::ArmRemoved { pattern },
                    operands: Vec::new(),
                    form: Form::Insert {
                        text: " if crate::__mutastic::at({n}) != {n}".into(),
                    },
                    claimed: Some((removed, String::new())),
                },
                context,
            );
        }
    }
}

/// `a op b` written so it groups as the mutant does, in parentheses only where they're needed: an
/// operand that is itself a calculation, and the whole where it's an operand of another.
fn grouped(expr: &ast::BinExpr, op: &str) -> Option<(std::ops::Range<usize>, String)> {
    let side = |side: ast::Expr| {
        let text = plain(side.syntax());
        if matches!(side, ast::Expr::BinExpr(_) | ast::Expr::RangeExpr(_)) {
            format!("({text})")
        } else {
            text
        }
    };
    let text = format!("{} {op} {}", side(expr.lhs()?), side(expr.rhs()?));
    let nested = expr.syntax().parent().is_some_and(|parent| {
        matches!(
            parent.kind(),
            SyntaxKind::BIN_EXPR
                | SyntaxKind::PREFIX_EXPR
                | SyntaxKind::METHOD_CALL_EXPR
                | SyntaxKind::CAST_EXPR
                | SyntaxKind::FIELD_EXPR
                | SyntaxKind::INDEX_EXPR
                | SyntaxKind::TRY_EXPR
        )
    });
    let range = expr.syntax().text_range();
    let text = if nested { format!("({text})") } else { text };
    Some((usize::from(range.start())..usize::from(range.end()), text))
}

/// `_` or a plain binding: an arm that matches whatever reaches it, when unguarded.
fn catches_all(arm: &ast::MatchArm) -> bool {
    if arm.guard().is_some() {
        return false;
    }
    match arm.pat() {
        Some(ast::Pat::WildcardPat(_)) => true,
        Some(ast::Pat::IdentPat(pat)) => {
            pat.pat().is_none()
                && pat.name().is_some_and(|name| {
                    name.text()
                        .starts_with(|c: char| c.is_lowercase() || c == '_')
                })
        }
        _ => false,
    }
}

/// Whether a pattern binds a name: a lowercase identifier, not a constant or a unit variant.
fn binds(pat: &ast::Pat) -> bool {
    pat.syntax()
        .descendants()
        .filter_map(ast::IdentPat::cast)
        .filter_map(|ident| ident.name())
        .any(|name| {
            name.text()
                .starts_with(|c: char| c.is_lowercase() || c == '_')
        })
}

/// Whether an expression is visibly an `Ordering`: a `cmp`, `partial_cmp` or `then` call.
fn orders(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::MethodCallExpr(call) => call.name_ref().is_some_and(|name| {
            matches!(
                name.text().as_str(),
                "cmp" | "partial_cmp" | "then" | "then_with"
            )
        }),
        _ => false,
    }
}
