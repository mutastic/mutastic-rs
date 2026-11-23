//! The operators beyond comparisons, conditions, arithmetic and returns: statements removed,
//! library methods swapped for their counterparts, iterator stages left out, and range ends moved.
//!
//! Each is decided from syntax, so each says what it can see: a method is swapped by its name, an
//! iterator is one when its chain visibly starts from one, and a call returns nothing when the
//! crate declares it so or it's one of std's that don't. A guess that's wrong doesn't compile, and
//! the build withdraws it.

use super::*;
use crate::arid::{ONLY_CHECKS_A_VALUE, ONLY_WRITES_OUTPUT};
use crate::callrules::Rewrite;
use ra_ap_syntax::ast::{HasArgList, RangeItem};

/// Method names and their counterparts, by operator.
const SWAPS: &[(&str, &[(&str, &str)])] = &[
    (
        SWAP_STRING,
        &[
            ("starts_with", "ends_with"),
            ("to_uppercase", "to_lowercase"),
            ("to_ascii_uppercase", "to_ascii_lowercase"),
            ("make_ascii_uppercase", "make_ascii_lowercase"),
            ("trim_start", "trim_end"),
            ("trim_start_matches", "trim_end_matches"),
            ("strip_prefix", "strip_suffix"),
        ],
    ),
    (
        SWAP_MATH,
        &[
            ("min", "max"),
            ("min_by", "max_by"),
            ("min_by_key", "max_by_key"),
            ("floor", "ceil"),
        ],
    ),
    (SWAP_MATCH, &[("any", "all")]),
];

/// std's methods and functions that return nothing, so removing a call is all a mutant can do.
const VOID_STD: &[&str] = &[
    "push",
    "push_str",
    "push_back",
    "push_front",
    "clear",
    "sort",
    "sort_by",
    "sort_by_key",
    "sort_unstable",
    "sort_unstable_by",
    "sort_unstable_by_key",
    "dedup",
    "dedup_by",
    "dedup_by_key",
    "truncate",
    "extend",
    "extend_from_slice",
    "retain",
    "retain_mut",
    "reverse",
    "drop",
    "swap",
    "fill",
    "resize",
    "append",
    "reserve",
    "shrink_to_fit",
    "store",
    "set",
    "insert_str",
];

/// The calls that only check a value (`x.unwrap();`). A statement of one is work when what it
/// checks comes from a call, `handle.join().unwrap();`, and that call is removed; one that checks
/// a value it's given only checks it, as the Java edition's `Objects.requireNonNull(x);` does.
const CHECK_CALLS: &[&str] = &["unwrap", "expect"];

/// Calls that only give a view of a value (`x.as_ref().unwrap();`), so checking what they give
/// checks the value itself.
const VIEW_CALLS: &[&str] = &["as_ref", "as_mut", "as_deref", "as_deref_mut"];

/// Macros whose statement only writes output, to the terminal or a logger: removing one changes
/// nothing a test should have to notice, so its mutant is made, for its id, and skipped as arid,
/// as the Java edition skips a call to a logger or `System.out`. A `write!` goes to a writer the
/// code chose, which a test may read, so it isn't one.
pub(super) const OUTPUT_MACROS: &[&str] = &[
    "print", "println", "eprint", "eprintln", "dbg", "trace", "debug", "info", "warn", "error",
    "log", "event",
];

/// Macros that check whether a log level is on, `log_enabled!(Level::Debug)` and `tracing`'s
/// `enabled!`: around a body that only writes output, all they decide is whether it's written.
const LEVEL_CHECKS: &[&str] = &[
    "log_enabled",
    "enabled",
    "event_enabled",
    "span_enabled",
    "level_enabled",
];

/// The last segment of a macro's path: `warn` for `log::warn!(..)`.
fn macro_name(output: &ast::MacroExpr) -> String {
    output
        .macro_call()
        .and_then(|call| call.path())
        .and_then(|path| path.segment())
        .map(|segment| segment.syntax().text().to_string())
        .unwrap_or_default()
}

/// An output macro, as a statement or a block's last expression.
fn writes_output(expr: Option<ast::Expr>) -> bool {
    matches!(expr, Some(ast::Expr::MacroExpr(output)) if OUTPUT_MACROS.contains(&macro_name(&output).as_str()))
}

/// `if log_enabled!(Level::Debug) { debug!(..); }`: a level's check, `!` or not, with no `else`,
/// around a body each statement of which is an output macro. A body that does anything else, a
/// `let` or a call of the code's own, keeps the check counted.
pub(super) fn guards_only_output(expr: &ast::IfExpr) -> bool {
    let mut condition = expr.condition();
    loop {
        condition = match condition {
            Some(ast::Expr::ParenExpr(paren)) => paren.expr(),
            Some(ast::Expr::PrefixExpr(prefix)) if prefix.op_kind() == Some(ast::UnaryOp::Not) => {
                prefix.expr()
            }
            other => break condition = other,
        };
    }
    let checks = matches!(&condition, Some(ast::Expr::MacroExpr(check)) if LEVEL_CHECKS.contains(&macro_name(check).as_str()));
    let Some(body) = expr.then_branch().and_then(|block| block.stmt_list()) else {
        return false;
    };
    if !checks || expr.else_branch().is_some() {
        return false;
    }
    let mut statements = 0;
    for statement in body.statements() {
        let ast::Stmt::ExprStmt(statement) = statement else {
            return false;
        };
        if statement.semicolon_token().is_none() || !writes_output(statement.expr()) {
            return false;
        }
        statements += 1;
    }
    match body.tail_expr() {
        Some(tail) => writes_output(Some(tail)),
        None => statements > 0,
    }
}

/// Item-preserving iterator stages, which can be left out without changing the item type.
const STAGES: &[&str] = &[
    "filter",
    "skip",
    "take",
    "skip_while",
    "take_while",
    "step_by",
    "rev",
];

/// Methods that start an iterator.
const ITERATOR_SOURCES: &[&str] = &[
    "iter",
    "iter_mut",
    "into_iter",
    "chars",
    "char_indices",
    "bytes",
    "lines",
    "split",
    "rsplit",
    "splitn",
    "split_terminator",
    "split_whitespace",
    "split_ascii_whitespace",
    "windows",
    "chunks",
    "chunks_exact",
    "keys",
    "values",
    "values_mut",
    "into_keys",
    "into_values",
    "drain",
    "matches",
    "match_indices",
];

/// Methods that take an iterator and give one.
const ITERATOR_ADAPTORS: &[&str] = &[
    "map",
    "filter",
    "filter_map",
    "enumerate",
    "zip",
    "chain",
    "skip",
    "take",
    "rev",
    "cloned",
    "copied",
    "flat_map",
    "flatten",
    "peekable",
    "skip_while",
    "take_while",
    "step_by",
    "inspect",
    "map_while",
    "scan",
    "fuse",
];

/// Sources that never end: leaving out a `take` there makes a mutant that hangs.
const UNBOUNDED: &[&str] = &["repeat", "repeat_with", "cycle", "successors", "from_fn"];

/// Methods that consume an iterator into a value: a chain up to one of these keeps its type when a
/// stage is left out.
const CONSUMERS: &[&str] = &[
    "count",
    "sum",
    "product",
    "collect",
    "any",
    "all",
    "min",
    "max",
    "min_by",
    "max_by",
    "min_by_key",
    "max_by_key",
    "last",
    "next",
    "nth",
    "find",
    "find_map",
    "position",
    "fold",
    "try_fold",
    "reduce",
    "for_each",
    "try_for_each",
    "partition",
    "unzip",
    "eq",
    "ne",
    "lt",
    "le",
    "gt",
    "ge",
    "cmp",
];

/// Consumers order doesn't matter to, so `rev` before them changes nothing.
const ORDER_BLIND: &[&str] = &["count", "sum", "product", "any", "all", "min", "max"];

impl Finder<'_> {
    /// The operators that look at one node.
    pub(super) fn visit(&mut self, node: &SyntaxNode, context: Context<'_>) {
        if let Some(statement) = ast::ExprStmt::cast(node.clone()) {
            self.statement(&statement, context);
        } else if let Some(statement) = ast::LetStmt::cast(node.clone()) {
            self.let_statement(&statement, context);
        } else if let Some(call) = ast::MethodCallExpr::cast(node.clone()) {
            self.method_call(&call, context);
        } else if let Some(call) = ast::CallExpr::cast(node.clone()) {
            self.function_call(&call, context);
        } else if let Some(range) = ast::RangeExpr::cast(node.clone()) {
            self.range_end(&range, context);
        } else if let Some(expr) = ast::BinExpr::cast(node.clone()) {
            self.more_binary(&expr, context);
        } else if let Some(prefix) = ast::PrefixExpr::cast(node.clone()) {
            self.negation(&prefix, context);
        } else if let Some(record) = ast::RecordExpr::cast(node.clone()) {
            self.struct_fields(&record, context);
        } else if let Some(literal) = ast::Literal::cast(node.clone()) {
            self.literal(&literal, context);
        } else if let Some(call) = ast::MacroExpr::cast(node.clone()) {
            self.macro_call(&call, context);
        } else if let Some(expr) = ast::TryExpr::cast(node.clone()) {
            self.unwrap_try(&expr, context);
        } else if let Some(expr) = ast::MatchExpr::cast(node.clone()) {
            self.switch_arms(&expr, context);
        } else if matches!(
            node.kind(),
            SyntaxKind::BREAK_EXPR | SyntaxKind::CONTINUE_EXPR
        ) {
            self.swap_break(node, context);
        } else if let Some(guard) = ast::MatchGuard::cast(node.clone()) {
            if let Some(condition) = guard.condition() {
                self.condition(&condition, context);
            }
        }
    }

    /// A statement removed: a call whose result isn't used, or an assignment.
    fn statement(&mut self, statement: &ast::ExprStmt, context: Context<'_>) {
        // A statement under an attribute (`#[cfg(feature = "debug")]`) may not be there at all.
        if statement.semicolon_token().is_none() || plain(statement.syntax()).starts_with("#[") {
            return;
        }
        let Some(expr) = statement.expr() else {
            return;
        };
        if let ast::Expr::BinExpr(assign) = &expr {
            if let Some(BinaryOp::Assignment { op: None }) = assign.op_kind() {
                self.assignment(statement, assign, context);
            }
            return;
        }
        self.remove_throw(statement, context);
        if let ast::Expr::MacroExpr(output) = &expr {
            self.output_macro(statement, output, context);
            return;
        }
        let Some((call, name)) = called(&expr) else {
            return;
        };
        if CHECK_CALLS.contains(&name.as_str()) {
            self.checked_call(statement, &call, context);
            return;
        }
        let operator = if self.facts.unit_fns.contains(&name) || VOID_STD.contains(&name.as_str()) {
            REMOVE_VOID_CALL
        } else {
            REMOVE_DISCARDED_CALL
        };
        self.removal(operator, statement.syntax(), &call, String::new(), context);
    }

    /// `handle.join().unwrap();` removed, as the call whose result it checks is; or `x.unwrap();`,
    /// which only checks a value it's given, made, so it's counted, and skipped as arid.
    fn checked_call(
        &mut self,
        statement: &ast::ExprStmt,
        check: &SyntaxNode,
        context: Context<'_>,
    ) {
        let mut checked = checked_value(check);
        while let Some((call, name)) = checked.as_ref().and_then(called) {
            if CHECK_CALLS.contains(&name.as_str()) || VIEW_CALLS.contains(&name.as_str()) {
                checked = checked_value(&call);
                continue;
            }
            // What's checked is a result, so the call returns one, and it's thrown away.
            self.removal(
                REMOVE_DISCARDED_CALL,
                statement.syntax(),
                &call,
                String::new(),
                context,
            );
            return;
        }
        let before = self.found.mutants.len();
        self.removal(
            REMOVE_DISCARDED_CALL,
            statement.syntax(),
            check,
            String::new(),
            context,
        );
        if self.found.mutants.len() > before {
            if let Some(mutant) = self.found.mutants.last_mut() {
                mutant.skipped = Some(ONLY_CHECKS_A_VALUE.to_string());
            }
        }
    }

    /// `log::warn!(..);` or `eprintln!(..);` removed: made, so it's counted and keeps an id, and
    /// skipped as arid, so a change that only adds output says why nothing ran.
    fn output_macro(
        &mut self,
        statement: &ast::ExprStmt,
        output: &ast::MacroExpr,
        context: Context<'_>,
    ) {
        let Some(call) = output.macro_call() else {
            return;
        };
        if !OUTPUT_MACROS.contains(&macro_name(output).as_str()) {
            return;
        }
        let before = self.found.mutants.len();
        self.removal(
            REMOVE_VOID_CALL,
            statement.syntax(),
            call.syntax(),
            String::new(),
            context,
        );
        if self.found.mutants.len() > before {
            if let Some(mutant) = self.found.mutants.last_mut() {
                mutant.skipped = Some(ONLY_WRITES_OUTPUT.to_string());
            }
        }
    }

    /// `total = total + p;` becomes `let _ = total + p;`: the variable keeps its value, and what
    /// it would have been given is still worked out.
    fn assignment(
        &mut self,
        statement: &ast::ExprStmt,
        assign: &ast::BinExpr,
        context: Context<'_>,
    ) {
        let (Some(target), Some(value)) = (assign.lhs(), assign.rhs()) else {
            return;
        };
        let operator = match &target {
            ast::Expr::PathExpr(path) => {
                let name = plain(path.syntax());
                if name.contains("::") || self.declared_without_value(statement.syntax(), &name) {
                    return;
                }
                REMOVE_ASSIGNMENT
            }
            ast::Expr::FieldExpr(_) | ast::Expr::IndexExpr(_) => REMOVE_FIELD_WRITE,
            ast::Expr::PrefixExpr(prefix) if prefix.op_kind() == Some(ast::UnaryOp::Deref) => {
                REMOVE_FIELD_WRITE
            }
            _ => return,
        };
        // A value whose type only its place gives (`None`, `Vec::new()`) can't stand alone, and
        // has nothing to work out.
        let replacement = if untyped(&value) {
            String::new()
        } else {
            format!("let _ = {};", plain(value.syntax()))
        };
        self.removal(
            operator,
            statement.syntax(),
            target.syntax(),
            replacement,
            context,
        );
    }

    /// `let x;` with no value: its first assignment can't be removed.
    fn declared_without_value(&self, at: &SyntaxNode, name: &str) -> bool {
        at.ancestors()
            .find_map(ast::Fn::cast)
            .into_iter()
            .flat_map(|function| function.syntax().descendants().collect::<Vec<_>>())
            .filter_map(ast::LetStmt::cast)
            .filter(|binding| binding.initializer().is_none())
            .any(|binding| {
                binding
                    .pat()
                    .is_some_and(|pat| plain(pat.syntax()).trim_start_matches("mut ") == name)
            })
    }

    fn removal(
        &mut self,
        operator: &'static str,
        statement: &SyntaxNode,
        caret: &SyntaxNode,
        replacement: String,
        context: Context<'_>,
    ) {
        self.make(
            Make {
                operator,
                node: statement,
                site: statement.text_range(),
                caret: caret.text_range(),
                replacement,
                change: Change::Removed {
                    statement: plain(statement),
                },
                operands: Vec::new(),
                form: Form::Statement,
                claimed: None,
            },
            context,
        );
    }

    /// `let _ = f();` removed; and `let Some(x) = e else { .. }` with `e` always `None`.
    fn let_statement(&mut self, statement: &ast::LetStmt, context: Context<'_>) {
        let (Some(pat), Some(value)) = (statement.pat(), statement.initializer()) else {
            return;
        };
        if statement.let_else().is_some() {
            self.never_some(&pat, &value, context);
            return;
        }
        if matches!(pat, ast::Pat::WildcardPat(_)) {
            if let Some((call, _)) = called(&value) {
                self.removal(
                    REMOVE_DISCARDED_CALL,
                    statement.syntax(),
                    &call,
                    String::new(),
                    context,
                );
            }
            return;
        }
        self.stored_then_checked(statement, &pat, &value, context);
    }

    /// `let removed = remove_file(&p); removed.unwrap();` removed, both statements, caret on the
    /// call: the value is only checked, so this is `remove_file(&p).unwrap();` written in two, and
    /// goes as that does. The check alone stays skipped as arid. Only when the check comes next
    /// and nothing else uses the value: removing what stands between would remove more than the
    /// call.
    fn stored_then_checked(
        &mut self,
        statement: &ast::LetStmt,
        pat: &ast::Pat,
        value: &ast::Expr,
        context: Context<'_>,
    ) {
        let ast::Pat::IdentPat(binding) = pat else {
            return;
        };
        if binding.ref_token().is_some() || binding.pat().is_some() {
            return;
        }
        let Some(name) = binding.name().map(|name| name.text().to_string()) else {
            return;
        };
        let Some(call) = work_call(value) else {
            return;
        };
        let Some(check) = statement
            .syntax()
            .siblings(ra_ap_syntax::Direction::Next)
            .skip(1)
            .find_map(ast::Stmt::cast)
        else {
            return;
        };
        let ast::Stmt::ExprStmt(check) = check else {
            return;
        };
        if !checks_only(&check, &name)
            || plain(check.syntax()).starts_with("#[")
            || plain(statement.syntax()).starts_with("#[")
        {
            return;
        }
        // Used anywhere else after the `let`, the value is more than checked.
        let Some(block) = statement.syntax().parent() else {
            return;
        };
        let uses = block
            .descendants()
            .filter_map(ast::NameRef::cast)
            .filter(|reference| reference.text() == name.as_str())
            .filter(|reference| {
                reference.syntax().text_range().start() > statement.syntax().text_range().end()
            })
            .filter(|reference| {
                !check
                    .syntax()
                    .text_range()
                    .contains_range(reference.syntax().text_range())
            })
            .count();
        if uses > 0 {
            return;
        }
        let site = statement
            .syntax()
            .text_range()
            .cover(check.syntax().text_range());
        self.make(
            Make {
                operator: REMOVE_DISCARDED_CALL,
                node: statement.syntax(),
                site,
                caret: call.text_range(),
                replacement: String::new(),
                change: Change::Removed {
                    statement: format!("{} {}", plain(statement.syntax()), plain(check.syntax())),
                },
                operands: Vec::new(),
                form: Form::Statement,
                claimed: None,
            },
            context,
        );
    }

    /// A `let Some(..) = e` that never matches: `e` is `None`.
    pub(super) fn never_some(&mut self, pat: &ast::Pat, value: &ast::Expr, context: Context<'_>) {
        let ast::Pat::TupleStructPat(some) = pat else {
            return;
        };
        if some.path().map(|path| plain(path.syntax())).as_deref() != Some("Some") {
            return;
        }
        // `&opt` has no `filter`, and no `None` of its type can be written that outlives it.
        if matches!(value, ast::Expr::RefExpr(_)) {
            return;
        }
        // `Some(ref mut x) = self.field` binds into the field itself. Inside the switch's `match`
        // the field is a value, a copy, so a change through `x` would be lost with no mutant live.
        if is_place(value) && binds_by_reference(pat) {
            return;
        }
        let range = value.syntax().text_range();
        // Still worked out, and typed as it was: a bare `None` can leave its type unknown.
        let never = format!("{}.filter(|_| false)", receiver_text(value));
        self.make(
            Make {
                operator: REMOVE_CONDITIONAL,
                node: value.syntax(),
                site: range,
                caret: range,
                replacement: never.clone(),
                change: Change::Always {
                    condition: format!("`let {} = {}`", plain(pat.syntax()), plain(value.syntax())),
                    truth: false,
                },
                operands: Vec::new(),
                form: Form::Expression,
                claimed: None,
            },
            context,
        );
    }

    fn method_call(&mut self, call: &ast::MethodCallExpr, context: Context<'_>) {
        let Some(name) = call.name_ref() else {
            return;
        };
        let text = name.text().to_string();
        self.call_rules(call, &text, context);
        for (operator, pairs) in SWAPS {
            if let Some(after) = counterpart(pairs, &text) {
                self.swap_name(
                    operator,
                    call.syntax(),
                    name.syntax().text_range(),
                    &text,
                    after,
                    context,
                );
            }
        }
        if STAGES.contains(&text.as_str()) {
            self.stage(call, &text, REMOVE_STAGE, context);
        }
        self.more_method_call(call, &text, name.syntax().text_range(), context);
        self.rollback_method_call(call, &text, name.syntax().text_range(), context);
        self.extended_method_call(call, &text, context);
    }

    /// `cmp::min(a, b)` and `cmp::max(a, b)`, swapped; `Some(x)` made `None`.
    fn function_call(&mut self, call: &ast::CallExpr, context: Context<'_>) {
        self.some_made_none(call, context);
        self.rollback_function_call(call, context);
        self.extended_function_call(call, context);
        let Some(ast::Expr::PathExpr(path)) = call.expr() else {
            return;
        };
        let Some(segment) = path.path().and_then(|path| path.segment()) else {
            return;
        };
        let Some(name) = segment.name_ref() else {
            return;
        };
        let whole = plain(path.syntax());
        let qualifier = whole.strip_suffix(name.text().as_str()).unwrap_or_default();
        if !matches!(qualifier, "" | "cmp::" | "std::cmp::" | "core::cmp::") {
            return;
        }
        let argc = call.arg_list().map_or(0, |args| args.args().count());
        let text = name.text().to_string();
        if let (2, Some(after)) = (argc, counterpart(&[("min", "max")], &text)) {
            self.swap_name(
                SWAP_MATH,
                call.syntax(),
                name.syntax().text_range(),
                &text,
                after,
                context,
            );
        }
    }

    pub(super) fn swap_name(
        &mut self,
        operator: &'static str,
        site: &SyntaxNode,
        name: TextRange,
        before: &str,
        after: &str,
        context: Context<'_>,
    ) {
        self.make(
            Make {
                operator,
                node: site,
                site: site.text_range(),
                caret: name,
                replacement: plain_replacing(site, &[(name, after)]),
                change: Change::Swapped {
                    before: before.into(),
                    after: after.into(),
                },
                operands: Vec::new(),
                form: form_of(site),
                claimed: None,
            },
            context,
        );
    }

    /// An iterator stage left out: `xs.iter().filter(p).count()` to `xs.iter().count()`. The site is
    /// the chain up to the call that consumes it, or the whole `for` loop it feeds, so the type
    /// the stage changes is never seen.
    pub(super) fn stage(
        &mut self,
        call: &ast::MethodCallExpr,
        name: &str,
        operator: &'static str,
        context: Context<'_>,
    ) {
        let Some(receiver) = call.receiver() else {
            return;
        };
        if !visibly_iterator(&receiver) {
            return;
        }
        if matches!(name, "take" | "take_while") && unbounded(&receiver) {
            return;
        }
        let Some((site, consumer)) = consumer_of(call.syntax()) else {
            return;
        };
        if name == "rev" && ORDER_BLIND.contains(&consumer.as_str()) {
            return;
        }
        let removed = TextRange::new(
            receiver.syntax().text_range().end(),
            call.syntax().text_range().end(),
        );
        let piece: String = plain_replacing(call.syntax(), &[(receiver.syntax().text_range(), "")]);
        let caret = call
            .name_ref()
            .map_or(removed, |name| name.syntax().text_range());
        // The card's claim deletes the stage from its `.` on, so it reads on the stage's own line.
        let dot = call
            .dot_token()
            .map_or(removed.start(), |dot| dot.text_range().start());
        let deleted = usize::from(dot)..usize::from(call.syntax().text_range().end());
        self.make(
            Make {
                operator,
                node: &site,
                site: site.text_range(),
                caret,
                replacement: plain_replacing(&site, &[(removed, "")]),
                change: Change::Replaced {
                    before: piece,
                    after: String::new(),
                },
                operands: Vec::new(),
                form: form_of(&site),
                claimed: Some((deleted, String::new())),
            },
            context,
        );
    }

    /// `lo..hi` and `lo..=hi`, each to the other, where the change keeps its type: a `for` loop's
    /// range, a slice's (`&s[lo..hi]`), and the range a `contains` asks of.
    fn range_end(&mut self, range: &ast::RangeExpr, context: Context<'_>) {
        let (Some(op), Some(end)) = (range.op_token(), range.end()) else {
            return;
        };
        let after = match op.kind() {
            SyntaxKind::DOT2 => "..=",
            SyntaxKind::DOT2EQ => "..",
            _ => return,
        };
        let mut outer = range.syntax().clone();
        while let Some(parent) = outer
            .parent()
            .filter(|p| p.kind() == SyntaxKind::PAREN_EXPR)
        {
            outer = parent;
        }
        let Some(parent) = outer.parent() else {
            return;
        };
        let site = if let Some(each) = ast::ForExpr::cast(parent.clone()) {
            (each.iterable().map(|it| it.syntax().clone()) == Some(outer)).then_some(parent)
        } else if let Some(index) = ast::IndexExpr::cast(parent.clone()) {
            let borrowed = parent.parent().filter(|p| p.kind() == SyntaxKind::REF_EXPR);
            (index.index().map(|i| i.syntax().clone()) == Some(outer))
                .then_some(borrowed)
                .flatten()
        } else if let Some(call) = ast::MethodCallExpr::cast(parent.clone()) {
            let asks = call
                .name_ref()
                .is_some_and(|name| name.text() == "contains");
            (asks && call.receiver().map(|r| r.syntax().clone()) == Some(outer)).then_some(parent)
        } else {
            None
        };
        let Some(site) = site else {
            return;
        };
        let start = range
            .start()
            .map(|start| plain(start.syntax()))
            .unwrap_or_default();
        self.make(
            Make {
                operator: BOUNDARY,
                node: &site,
                site: site.text_range(),
                caret: op.text_range(),
                replacement: plain_replacing(&site, &[(op.text_range(), after)]),
                change: Change::Boundary {
                    before: op.text().to_string(),
                    after: after.into(),
                },
                operands: vec![start, plain(end.syntax())],
                form: form_of(&site),
                claimed: None,
            },
            context,
        );
    }
}

/// An expression as the receiver of a method call: in parentheses unless it binds tighter.
/// Whether `expr` names a place, memory a pattern can bind into, rather than a value: a variable,
/// a field, an element or a dereference, in parentheses or not.
fn is_place(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::PathExpr(_) | ast::Expr::FieldExpr(_) | ast::Expr::IndexExpr(_) => true,
        ast::Expr::PrefixExpr(prefix) => prefix.op_kind() == Some(ast::UnaryOp::Deref),
        ast::Expr::ParenExpr(paren) => paren.expr().is_some_and(|inner| is_place(&inner)),
        _ => false,
    }
}

/// Whether `pat` binds anything by `ref` or `ref mut`, at any depth.
fn binds_by_reference(pat: &ast::Pat) -> bool {
    pat.syntax()
        .descendants()
        .filter_map(ast::IdentPat::cast)
        .any(|binding| binding.ref_token().is_some())
}

pub(super) fn receiver_text(expr: &ast::Expr) -> String {
    let text = plain(expr.syntax());
    match expr {
        ast::Expr::PathExpr(_)
        | ast::Expr::CallExpr(_)
        | ast::Expr::MethodCallExpr(_)
        | ast::Expr::FieldExpr(_)
        | ast::Expr::ParenExpr(_)
        | ast::Expr::IndexExpr(_)
        | ast::Expr::Literal(_) => text,
        _ => format!("({text})"),
    }
}

/// `None`, `Default::default()`, `Vec::new()`: an expression whose type comes only from where it
/// is.
pub(super) fn untyped(expr: &ast::Expr) -> bool {
    let text = plain(expr.syntax());
    text == "None"
        || text.ends_with("::new()")
        || text.ends_with("::default()")
        || text.ends_with("Default::default()")
}

/// The call a statement makes, through `?` and `.await`, with the name it calls.
fn called(expr: &ast::Expr) -> Option<(SyntaxNode, String)> {
    match expr {
        ast::Expr::TryExpr(inner) => called(&inner.expr()?),
        ast::Expr::AwaitExpr(inner) => called(&inner.expr()?),
        ast::Expr::CallExpr(_) | ast::Expr::MethodCallExpr(_) => {
            let name = callee(expr.syntax())?;
            Some((expr.syntax().clone(), name))
        }
        _ => None,
    }
}

/// The call that does the work in what a `let` stores: `remove_file(&p)`, through any check or
/// view on it (`h.join().as_ref()`); none for a value it's given.
fn work_call(value: &ast::Expr) -> Option<SyntaxNode> {
    let (mut call, mut name) = called(value)?;
    while CHECK_CALLS.contains(&name.as_str()) || VIEW_CALLS.contains(&name.as_str()) {
        (call, name) = checked_value(&call).as_ref().and_then(called)?;
    }
    Some(call)
}

/// Whether `statement` only checks the value `name`: `name.unwrap();`, `name.expect("..");`, or the
/// same through a view, `name.as_ref().unwrap();`.
fn checks_only(statement: &ast::ExprStmt, name: &str) -> bool {
    if statement.semicolon_token().is_none() {
        return false;
    }
    let Some((call, check)) = statement.expr().as_ref().and_then(called) else {
        return false;
    };
    if !CHECK_CALLS.contains(&check.as_str()) {
        return false;
    }
    let mut checked = checked_value(&call);
    while let Some((call, view)) = checked.as_ref().and_then(called) {
        if !(CHECK_CALLS.contains(&view.as_str()) || VIEW_CALLS.contains(&view.as_str())) {
            return false;
        }
        checked = checked_value(&call);
    }
    matches!(&checked, Some(ast::Expr::PathExpr(path)) if plain(path.syntax()) == name)
}

/// What a check or a view is called on: `x` in `x.unwrap()` or `Option::unwrap(x)`.
fn checked_value(call: &SyntaxNode) -> Option<ast::Expr> {
    if let Some(call) = ast::MethodCallExpr::cast(call.clone()) {
        return call.receiver();
    }
    ast::CallExpr::cast(call.clone())?.arg_list()?.args().next()
}

pub(super) fn counterpart<'p>(pairs: &'p [(&'p str, &'p str)], name: &str) -> Option<&'p str> {
    pairs.iter().find_map(|(a, b)| {
        if *a == name {
            Some(*b)
        } else if *b == name {
            Some(*a)
        } else {
            None
        }
    })
}

/// Whether an expression is visibly an iterator: a chain that starts with `.iter()` or the like,
/// or a range.
pub(super) fn visibly_iterator(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::ParenExpr(inner) => inner.expr().is_some_and(|inner| visibly_iterator(&inner)),
        ast::Expr::RangeExpr(_) => true,
        ast::Expr::MethodCallExpr(call) => {
            let name = call
                .name_ref()
                .map(|n| n.text().to_string())
                .unwrap_or_default();
            ITERATOR_SOURCES.contains(&name.as_str())
                || UNBOUNDED.contains(&name.as_str())
                || (ITERATOR_ADAPTORS.contains(&name.as_str())
                    && call.receiver().is_some_and(|r| visibly_iterator(&r)))
        }
        ast::Expr::CallExpr(call) => call
            .expr()
            .map(|callee| plain(callee.syntax()))
            .is_some_and(|path| {
                let name = last_segment(&path);
                path.contains("iter::")
                    && (UNBOUNDED.contains(&name) || matches!(name, "once" | "empty"))
            }),
        _ => false,
    }
}

/// Whether an iterator's chain starts from a source that never ends: `(0..)`, `repeat`, `cycle`.
fn unbounded(expr: &ast::Expr) -> bool {
    expr.syntax()
        .descendants_with_tokens()
        .any(|element| match element {
            ra_ap_syntax::NodeOrToken::Node(node) => {
                ast::RangeExpr::cast(node).is_some_and(|range| range.end().is_none())
            }
            ra_ap_syntax::NodeOrToken::Token(token) => {
                token.kind() == SyntaxKind::IDENT && UNBOUNDED.contains(&token.text())
            }
        })
}

/// The expression a stage's chain is consumed in, with the consumer's name: the consuming call,
/// or the `for` loop the chain feeds.
fn consumer_of(stage: &SyntaxNode) -> Option<(SyntaxNode, String)> {
    let mut chain = stage.clone();
    loop {
        let parent = chain.parent()?;
        if let Some(call) = ast::MethodCallExpr::cast(parent.clone()) {
            if call.receiver().map(|r| r.syntax().clone()) != Some(chain.clone()) {
                return None;
            }
            let name = call.name_ref()?.text().to_string();
            if CONSUMERS.contains(&name.as_str()) {
                return Some((parent, name));
            }
            if !ITERATOR_ADAPTORS.contains(&name.as_str()) {
                return None;
            }
            chain = parent;
            continue;
        }
        let each = ast::ForExpr::cast(parent.clone())?;
        return (each.iterable().map(|it| it.syntax().clone()) == Some(chain))
            .then(|| (parent, "for".to_string()));
    }
}

impl Finder<'_> {
    /// A rules file's operators on a call to a method they name: by name only, as the source has no
    /// types; a rewrite that doesn't fit the receiver doesn't compile, and is withdrawn.
    fn call_rules(&mut self, call: &ast::MethodCallExpr, name: &str, context: Context<'_>) {
        let (Some(receiver), Some(name_ref)) = (call.receiver(), call.name_ref()) else {
            return;
        };
        // What a card's words name the call by: `.unwrap_or(8080)`.
        let piece = plain_replacing(call.syntax(), &[(receiver.syntax().text_range(), "")]);
        let name_range = name_ref.syntax().text_range();
        for rule in crate::callrules::loaded() {
            if !rule.methods.iter().any(|method| method == name) {
                continue;
            }
            match &rule.rewrite {
                Rewrite::Remove => self.omit_method(call, &receiver, rule.id, context),
                Rewrite::Rename(_) => {
                    if let Some(to) = rule.to(name) {
                        self.swap_name(rule.id, call.syntax(), name_range, name, &to, context);
                    }
                }
                Rewrite::KeepArgument => {
                    let args: Vec<ast::Expr> = call
                        .arg_list()
                        .map(|list| list.args().collect())
                        .unwrap_or_default();
                    // Only a call of one argument has one to keep.
                    if let [kept] = args.as_slice() {
                        // The receiver is still worked out when it could do something, as the
                        // Java edition's is.
                        let after = if super::stronger::is_pure(&receiver) {
                            receiver_text(kept)
                        } else {
                            format!(
                                "{{ let _ = {}; {} }}",
                                plain(receiver.syntax()),
                                plain(kept.syntax())
                            )
                        };
                        self.replaced(
                            rule.id,
                            call.syntax(),
                            name_range,
                            piece.clone(),
                            after,
                            context,
                        );
                    }
                }
                Rewrite::Unchecked => self.unchecked(rule.id, call, name, piece.clone(), context),
                Rewrite::Call { .. } => {
                    // From the name to the call's end: its type arguments go with its own.
                    let rest = TextRange::new(name_range.start(), call.syntax().text_range().end());
                    let to = rule.to(name).unwrap_or_default();
                    let after = plain_replacing(call.syntax(), &[(rest, &to)]);
                    self.replaced(
                        rule.id,
                        call.syntax(),
                        name_range,
                        piece.clone(),
                        after,
                        context,
                    );
                }
            }
        }
    }
}
