//! The `stronger` operators whose mutants compile whenever the code did, or nearly: each takes its
//! type from what it replaces, swaps within a family of methods with the same signature, or is
//! held back where its type can't be seen (`==` to `<` only beside a number, a length or a cast).

use super::operators::{counterpart, receiver_text};
use super::*;
use ra_ap_syntax::ast::HasArgList;
use ra_ap_syntax::TextSize;

/// `checked_add` and `checked_sub`, and so on through each family: `mutastic/math`'s method forms.
const MATH_METHODS: &[(&str, &str)] = &[("add", "sub"), ("mul", "div")];
const OVERFLOW_FAMILIES: &[&str] = &["checked_", "wrapping_", "saturating_", "overflowing_"];
/// The operations a `checked_` call can be made unchecked for: `Some(a.wrapping_add(b))`.
const UNCHECKED: &[&str] = &[
    "add", "sub", "mul", "div", "rem", "pow", "neg", "abs", "shl", "shr",
];
/// The operations a `saturating_` call can wrap for instead.
const SATURATING: &[&str] = &["add", "sub", "mul", "div", "pow", "neg", "abs"];
/// Calls whose number is a size, a capacity or a wait: arid, as in the Java edition.
const ARID_NUMBER_CALLS: &[&str] = &[
    "with_capacity",
    "reserve",
    "reserve_exact",
    "sleep",
    "from_secs",
    "from_millis",
    "from_micros",
    "from_nanos",
];
/// Calls whose text only tells a person something: arid for string-constant.
const MESSAGE_CALLS: &[&str] = &["Err", "expect", "context", "with_context", "wrap_err"];

impl Finder<'_> {
    /// `if order.is_paid()` to `if !order.is_paid()`: other-comparison's form for a condition that
    /// isn't a comparison.
    pub(super) fn negated(&mut self, condition: &ast::Expr, context: Context<'_>) {
        if let ast::Expr::MethodCallExpr(call) = condition {
            let name = call
                .name_ref()
                .map(|n| n.text().to_string())
                .unwrap_or_default();
            // optional/swap-present makes this one.
            if matches!(name.as_str(), "is_some" | "is_none" | "is_ok" | "is_err") {
                return;
            }
        }
        if matches!(condition, ast::Expr::MacroExpr(_)) {
            return;
        }
        let before = plain(condition.syntax());
        let after = format!("!{}", receiver_text(condition));
        self.replaced(
            OTHER_COMPARISON,
            condition.syntax(),
            condition.syntax().text_range(),
            before,
            after,
            context,
        );
    }

    /// `matches!(x, A)` to `!matches!(x, A)`.
    pub(super) fn macro_call(&mut self, call: &ast::MacroExpr, context: Context<'_>) {
        let name = call
            .macro_call()
            .and_then(|call| call.path())
            .map(|path| plain(path.syntax()))
            .unwrap_or_default();
        if name != "matches" {
            return;
        }
        let before = plain(call.syntax());
        let after = format!("!{before}");
        self.replaced(
            OTHER_COMPARISON,
            call.syntax(),
            call.syntax().text_range(),
            before,
            after,
            context,
        );
    }

    /// The comparisons boundary and the reversal don't make, and the bitwise operators.
    pub(super) fn more_binary(&mut self, expr: &ast::BinExpr, context: Context<'_>) {
        let (Some(op), Some(token), Some(lhs), Some(rhs)) =
            (expr.op_kind(), expr.op_token(), expr.lhs(), expr.rhs())
        else {
            return;
        };
        let before = token.text().to_string();
        let operands = vec![plain(lhs.syntax()), plain(rhs.syntax())];
        self.arithmetic(expr, &token, &before, &operands, context);
        match op {
            BinaryOp::CmpOp(_) => self.comparisons(expr, &token, &before, operands, context),
            BinaryOp::ArithOp(_) => {
                for after in bitwise(&before) {
                    let replacement = format!("({}) {after} ({})", operands[0], operands[1]);
                    self.operator_swap(
                        BITWISE,
                        expr,
                        &token,
                        &before,
                        after,
                        replacement,
                        &operands,
                        context,
                    );
                }
            }
            BinaryOp::Assignment { op: Some(_) } => {
                let Some(plain_op) = before.strip_suffix('=') else {
                    return;
                };
                for after in bitwise(plain_op).into_iter().filter(|after| {
                    // `&=` and `|=` don't become `^=`: on flags, that toggles, which the other
                    // swap already shows.
                    !(matches!(plain_op, "&" | "|") && *after == "^")
                }) {
                    let after = format!("{after}=");
                    let replacement = format!("{} {after} ({})", operands[0], operands[1]);
                    self.operator_swap(
                        BITWISE,
                        expr,
                        &token,
                        &before,
                        &after,
                        replacement,
                        &operands,
                        context,
                    );
                }
            }
            _ => {}
        }
    }

    fn comparisons(
        &mut self,
        expr: &ast::BinExpr,
        token: &ra_ap_syntax::SyntaxToken,
        before: &str,
        operands: Vec<String>,
        context: Context<'_>,
    ) {
        let (a, b) = (&operands[0], &operands[1]);
        // A size against zero: every change here is the condition always true or always false,
        // which remove-conditional makes, or one no size can tell apart.
        if (is_size(a) && is_zero(b)) || (is_size(b) && is_zero(a)) {
            return;
        }
        // `==` to `<` needs an order, which most types with `==` lack: only beside a number, a
        // character, a length or a cast.
        let ordered = orderable(a) || orderable(b);
        let compared: &[(&str, &str)] = match (before, ordered) {
            ("<", _) => &[("!=", ">")],
            ("<=", _) => &[("==", "<")],
            (">", _) => &[("!=", "<")],
            (">=", _) => &[("==", ">")],
            ("==", true) => &[("<=", "<"), (">=", ">")],
            ("!=", true) => &[("<", ">"), (">", "<")],
            _ => &[],
        };
        for (after, differs) in compared {
            let caret = token.text_range();
            self.make(
                Make {
                    operator: COMPARISON,
                    node: expr.syntax(),
                    site: expr.syntax().text_range(),
                    caret,
                    replacement: format!("({a}) {after} ({b})"),
                    change: Change::Compared {
                        before: before.into(),
                        after: (*after).into(),
                        differs: (*differs).into(),
                    },
                    operands: operands.clone(),
                    form: Form::Expression,
                    claimed: None,
                },
                context,
            );
        }
        let others: &[&str] = match (before, ordered) {
            ("<", _) => &[">", "=="],
            ("<=", _) => &[">=", "!="],
            (">", _) => &["<", "=="],
            (">=", _) => &["<=", "!="],
            ("==", true) => &["<", ">"],
            ("!=", true) => &["<=", ">="],
            _ => &[],
        };
        for after in others {
            let replacement = format!("({a}) {after} ({b})");
            self.operator_swap(
                OTHER_COMPARISON,
                expr,
                token,
                before,
                after,
                replacement,
                &operands,
                context,
            );
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the parts of one mutant, passed on to `Make` as they're found"
    )]
    fn operator_swap(
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
        self.make(
            Make {
                operator,
                node: expr.syntax(),
                site: expr.syntax().text_range(),
                caret: token.text_range(),
                replacement,
                change: Change::Swapped {
                    before: before.into(),
                    after: after.into(),
                },
                operands: operands.to_vec(),
                form: Form::Expression,
                claimed: None,
            },
            context,
        );
    }

    /// An expression replaced by another, the card claiming exactly the replacement.
    pub(super) fn replaced(
        &mut self,
        operator: &'static str,
        node: &SyntaxNode,
        caret: TextRange,
        before: String,
        after: String,
        context: Context<'_>,
    ) {
        self.make(
            Make {
                operator,
                node,
                site: node.text_range(),
                caret,
                replacement: after.clone(),
                change: Change::Replaced { before, after },
                operands: Vec::new(),
                form: form_of(node),
                claimed: None,
            },
            context,
        );
    }

    /// `-x` to `x`. A negative literal is a number, which constant-value changes.
    pub(super) fn negation(&mut self, prefix: &ast::PrefixExpr, context: Context<'_>) {
        if prefix.op_kind() != Some(ast::UnaryOp::Neg) {
            return;
        }
        let Some(operand) = prefix.expr() else {
            return;
        };
        if matches!(operand, ast::Expr::Literal(_)) {
            return;
        }
        let caret = prefix
            .op_token()
            .map_or(prefix.syntax().text_range(), |op| op.text_range());
        self.replaced(
            REMOVE_NEGATION,
            prefix.syntax(),
            caret,
            plain(prefix.syntax()),
            plain(operand.syntax()),
            context,
        );
    }

    /// `S { a, b, ..base }` to `S { a, ..base }`: one field left to the base.
    pub(super) fn struct_fields(&mut self, record: &ast::RecordExpr, context: Context<'_>) {
        let Some(list) = record.record_expr_field_list() else {
            return;
        };
        if list.dotdot_token().is_none() {
            return;
        }
        for field in list.fields() {
            let mut end = field.syntax().text_range().end();
            let mut next = field.syntax().next_sibling_or_token();
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
            let removed = TextRange::new(field.syntax().text_range().start(), end);
            self.make(
                Make {
                    operator: REMOVE_STRUCT_FIELD,
                    node: record.syntax(),
                    site: record.syntax().text_range(),
                    caret: field.syntax().text_range(),
                    replacement: plain_replacing(record.syntax(), &[(removed, "")]),
                    change: Change::Replaced {
                        before: plain(field.syntax()),
                        after: String::new(),
                    },
                    operands: Vec::new(),
                    form: form_of(record.syntax()),
                    claimed: Some((
                        usize::from(removed.start())..usize::from(removed.end()),
                        String::new(),
                    )),
                },
                context,
            );
        }
    }

    /// A string to `""` (an empty one to `"mutastic"`), and a whole number to the ones next to it,
    /// `0`, `1` and, where it's visibly signed, `-1`.
    pub(super) fn literal(&mut self, literal: &ast::Literal, context: Context<'_>) {
        let token = literal.token();
        match token.kind() {
            SyntaxKind::STRING => self.string_constant(literal, &token, context),
            SyntaxKind::INT_NUMBER => self.constant_value(literal, &token, context),
            _ => {}
        }
    }

    fn string_constant(
        &mut self,
        literal: &ast::Literal,
        token: &ra_ap_syntax::SyntaxToken,
        context: Context<'_>,
    ) {
        if self.returned.contains(&literal.syntax().text_range())
            || in_call_of(literal.syntax(), MESSAGE_CALLS)
        {
            return;
        }
        let text = token.text();
        let empty = text.trim_start_matches('r').trim_matches('#') == "\"\"";
        let after = if empty { "\"mutastic\"" } else { "\"\"" };
        self.replaced(
            STRING_CONSTANT,
            literal.syntax(),
            literal.syntax().text_range(),
            text.to_string(),
            after.to_string(),
            context,
        );
    }

    fn constant_value(
        &mut self,
        literal: &ast::Literal,
        token: &ra_ap_syntax::SyntaxToken,
        context: Context<'_>,
    ) {
        // A negative number is one site: `-5`, not `5` under a minus.
        let negated = literal
            .syntax()
            .parent()
            .and_then(ast::PrefixExpr::cast)
            .filter(|prefix| prefix.op_kind() == Some(ast::UnaryOp::Neg));
        let site = negated
            .as_ref()
            .map_or(literal.syntax().clone(), |prefix| prefix.syntax().clone());
        if self.returned.contains(&site.text_range()) || in_call_of(&site, ARID_NUMBER_CALLS) {
            return;
        }
        let Some((value, suffix)) = integer_literal(token.text()) else {
            return;
        };
        let value = if negated.is_some() { -value } else { value };
        // A counter's or a total's first value is arid, as in the Java edition.
        let first_value = site
            .parent()
            .is_some_and(|parent| parent.kind() == SyntaxKind::LET_STMT);
        if first_value && matches!(value, 0 | 1) {
            return;
        }
        let signed = negated.is_some() || suffix.starts_with('i');
        let mut values = vec![value + 1, value - 1, 0, 1, -1];
        values.retain(|candidate| *candidate != value && (signed || *candidate >= 0));
        if suffix.is_empty() {
            // Its type isn't written, so it might be the largest or smallest its type holds:
            // one past that would wrap silently, as lints are capped.
            values.retain(|candidate| !(*candidate == value + 1 && is_type_max(value)));
            values.retain(|candidate| !(*candidate == value - 1 && is_type_min(value)));
        }
        let mut seen = BTreeSet::new();
        for candidate in values {
            if !seen.insert(candidate) {
                continue;
            }
            let after = format!("{candidate}{suffix}");
            self.replaced(
                CONSTANT_VALUE,
                &site,
                site.text_range(),
                plain(&site),
                after,
                context,
            );
        }
    }

    /// `is_some` and `is_none`, `is_ok` and `is_err`; the default an `Option` or `Result` falls
    /// back on, taken always, or never; `checked_add` made unchecked; and math's method forms.
    pub(super) fn more_method_call(
        &mut self,
        call: &ast::MethodCallExpr,
        name: &str,
        name_range: TextRange,
        context: Context<'_>,
    ) {
        let args: Vec<ast::Expr> = call
            .arg_list()
            .map(|list| list.args().collect())
            .unwrap_or_default();
        let pairs = [("is_some", "is_none"), ("is_ok", "is_err")];
        if let (true, Some(after)) = (args.is_empty(), counterpart(&pairs, name)) {
            self.swap_name(
                SWAP_PRESENT,
                call.syntax(),
                name_range,
                name,
                after,
                context,
            );
        }
        self.defaults(call, name, &args, context);
        self.overflow_methods(call, name, name_range, context);
    }

    fn defaults(
        &mut self,
        call: &ast::MethodCallExpr,
        name: &str,
        args: &[ast::Expr],
        context: Context<'_>,
    ) {
        let Some(receiver) = call.receiver() else {
            return;
        };
        let fallback = match (name, args) {
            ("unwrap_or", [default]) => Some(plain(default.syntax())),
            ("unwrap_or_default", []) => Some("Default::default()".to_string()),
            ("unwrap_or_else", [default]) => Some(format!("({})()", plain(default.syntax()))),
            ("map_or", [default, _]) => Some(plain(default.syntax())),
            ("map_or_else", [default, _]) => Some(format!("({})()", plain(default.syntax()))),
            _ => None,
        };
        let before = plain(call.syntax());
        if let Some(fallback) = fallback {
            // The receiver is still worked out when it could do something.
            let after = if is_pure(&receiver) {
                fallback
            } else {
                format!("{{ let _ = {}; {fallback} }}", plain(receiver.syntax()))
            };
            self.replaced(
                ALWAYS_DEFAULT,
                call.syntax(),
                call.syntax().text_range(),
                before.clone(),
                after,
                context,
            );
        }
        if matches!(name, "unwrap_or" | "unwrap_or_default" | "unwrap_or_else") {
            let after = format!("{}.unwrap()", receiver_text(&receiver));
            self.replaced(
                NO_DEFAULT,
                call.syntax(),
                call.syntax().text_range(),
                before,
                after,
                context,
            );
        }
    }

    fn overflow_methods(
        &mut self,
        call: &ast::MethodCallExpr,
        name: &str,
        name_range: TextRange,
        context: Context<'_>,
    ) {
        for family in OVERFLOW_FAMILIES {
            let Some(operation) = name.strip_prefix(family) else {
                continue;
            };
            if let Some(other) = counterpart(MATH_METHODS, operation) {
                let after = format!("{family}{other}");
                self.swap_name(MATH, call.syntax(), name_range, name, &after, context);
            }
            let wrapping = format!("wrapping_{operation}");
            if *family == "checked_" && UNCHECKED.contains(&operation) {
                let before = plain(call.syntax());
                let after = format!(
                    "Some({})",
                    plain_replacing(call.syntax(), &[(name_range, &wrapping)])
                );
                self.replaced(
                    UNCHECKED_ARITHMETIC,
                    call.syntax(),
                    name_range,
                    before,
                    after,
                    context,
                );
            }
            if *family == "saturating_" && SATURATING.contains(&operation) {
                self.swap_name(
                    UNCHECKED_ARITHMETIC,
                    call.syntax(),
                    name_range,
                    name,
                    &wrapping,
                    context,
                );
            }
        }
    }

    /// `Some(x)` to `None`, where it isn't returned (return-value makes that one).
    pub(super) fn some_made_none(&mut self, call: &ast::CallExpr, context: Context<'_>) {
        let Some(ast::Expr::PathExpr(path)) = call.expr() else {
            return;
        };
        if plain(path.syntax()) != "Some" || self.returned.contains(&call.syntax().text_range()) {
            return;
        }
        let args: Vec<ast::Expr> = call
            .arg_list()
            .map(|list| list.args().collect())
            .unwrap_or_default();
        let [value] = args.as_slice() else {
            return;
        };
        let after = if is_pure(value) || super::operators::untyped(value) {
            "None".to_string()
        } else {
            format!("{{ let _ = {}; None }}", plain(value.syntax()))
        };
        self.replaced(
            OPTIONAL_EMPTY,
            call.syntax(),
            path.syntax().text_range(),
            plain(call.syntax()),
            after,
            context,
        );
    }

    /// An `Option` parameter that is `None` from the start: `let c = None;` written at the top of
    /// the body, each behind its own switch.
    pub(super) fn parameters(&mut self, function: &ast::Fn, context: Context<'_>) {
        let Some(list) = function.param_list() else {
            return;
        };
        let body_open = self.found.functions[context.function].body_open;
        for param in list.params() {
            let (Some(ast::Pat::IdentPat(pat)), Some(ty)) = (param.pat(), param.ty()) else {
                continue;
            };
            let ty = plain(ty.syntax());
            if !ty.starts_with("Option<") {
                continue;
            }
            let Some(name) = pat.name().map(|name| name.text().to_string()) else {
                continue;
            };
            let binding = if pat.mut_token().is_some() {
                format!("mut {name}")
            } else {
                name.clone()
            };
            let at = TextSize::from(body_open as u32);
            self.make(
                Make {
                    operator: NULL_PARAMETER,
                    node: param.syntax(),
                    site: TextRange::empty(at),
                    caret: pat.syntax().text_range(),
                    replacement: format!(" let {binding}: {ty} = None;"),
                    change: Change::Parameter {
                        name: name.clone(),
                        value: "None".into(),
                    },
                    operands: Vec::new(),
                    form: Form::Insert {
                        text: format!(
                            " let {binding} = match crate::__mutastic::at({{n}}) {{ {{n}} => None, _ => {name} }};"
                        ),
                    },
                    claimed: None,
                },
                context,
            );
        }
    }

    /// The values `return` gives anywhere in a body, as return-value's sites: other operators
    /// leave those alone.
    pub(super) fn mark_returns(&mut self, body: &SyntaxNode) {
        let returns: Vec<ast::Expr> =
            body.descendants()
                .filter_map(ast::ReturnExpr::cast)
                .filter(|ret| {
                    !ret.syntax().ancestors().take_while(|a| a != body).any(|a| {
                        ast::ClosureExpr::can_cast(a.kind()) || ast::Fn::can_cast(a.kind())
                    })
                })
                .filter_map(|ret| ret.expr())
                .collect();
        for value in returns {
            returned_leaves(&value, &mut self.returned);
        }
    }
}

/// The values an expression returns: its tail, its branches', its arms'.
fn returned_leaves(value: &ast::Expr, out: &mut std::collections::HashSet<TextRange>) {
    match value {
        ast::Expr::BlockExpr(block) => {
            if let Some(tail) = block.tail_expr() {
                returned_leaves(&tail, out);
            }
        }
        ast::Expr::IfExpr(expr) => {
            if let Some(tail) = expr.then_branch().and_then(|then| then.tail_expr()) {
                returned_leaves(&tail, out);
            }
            match expr.else_branch() {
                Some(ast::ElseBranch::Block(block)) => {
                    if let Some(tail) = block.tail_expr() {
                        returned_leaves(&tail, out);
                    }
                }
                Some(ast::ElseBranch::IfExpr(elif)) => {
                    returned_leaves(&ast::Expr::IfExpr(elif), out)
                }
                None => {}
            }
        }
        ast::Expr::MatchExpr(expr) => {
            for arm in expr
                .match_arm_list()
                .into_iter()
                .flat_map(|list| list.arms())
            {
                if let Some(tail) = arm.expr() {
                    returned_leaves(&tail, out);
                }
            }
        }
        other => {
            out.insert(other.syntax().text_range());
        }
    }
}

fn bitwise(op: &str) -> Vec<&'static str> {
    match op {
        "&" => vec!["|", "^"],
        "|" => vec!["&", "^"],
        "^" => vec!["&", "|"],
        "<<" => vec![">>"],
        ">>" => vec!["<<"],
        _ => Vec::new(),
    }
}

/// `v.len()`, `it.count()`: a size, never negative.
fn is_size(text: &str) -> bool {
    [".len()", ".count()", ".capacity()"]
        .iter()
        .any(|size| text.ends_with(size))
}

fn is_zero(text: &str) -> bool {
    integer_literal(text).is_some_and(|(value, _)| value == 0)
}

/// Whether a comparison's operand shows its type has an order: a number, a character, a size or
/// a cast.
fn orderable(text: &str) -> bool {
    integer_literal(text).is_some()
        || text.parse::<f64>().is_ok()
        || (text.starts_with('\'') && text.ends_with('\'') && text.len() >= 3)
        || is_size(text)
        || text.contains(" as ")
}

/// A path, a field, a literal: an expression whose value can be dropped unworked.
pub(super) fn is_pure(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::PathExpr(_) | ast::Expr::Literal(_) => true,
        ast::Expr::FieldExpr(field) => field.expr().is_some_and(|inner| is_pure(&inner)),
        ast::Expr::ParenExpr(inner) => inner.expr().is_some_and(|inner| is_pure(&inner)),
        _ => false,
    }
}

/// Whether a node is an argument of a call to one of `names`, directly.
fn in_call_of(node: &SyntaxNode, names: &[&str]) -> bool {
    let Some(list) = node.parent().and_then(ast::ArgList::cast) else {
        return false;
    };
    list.syntax()
        .parent()
        .and_then(|call| callee(&call))
        .is_some_and(|name| names.contains(&name.as_str()))
}

fn is_type_max(value: i128) -> bool {
    [7, 8, 15, 16, 31, 32, 63, 64]
        .iter()
        .any(|bits| value == (1i128 << bits) - 1)
}

fn is_type_min(value: i128) -> bool {
    [7, 15, 31, 63].iter().any(|bits| value == -(1i128 << bits))
}
