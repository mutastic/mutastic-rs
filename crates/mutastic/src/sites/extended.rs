//! The `extended` operators: changes that cost more, in noise or in unviable mutants, than the
//! gaps they find, on average. A function's whole body replaced, a parameter given its type's
//! default, an argument list or a regular expression changed, a loop's `break` and `continue`
//! swapped, an error no longer raised.

use super::operators::untyped;
use super::*;
use ra_ap_syntax::ast::HasArgList;
use ra_ap_syntax::TextSize;

/// Functions whose body is arid to replace: constructors, and what only formats or hashes.
const ARID_BODIES: &[&str] = &["new", "fmt", "hash"];
/// `rust_decimal`'s steps that round or rescale a value.
const DECIMAL_STEPS: &[&str] = &[
    "round_dp",
    "round_dp_with_strategy",
    "round_sf",
    "round_sf_with_strategy",
    "normalize",
    "trunc_with_scale",
];
/// Constructors of a regular expression from its text.
const REGEX_CONSTRUCTORS: &[&str] = &["Regex::new", "RegexBuilder::new", "Regex::try_from"];

impl Finder<'_> {
    /// return-value's extended forms: `Default::default()` for a type that visibly has one, and an
    /// `Err` this function returns elsewhere.
    pub(super) fn extended_returns(
        &self,
        returns: &str,
        text: &str,
        value: &ast::Expr,
    ) -> Vec<String> {
        let mut values = returned_instead(returns, text, value);
        if !self.operators.contains(&RETURN_VALUE_EXTENDED) {
            return values;
        }
        let (head, _) = generic_parts(returns);
        let head = last_segment(&head);
        if values.is_empty()
            && self.facts.default_types.contains(head)
            && !text.ends_with("default()")
        {
            values.push("Default::default()".into());
        }
        if head.ends_with("Result") {
            for error in &self.errors {
                let returned = format!("Err({error})");
                if returned != text {
                    values.push(returned);
                }
            }
        }
        values
    }

    /// The `Err` values in a body that mention nothing local, so any return in it could give one.
    pub(super) fn harvest_errors(&mut self, body: &SyntaxNode) {
        self.errors.clear();
        if !self.operators.contains(&RETURN_VALUE_EXTENDED) {
            return;
        }
        for call in body.descendants().filter_map(ast::CallExpr::cast) {
            let Some(ast::Expr::PathExpr(path)) = call.expr() else {
                continue;
            };
            if plain(path.syntax()) != "Err" {
                continue;
            }
            let args: Vec<ast::Expr> = call
                .arg_list()
                .map(|list| list.args().collect())
                .unwrap_or_default();
            let [error] = args.as_slice() else {
                continue;
            };
            let text = plain(error.syntax());
            if closed(error) && !self.errors.contains(&text) {
                self.errors.push(text);
            }
        }
    }

    /// A function's whole body to returning one value (return-value's), or to nothing.
    pub(super) fn replace_body(
        &mut self,
        function: &ast::Fn,
        body: &ast::BlockExpr,
        returns: &str,
        context: Context<'_>,
    ) {
        let name = &self.found.functions[context.function].name;
        if ARID_BODIES.contains(&name.as_str())
            || trivial_getter(body)
            || body.async_token().is_some()
        {
            return;
        }
        let values = if matches!(returns, "()" | "") {
            vec![String::new()]
        } else {
            inner_values(returns, "")
        };
        let original = plain(body.syntax());
        for value in values {
            if original == format!("{{ {value} }}") || (value.is_empty() && original == "{}") {
                continue;
            }
            let caret = function
                .name()
                .map_or(body.syntax().text_range(), |n| n.syntax().text_range());
            self.make(
                Make {
                    operator: REPLACE_BODY,
                    node: body.syntax(),
                    site: body.syntax().text_range(),
                    caret,
                    replacement: value.clone(),
                    change: Change::Body { value },
                    operands: Vec::new(),
                    form: Form::Body,
                    claimed: None,
                },
                context,
            );
        }
    }

    /// A parameter given its type's default as the function starts: `let p: T = Default::default();`.
    pub(super) fn default_parameters(&mut self, function: &ast::Fn, context: Context<'_>) {
        let Some(list) = function.param_list() else {
            return;
        };
        let body_open = self.found.functions[context.function].body_open;
        for param in list.params() {
            let (Some(ast::Pat::IdentPat(pat)), Some(ty)) = (param.pat(), param.ty()) else {
                continue;
            };
            let ty = plain(ty.syntax());
            // A reference, a trait, a closure have no default; an `Option` is null-parameter's.
            if ty.starts_with('&')
                || ty.starts_with("impl ")
                || ty.starts_with("dyn ")
                || ty.starts_with("Option<")
                || ty.contains("Fn(")
            {
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
            self.make(
                Make {
                    operator: DEFAULT_PARAMETER,
                    node: param.syntax(),
                    site: TextRange::empty(TextSize::from(body_open as u32)),
                    caret: pat.syntax().text_range(),
                    replacement: format!(" let {binding}: {ty} = Default::default();"),
                    change: Change::Parameter {
                        name: name.clone(),
                        value: "Default::default()".into(),
                    },
                    operands: Vec::new(),
                    form: Form::Insert {
                        text: format!(
                            " let {binding}: {ty} = match crate::__mutastic::at({{n}}) {{ {{n}} => Default::default(), _ => {name} }};"
                        ),
                    },
                    claimed: None,
                },
                context,
            );
        }
    }

    /// `break` and `continue`, each to the other, when they name no loop and carry no value.
    pub(super) fn swap_break(&mut self, node: &SyntaxNode, context: Context<'_>) {
        let (before, after, token) = if let Some(brk) = ast::BreakExpr::cast(node.clone()) {
            if brk.lifetime().is_some() || brk.expr().is_some() {
                return;
            }
            ("break", "continue", brk.break_token())
        } else if let Some(cont) = ast::ContinueExpr::cast(node.clone()) {
            if cont.lifetime().is_some() {
                return;
            }
            ("continue", "break", cont.continue_token())
        } else {
            return;
        };
        let Some(token) = token else {
            return;
        };
        self.make(
            Make {
                operator: SWAP_BREAK,
                node,
                site: node.text_range(),
                caret: token.text_range(),
                replacement: after.into(),
                change: Change::Swapped {
                    before: before.into(),
                    after: after.into(),
                },
                operands: Vec::new(),
                form: form_of(node),
                claimed: None,
            },
            context,
        );
    }

    /// `return Err(e);`, `panic!(..);` and `bail!(..);` removed: the code carries on.
    pub(super) fn remove_throw(&mut self, statement: &ast::ExprStmt, context: Context<'_>) {
        let Some(expr) = statement.expr() else {
            return;
        };
        let raises = match &expr {
            ast::Expr::ReturnExpr(ret) => ret.expr().is_some_and(|value| {
                matches!(&value, ast::Expr::CallExpr(call)
                    if call.expr().is_some_and(|f| plain(f.syntax()) == "Err"))
            }),
            ast::Expr::MacroExpr(call) => call
                .macro_call()
                .and_then(|call| call.path())
                .is_some_and(|path| {
                    matches!(last_segment(&plain(path.syntax())), "panic" | "bail")
                }),
            _ => false,
        };
        if !raises {
            return;
        }
        self.make(
            Make {
                operator: REMOVE_THROW,
                node: statement.syntax(),
                site: statement.syntax().text_range(),
                caret: expr.syntax().text_range(),
                replacement: String::new(),
                change: Change::Removed {
                    statement: plain(statement.syntax()),
                },
                operands: Vec::new(),
                form: Form::Statement,
                claimed: None,
            },
            context,
        );
    }

    /// `.inspect(f)` and `.map(f)` left out of an iterator's chain; `opt.filter(p)` to `opt`; a
    /// decimal's rounding step left out.
    pub(super) fn extended_method_call(
        &mut self,
        call: &ast::MethodCallExpr,
        name: &str,
        context: Context<'_>,
    ) {
        if matches!(name, "inspect" | "map") {
            self.stage(call, name, REMOVE_OTHER_STAGE, context);
        }
        let Some(receiver) = call.receiver() else {
            return;
        };
        let argc = call.arg_list().map_or(0, |list| list.args().count());
        if name == "filter" && argc == 1 && !super::operators::visibly_iterator(&receiver) {
            self.omit_method(call, &receiver, REMOVE_FILTER, context);
        }
        if DECIMAL_STEPS.contains(&name) {
            self.omit_method(call, &receiver, REMOVE_STEP, context);
        }
        let args: Vec<ast::Expr> = call
            .arg_list()
            .map(|list| list.args().collect())
            .unwrap_or_default();
        self.drop_vararg(call.syntax(), &args, context);
    }

    /// A regular expression changed, and the last value of an array built for a call left out.
    pub(super) fn extended_function_call(&mut self, call: &ast::CallExpr, context: Context<'_>) {
        let args: Vec<ast::Expr> = call
            .arg_list()
            .map(|list| list.args().collect())
            .unwrap_or_default();
        let callee_path = call.expr().map(|f| plain(f.syntax())).unwrap_or_default();
        let constructs_regex = REGEX_CONSTRUCTORS.iter().any(|constructor| {
            callee_path == *constructor || callee_path.ends_with(&format!("::{constructor}"))
        });
        if let (true, Some(ast::Expr::Literal(literal))) = (constructs_regex, args.first()) {
            self.regex_constant(literal, context);
        }
        self.drop_vararg(call.syntax(), &args, context);
    }

    /// `^\d+$` to `\d+$`, `^\d+`, and `^\D+$`: anchors dropped and a class turned round, each of
    /// which is still a valid expression.
    fn regex_constant(&mut self, literal: &ast::Literal, context: Context<'_>) {
        let token = literal.token();
        if token.kind() != SyntaxKind::STRING {
            return;
        }
        let text = token.text();
        let (Some(open), Some(close)) = (text.find('"'), text.rfind('"')) else {
            return;
        };
        if close <= open {
            return;
        }
        let (prefix, body, suffix) = (&text[..=open], &text[open + 1..close], &text[close..]);
        let mut variants = Vec::new();
        if let Some(rest) = body.strip_prefix('^') {
            variants.push(rest.to_string());
        }
        if let Some(rest) = body.strip_suffix('$') {
            if !rest.ends_with('\\') {
                variants.push(rest.to_string());
            }
        }
        for (class, other) in [
            ("\\d", "\\D"),
            ("\\w", "\\W"),
            ("\\s", "\\S"),
            ("\\D", "\\d"),
            ("\\W", "\\w"),
            ("\\S", "\\s"),
        ] {
            if let Some(at) = body.find(class) {
                variants.push(format!(
                    "{}{other}{}",
                    &body[..at],
                    &body[at + class.len()..]
                ));
            }
        }
        for variant in variants {
            let after = format!("{prefix}{variant}{suffix}");
            self.make(
                Make {
                    operator: REGEX_CONSTANT,
                    node: literal.syntax(),
                    site: literal.syntax().text_range(),
                    caret: literal.syntax().text_range(),
                    replacement: after.clone(),
                    change: Change::Replaced {
                        before: text.to_string(),
                        after,
                    },
                    operands: Vec::new(),
                    form: Form::Expression,
                    claimed: None,
                },
                context,
            );
        }
    }

    /// The last value of an array or `vec!` built as a call's argument, left out. The site is the
    /// whole call, so an array's new length is never seen outside it.
    fn drop_vararg(&mut self, call: &SyntaxNode, args: &[ast::Expr], context: Context<'_>) {
        for arg in args {
            let Some(removed) = last_element(arg) else {
                continue;
            };
            let text = call.text().to_string();
            let offset = |at: TextSize| usize::from(at - call.text_range().start());
            let piece = text
                .get(offset(removed.start())..offset(removed.end()))
                .unwrap_or_default()
                .trim_start_matches(',')
                .trim()
                .to_string();
            self.make(
                Make {
                    operator: DROP_VARARG,
                    node: call,
                    site: call.text_range(),
                    caret: removed,
                    replacement: plain_replacing(call, &[(removed, "")]),
                    change: Change::Replaced {
                        before: piece,
                        after: String::new(),
                    },
                    operands: Vec::new(),
                    form: form_of(call),
                    claimed: Some((
                        usize::from(removed.start())..usize::from(removed.end()),
                        String::new(),
                    )),
                },
                context,
            );
        }
    }
}

/// The range of an argument's last array element, with the comma before it: `, c` in `&[a, b, c]`
/// or `vec![a, b, c]`. `None` for anything else, or an empty one.
fn last_element(arg: &ast::Expr) -> Option<TextRange> {
    let array = match arg {
        ast::Expr::RefExpr(reference) => match reference.expr()? {
            ast::Expr::ArrayExpr(array) if array.semicolon_token().is_none() => array,
            _ => return None,
        },
        ast::Expr::MacroExpr(call) => {
            let call = call.macro_call()?;
            if plain(call.path()?.syntax()) != "vec" {
                return None;
            }
            return last_in_tokens(&call.token_tree()?);
        }
        _ => return None,
    };
    let elements: Vec<ast::Expr> = array.exprs().collect();
    let last = elements.last()?;
    let start = match elements.len() {
        1 => last.syntax().text_range().start(),
        n => elements[n - 2].syntax().text_range().end(),
    };
    Some(TextRange::new(start, last.syntax().text_range().end()))
}

/// The last top-level element of a `vec![..]`'s tokens, with the comma before it.
fn last_in_tokens(tree: &ast::TokenTree) -> Option<TextRange> {
    let inner: Vec<ra_ap_syntax::SyntaxElement> = tree
        .syntax()
        .children_with_tokens()
        .filter(|element| {
            !matches!(
                element.kind(),
                SyntaxKind::L_BRACK
                    | SyntaxKind::R_BRACK
                    | SyntaxKind::WHITESPACE
                    | SyntaxKind::COMMENT
            )
        })
        .collect();
    if inner
        .iter()
        .any(|element| element.kind() == SyntaxKind::SEMICOLON)
    {
        return None;
    }
    let mut items = inner.as_slice();
    if items
        .last()
        .is_some_and(|last| last.kind() == SyntaxKind::COMMA)
    {
        items = &items[..items.len() - 1];
    }
    let end = items.last()?.text_range().end();
    match items
        .iter()
        .rposition(|element| element.kind() == SyntaxKind::COMMA)
    {
        Some(comma) => {
            let before = items.get(comma.checked_sub(1)?)?;
            Some(TextRange::new(before.text_range().end(), end))
        }
        None => Some(TextRange::new(items.first()?.text_range().start(), end)),
    }
}

/// A body that only hands back a field: `self.name`, `&self.name`, `self.name.clone()`.
fn trivial_getter(body: &ast::BlockExpr) -> bool {
    let Some(list) = body.stmt_list() else {
        return false;
    };
    if list.statements().next().is_some() {
        return false;
    }
    let Some(tail) = list.tail_expr() else {
        return false;
    };
    let text = plain(tail.syntax());
    let field = text.trim_start_matches('&').trim_end_matches(".clone()");
    field.starts_with("self.") && field[5..].chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Whether an expression mentions nothing local: every path in it is qualified, or names a type,
/// constant or variant.
fn closed(expr: &ast::Expr) -> bool {
    if untyped(expr) {
        return false;
    }
    std::iter::once(expr.syntax().clone())
        .chain(expr.syntax().descendants())
        .all(|node| {
            if ast::ClosureExpr::can_cast(node.kind()) {
                return false;
            }
            match ast::PathExpr::cast(node) {
                Some(path) => {
                    let text = plain(path.syntax());
                    text.contains("::") || text.starts_with(|c: char| c.is_uppercase())
                }
                None => true,
            }
        })
}
