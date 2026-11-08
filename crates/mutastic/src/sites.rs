//! Finding mutation sites in a crate's source, with rust-analyzer's parser.
//!
//! Only function bodies are mutated, and only where a runtime switch can stand: never in a `const
//! fn`, a type, a pattern, a const argument, an array's length, a nested item or a macro's tokens.
//! Test code (`#[test]` functions and `#[cfg(test)]` modules) is never mutated.

use std::collections::BTreeMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

use ra_ap_syntax::ast::{self, ArithOp, BinaryOp, CmpOp, HasAttrs, HasName, LogicOp, Ordering};
use ra_ap_syntax::{AstNode, Edition, SourceFile, SyntaxKind, SyntaxNode, TextRange};

use crate::id;

/// The operators, by the ids the Java edition uses.
pub const BOUNDARY: &str = "mutastic/boundary";
pub const REMOVE_CONDITIONAL: &str = "mutastic/remove-conditional";
pub const LOGICAL_CONNECTOR: &str = "mutastic/logical-connector";
pub const MATH: &str = "mutastic/math";
pub const OTHER_COMPARISON: &str = "mutastic/other-comparison";
pub const RETURN_VALUE: &str = "mutastic/return-value";

/// The operators a run uses when none are named: the Java edition's `defaults`, as far as built.
pub const DEFAULTS: &[&str] = &[
    BOUNDARY,
    REMOVE_CONDITIONAL,
    LOGICAL_CONNECTOR,
    MATH,
    RETURN_VALUE,
];
/// `stronger` adds the condition reversed (`==` to `!=`, `<` to `>=`).
pub const STRONGER: &[&str] = &[
    BOUNDARY,
    REMOVE_CONDITIONAL,
    LOGICAL_CONNECTOR,
    MATH,
    RETURN_VALUE,
    OTHER_COMPARISON,
];

/// What a mutant changes, with what the card needs to say about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// The edge of a comparison moved: `<` to `<=`.
    Boundary { before: String, after: String },
    /// A comparison reversed: `<` to `>=`, `==` to `!=`.
    Reversed { before: String, after: String },
    /// A condition that always holds, or never does.
    Always { condition: String, truth: bool },
    /// `&&` and `||`.
    Connector { before: String, after: String },
    /// An arithmetic operator to another.
    Math { before: String, after: String },
    /// A returned value to another.
    Returns { value: String },
}

/// One mutant: a change at one site, behind its own switch.
#[derive(Clone, Debug)]
pub struct Mutant {
    /// The switch's number at run time, from 1.
    pub ordinal: u32,
    pub id: String,
    pub fingerprint: String,
    pub operator: &'static str,
    pub change: Change,
    /// The file, relative to the workspace root.
    pub file: PathBuf,
    /// Bytes of the site in the original file.
    pub site: Range<usize>,
    /// Bytes to put a caret under, in the original file.
    pub caret: Range<usize>,
    /// The code that replaces the site when this mutant is live.
    pub replacement: String,
    /// The two sides of a binary operator, as written (whitespace collapsed).
    pub operands: Vec<String>,
    pub line: usize,
    pub column: usize,
    /// Index into `Crate::functions`.
    pub function: usize,
}

impl Mutant {
    /// The change the card claims, as an edit to the original text: the bytes to replace and
    /// what with. A truth test writes this into the source and checks the mutant behaves the same.
    pub fn claim(&self) -> (Range<usize>, String) {
        match &self.change {
            Change::Boundary { after, .. }
            | Change::Reversed { after, .. }
            | Change::Connector { after, .. }
            | Change::Math { after, .. } => (self.caret.clone(), after.clone()),
            Change::Always { truth, .. } => (self.site.clone(), truth.to_string()),
            Change::Returns { value } => (self.site.clone(), value.clone()),
        }
    }
}

/// A function with mutants in it.
#[derive(Clone, Debug)]
pub struct Function {
    pub ordinal: u32,
    /// `discount`, for the cards.
    pub name: String,
    /// `pricing::Pricing::discount`.
    pub path: String,
    /// The module, `pricing`.
    pub module: String,
    pub file: PathBuf,
    /// The function's bytes in the original file.
    pub range: Range<usize>,
    /// Where a statement can go at the top of its body.
    pub body_open: usize,
    /// Its whole-number parameters, by name and type, for "inputs seen".
    pub numbers: Vec<(String, String)>,
}

/// A source file and what was found in it.
#[derive(Clone, Debug)]
pub struct File {
    pub path: PathBuf,
    pub text: String,
    /// Lines left unmutated because the parser couldn't vouch for them.
    pub unparsed: Vec<(usize, usize)>,
}

/// Everything found in one crate.
#[derive(Clone, Debug, Default)]
pub struct Crate {
    pub files: Vec<File>,
    pub functions: Vec<Function>,
    pub mutants: Vec<Mutant>,
}

const WHOLE_NUMBERS: &[&str] = &[
    "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
];

/// Finds every mutant in the files reachable from `roots`, which are relative to `workspace`.
pub fn find(workspace: &Path, crate_name: &str, roots: &[PathBuf], operators: &[&str]) -> Crate {
    let mut found = Crate::default();
    let mut seen = std::collections::BTreeSet::new();
    let mut queue: Vec<(PathBuf, String, bool)> = roots
        .iter()
        .map(|root| (root.clone(), String::new(), true))
        .collect();
    while let Some((file, module, is_root)) = queue.pop() {
        if !seen.insert(file.clone()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(workspace.join(&file)) else {
            continue;
        };
        let parse = SourceFile::parse(&text, Edition::CURRENT);
        let tree = parse.tree();
        for (child, child_module) in child_modules(&file, &module, is_root, &tree) {
            queue.push((child, child_module, false));
        }
        let damaged = damaged_items(&tree, parse.errors().iter().map(|error| error.range()));
        let mut finder = Finder {
            text: &text,
            lines: line_starts(&text),
            file: &file,
            module: &module,
            crate_name,
            operators,
            damaged: &damaged,
            found: &mut found,
            nth: BTreeMap::new(),
        };
        for function in tree.syntax().descendants().filter_map(ast::Fn::cast) {
            finder.function(&function);
        }
        let lines = line_starts(&text);
        let unparsed = damaged
            .iter()
            .map(|range| {
                (
                    line_of(&lines, usize::from(range.start())),
                    line_of(&lines, usize::from(range.end())),
                )
            })
            .collect();
        found.files.push(File {
            path: file,
            text,
            unparsed,
        });
    }
    found.files.sort_by(|a, b| a.path.cmp(&b.path));
    found
}

/// The files of `mod name;` declarations in `tree`, with their module paths.
fn child_modules(
    file: &Path,
    module: &str,
    is_root: bool,
    tree: &SourceFile,
) -> Vec<(PathBuf, String)> {
    let dir = match file.parent() {
        Some(parent) if is_root || file.file_name().is_some_and(|name| name == "mod.rs") => {
            parent.to_path_buf()
        }
        Some(parent) => parent.join(file.file_stem().unwrap_or_default()),
        None => PathBuf::new(),
    };
    let mut children = Vec::new();
    for declared in tree.syntax().descendants().filter_map(ast::Module::cast) {
        if declared.item_list().is_some() || is_test_only(declared.syntax()) {
            continue;
        }
        let Some(name) = declared.name().map(|name| name.text().to_string()) else {
            continue;
        };
        let inline: Vec<String> = declared
            .syntax()
            .ancestors()
            .skip(1)
            .filter_map(ast::Module::cast)
            .filter_map(|outer| outer.name().map(|name| name.text().to_string()))
            .collect();
        let mut base = dir.clone();
        for outer in inline.iter().rev() {
            base.push(outer);
        }
        let path = path_attribute(&declared)
            .map(|path| base.join(path))
            .unwrap_or_else(|| {
                let flat = base.join(format!("{name}.rs"));
                if flat.exists() || !base.join(&name).join("mod.rs").exists() {
                    flat
                } else {
                    base.join(&name).join("mod.rs")
                }
            });
        let mut parts: Vec<String> = module
            .split("::")
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect();
        parts.extend(inline.into_iter().rev());
        parts.push(name);
        children.push((path, parts.join("::")));
    }
    children
}

fn path_attribute(module: &ast::Module) -> Option<String> {
    module.attrs().find_map(|attr| {
        let text = attr.syntax().text().to_string();
        let inner = text.strip_prefix("#[")?.strip_suffix(']')?.trim();
        let value = inner.strip_prefix("path")?.trim().strip_prefix('=')?.trim();
        Some(value.trim_matches('"').to_string())
    })
}

/// Top-level items holding a parse error: recovery can distort the tree around one, so the whole
/// item is left alone.
fn damaged_items(tree: &SourceFile, errors: impl Iterator<Item = TextRange>) -> Vec<TextRange> {
    let mut damaged: Vec<TextRange> = errors
        .map(|error| {
            tree.syntax()
                .children()
                .map(|item| item.text_range())
                .find(|item| item.contains_range(error))
                .unwrap_or(error)
        })
        .collect();
    damaged.sort_by_key(|range| (range.start(), range.end()));
    damaged.dedup();
    damaged
}

/// Whether a node is test code: a `#[test]`-like function, or under `#[cfg(test)]`.
fn is_test_only(node: &SyntaxNode) -> bool {
    node.ancestors().any(|ancestor| {
        let Some(item) = ast::AnyHasAttrs::cast(ancestor) else {
            return false;
        };
        item.attrs().any(|attr| {
            let text: String = attr
                .syntax()
                .text()
                .to_string()
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            text == "#[test]"
                || text.ends_with("::test]")
                || text.contains("cfg(test)")
                || text.starts_with("#[bench]")
        })
    })
}

pub fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(at, _)| at + 1))
        .collect()
}

/// The 1-based line holding `offset`.
pub fn line_of(starts: &[usize], offset: usize) -> usize {
    starts.partition_point(|&start| start <= offset)
}

/// A node's text with whitespace and comments collapsed to single spaces, so a copy of it never
/// adds a line.
pub fn plain(node: &SyntaxNode) -> String {
    let mut out = String::new();
    for token in node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        match token.kind() {
            SyntaxKind::WHITESPACE | SyntaxKind::COMMENT => {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            _ => out.push_str(token.text()),
        }
    }
    out.trim().to_string()
}

struct Finder<'a> {
    text: &'a str,
    lines: Vec<usize>,
    file: &'a Path,
    module: &'a str,
    crate_name: &'a str,
    operators: &'a [&'a str],
    damaged: &'a [TextRange],
    found: &'a mut Crate,
    /// How many identical sites each function has had so far, for ids.
    nth: BTreeMap<String, u32>,
}

/// What the walk knows about where it is.
#[derive(Clone, Copy)]
struct Context<'r> {
    function: usize,
    returns: Option<&'r str>,
    in_closure: bool,
}

impl Finder<'_> {
    fn function(&mut self, function: &ast::Fn) {
        let range = function.syntax().text_range();
        if function.const_token().is_some()
            || is_test_only(function.syntax())
            || self
                .damaged
                .iter()
                .any(|damaged| damaged.contains_range(range))
        {
            return;
        }
        let Some(body) = function.body() else {
            return;
        };
        let Some(name) = function.name().map(|name| name.text().to_string()) else {
            return;
        };
        let returns = function
            .ret_type()
            .and_then(|ret| ret.ty())
            .map(|ty| plain(ty.syntax()));
        let numbers = function
            .param_list()
            .map(|list| {
                list.params()
                    .filter_map(|param| {
                        let ast::Pat::IdentPat(pat) = param.pat()? else {
                            return None;
                        };
                        let ty = plain(param.ty()?.syntax());
                        let name = pat.name()?.text().to_string();
                        WHOLE_NUMBERS.contains(&ty.as_str()).then_some((name, ty))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let index = self.found.functions.len();
        let body_open = body
            .stmt_list()
            .and_then(|list| list.l_curly_token())
            .map(|curly| usize::from(curly.text_range().end()))
            .unwrap_or_else(|| usize::from(body.syntax().text_range().start()) + 1);
        self.found.functions.push(Function {
            ordinal: index as u32 + 1,
            name: name.clone(),
            path: item_path(self.module, function, &name),
            module: self.module.to_string(),
            file: self.file.to_path_buf(),
            range: usize::from(range.start())..usize::from(range.end()),
            body_open,
            numbers,
        });
        let context = Context {
            function: index,
            returns: returns.as_deref(),
            in_closure: false,
        };
        if let Some(returns) = context.returns {
            if let Some(tail) = body.tail_expr() {
                self.returned(&tail, returns, context);
            }
        }
        self.walk(body.syntax(), context);
    }

    fn walk(&mut self, node: &SyntaxNode, context: Context<'_>) {
        if ast::Item::can_cast(node.kind())
            || ast::Type::can_cast(node.kind())
            || ast::Pat::can_cast(node.kind())
            || matches!(
                node.kind(),
                SyntaxKind::CONST_ARG | SyntaxKind::MACRO_CALL | SyntaxKind::ATTR
            )
        {
            return;
        }
        let mut context = context;
        if let Some(block) = ast::BlockExpr::cast(node.clone()) {
            if block.const_token().is_some() {
                return;
            }
        }
        if ast::ClosureExpr::can_cast(node.kind()) {
            context.in_closure = true;
        }
        if let Some(expr) = ast::BinExpr::cast(node.clone()) {
            self.binary(&expr, context);
        }
        if let Some(expr) = ast::IfExpr::cast(node.clone()) {
            if let Some(condition) = expr.condition() {
                self.condition(&condition, context);
            }
        }
        if let Some(expr) = ast::WhileExpr::cast(node.clone()) {
            if let Some(condition) = expr.condition() {
                self.condition(&condition, context);
            }
        }
        if let Some(expr) = ast::ReturnExpr::cast(node.clone()) {
            if let (Some(value), Some(returns), false) =
                (expr.expr(), context.returns, context.in_closure)
            {
                self.returned(&value, returns, context);
            }
        }
        if let Some(array) = ast::ArrayExpr::cast(node.clone()) {
            // `[x; N]`: the length is a constant.
            if array.semicolon_token().is_some() {
                if let Some(first) = array.syntax().children().next() {
                    self.walk(&first, context);
                }
                return;
            }
        }
        for child in node.children() {
            self.walk(&child, context);
        }
    }

    /// A condition that isn't a comparison or a connector (`if order.is_paid()`): always true,
    /// or always false. Comparisons get theirs as binary expressions, wherever they are.
    fn condition(&mut self, condition: &ast::Expr, context: Context<'_>) {
        match condition {
            ast::Expr::ParenExpr(paren) => {
                if let Some(inner) = paren.expr() {
                    self.condition(&inner, context);
                }
            }
            ast::Expr::PrefixExpr(prefix) if prefix.op_kind() == Some(ast::UnaryOp::Not) => {
                if let Some(inner) = prefix.expr() {
                    self.condition(&inner, context);
                }
            }
            ast::Expr::BinExpr(binary) => {
                if let Some(BinaryOp::LogicOp(_)) = binary.op_kind() {
                    for side in [binary.lhs(), binary.rhs()].into_iter().flatten() {
                        self.condition(&side, context);
                    }
                }
            }
            ast::Expr::LetExpr(_) | ast::Expr::Literal(_) | ast::Expr::BlockExpr(_) => {}
            other => {
                let text = plain(other.syntax());
                for truth in [true, false] {
                    self.add(
                        REMOVE_CONDITIONAL,
                        other.syntax(),
                        other.syntax().text_range(),
                        truth.to_string(),
                        Change::Always {
                            condition: format!("`{text}`"),
                            truth,
                        },
                        Vec::new(),
                        context,
                    );
                }
            }
        }
    }

    fn binary(&mut self, expr: &ast::BinExpr, context: Context<'_>) {
        let (Some(op), Some(token), Some(lhs), Some(rhs)) =
            (expr.op_kind(), expr.op_token(), expr.lhs(), expr.rhs())
        else {
            return;
        };
        let before = token.text().to_string();
        let operands = vec![plain(lhs.syntax()), plain(rhs.syntax())];
        let caret = token.text_range();
        let swap = |after: &str| format!("({}) {after} ({})", operands[0], operands[1]);
        match op {
            BinaryOp::CmpOp(cmp) => {
                if let CmpOp::Ord { ordering, strict } = cmp {
                    let after = match (ordering, strict) {
                        (Ordering::Less, true) => "<=",
                        (Ordering::Less, false) => "<",
                        (Ordering::Greater, true) => ">=",
                        (Ordering::Greater, false) => ">",
                    };
                    self.add(
                        BOUNDARY,
                        expr.syntax(),
                        caret,
                        swap(after),
                        Change::Boundary {
                            before: before.clone(),
                            after: after.into(),
                        },
                        operands.clone(),
                        context,
                    );
                }
                let condition = format!("`{} {before} {}`", operands[0], operands[1]);
                for truth in [true, false] {
                    self.add(
                        REMOVE_CONDITIONAL,
                        expr.syntax(),
                        expr.syntax().text_range(),
                        truth.to_string(),
                        Change::Always {
                            condition: condition.clone(),
                            truth,
                        },
                        operands.clone(),
                        context,
                    );
                }
                let reversed = match cmp {
                    CmpOp::Eq { negated: false } => "!=",
                    CmpOp::Eq { negated: true } => "==",
                    CmpOp::Ord {
                        ordering: Ordering::Less,
                        strict: true,
                    } => ">=",
                    CmpOp::Ord {
                        ordering: Ordering::Less,
                        strict: false,
                    } => ">",
                    CmpOp::Ord {
                        ordering: Ordering::Greater,
                        strict: true,
                    } => "<=",
                    CmpOp::Ord {
                        ordering: Ordering::Greater,
                        strict: false,
                    } => "<",
                };
                self.add(
                    OTHER_COMPARISON,
                    expr.syntax(),
                    caret,
                    swap(reversed),
                    Change::Reversed {
                        before,
                        after: reversed.into(),
                    },
                    operands,
                    context,
                );
            }
            BinaryOp::LogicOp(logic) => {
                if expr
                    .syntax()
                    .descendants()
                    .any(|node| ast::LetExpr::can_cast(node.kind()))
                {
                    return;
                }
                let after = match logic {
                    LogicOp::And => "||",
                    LogicOp::Or => "&&",
                };
                self.add(
                    LOGICAL_CONNECTOR,
                    expr.syntax(),
                    caret,
                    swap(after),
                    Change::Connector {
                        before,
                        after: after.into(),
                    },
                    operands,
                    context,
                );
            }
            BinaryOp::ArithOp(arith) => {
                if let Some(after) = math(arith) {
                    self.add(
                        MATH,
                        expr.syntax(),
                        caret,
                        swap(after),
                        Change::Math {
                            before,
                            after: after.into(),
                        },
                        operands,
                        context,
                    );
                }
            }
            BinaryOp::Assignment { op: Some(arith) } => {
                if let Some(after) = math(arith) {
                    let after = format!("{after}=");
                    self.add(
                        MATH,
                        expr.syntax(),
                        caret,
                        format!("{} {after} ({})", operands[0], operands[1]),
                        Change::Math { before, after },
                        operands,
                        context,
                    );
                }
            }
            BinaryOp::Assignment { op: None } => {}
        }
    }

    /// A value a function returns: by `return`, or as the tail of its body, an `if`'s branches or
    /// a `match`'s arms.
    fn returned(&mut self, value: &ast::Expr, returns: &str, context: Context<'_>) {
        match value {
            ast::Expr::BlockExpr(block) => {
                if block.unsafe_token().is_some()
                    || block.label().is_some()
                    || block.const_token().is_some()
                {
                    return;
                }
                if let Some(tail) = block.tail_expr() {
                    self.returned(&tail, returns, context);
                }
            }
            ast::Expr::IfExpr(expr) => {
                if expr.else_branch().is_none() {
                    return;
                }
                if let Some(then) = expr.then_branch() {
                    if let Some(tail) = then.tail_expr() {
                        self.returned(&tail, returns, context);
                    }
                }
                match expr.else_branch() {
                    Some(ast::ElseBranch::Block(block)) => {
                        if let Some(tail) = block.tail_expr() {
                            self.returned(&tail, returns, context);
                        }
                    }
                    Some(ast::ElseBranch::IfExpr(elif)) => {
                        self.returned(&ast::Expr::IfExpr(elif), returns, context)
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
                        self.returned(&tail, returns, context);
                    }
                }
            }
            ast::Expr::MacroExpr(call) if diverges(call) => {}
            ast::Expr::ReturnExpr(_)
            | ast::Expr::LoopExpr(_)
            | ast::Expr::WhileExpr(_)
            | ast::Expr::ForExpr(_)
            | ast::Expr::BreakExpr(_)
            | ast::Expr::ContinueExpr(_) => {}
            other => {
                let text = plain(other.syntax());
                for replacement in returned_instead(returns, &text, other) {
                    self.add(
                        RETURN_VALUE,
                        other.syntax(),
                        other.syntax().text_range(),
                        replacement.clone(),
                        Change::Returns { value: replacement },
                        Vec::new(),
                        context,
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add(
        &mut self,
        operator: &'static str,
        node: &SyntaxNode,
        caret: TextRange,
        replacement: String,
        change: Change,
        operands: Vec<String>,
        context: Context<'_>,
    ) {
        if !self.operators.contains(&operator) {
            return;
        }
        let range = node.text_range();
        let function = &self.found.functions[context.function];
        let original = plain(node);
        let key = format!("{}\0{operator}\0{original}\0{replacement}", function.path);
        let nth = self.nth.entry(key).or_insert(0);
        let (id, fingerprint) = id::of(&[
            self.crate_name,
            &function.path,
            operator,
            &nth.to_string(),
            &original,
            &replacement,
        ]);
        *nth += 1;
        let start = usize::from(range.start());
        let line = line_of(&self.lines, start);
        let caret_start = usize::from(caret.start());
        let column = self.text[self.lines[line - 1]..caret_start].chars().count() + 1;
        let ordinal = self.found.mutants.len() as u32 + 1;
        self.found.mutants.push(Mutant {
            ordinal,
            id,
            fingerprint,
            operator,
            change,
            file: self.file.to_path_buf(),
            site: start..usize::from(range.end()),
            caret: caret_start..usize::from(caret.end()),
            replacement,
            operands,
            line,
            column,
            function: context.function,
        });
    }
}

/// `panic!`, `unreachable!`, `todo!` and `unimplemented!` return nothing to change.
fn diverges(call: &ast::MacroExpr) -> bool {
    let name = call
        .macro_call()
        .and_then(|call| call.path())
        .and_then(|path| path.segment())
        .map(|segment| segment.syntax().text().to_string())
        .unwrap_or_default();
    matches!(
        name.as_str(),
        "panic" | "unreachable" | "todo" | "unimplemented"
    )
}

fn math(op: ArithOp) -> Option<&'static str> {
    match op {
        ArithOp::Add => Some("-"),
        ArithOp::Sub => Some("+"),
        ArithOp::Mul => Some("/"),
        ArithOp::Div => Some("*"),
        ArithOp::Rem => Some("*"),
        _ => None,
    }
}

/// What a return gives instead, by the function's declared type: `0` for a number (`1` if it's
/// `0` already), the other `bool`, `None`, or an empty string or vector.
fn returned_instead(returns: &str, text: &str, value: &ast::Expr) -> Vec<String> {
    // `0`, `0u32`, `0.0`, `0_f64`: a literal whose digits are all zero.
    let digits: String = text
        .split(|c: char| c.is_ascii_alphabetic())
        .next()
        .unwrap_or_default()
        .replace('_', "");
    let is_zero = matches!(value, ast::Expr::Literal(_))
        && !digits.is_empty()
        && digits.chars().all(|c| c == '0' || c == '.');
    let base = returns.trim();
    if WHOLE_NUMBERS.contains(&base) {
        return vec![if is_zero { "1".into() } else { "0".into() }];
    }
    match base {
        "f32" | "f64" => vec![if is_zero { "1.0".into() } else { "0.0".into() }],
        "bool" => match text {
            "true" => vec!["false".into()],
            "false" => vec!["true".into()],
            _ => vec!["true".into(), "false".into()],
        },
        "String" if !matches!(text, "String::new()" | "String::default()") => {
            vec!["String::new()".into()]
        }
        "&str" | "&'static str" if text != "\"\"" => vec!["\"\"".into()],
        _ if base.starts_with("Option<") && text != "None" => vec!["None".into()],
        _ if base.starts_with("Vec<") && !matches!(text, "Vec::new()" | "vec![]") => {
            vec!["Vec::new()".into()]
        }
        _ => Vec::new(),
    }
}

/// `pricing::Pricing::discount`: the module, any `impl` or `trait`, and the function's name.
fn item_path(module: &str, function: &ast::Fn, name: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for ancestor in function.syntax().ancestors().skip(1) {
        if let Some(imp) = ast::Impl::cast(ancestor.clone()) {
            let ty = imp
                .self_ty()
                .map(|ty| plain(ty.syntax()))
                .unwrap_or_default();
            parts.push(match imp.trait_() {
                Some(tr) => format!("<{ty} as {}>", plain(tr.syntax())),
                None => ty,
            });
        } else if let Some(tr) = ast::Trait::cast(ancestor.clone()) {
            parts.push(tr.name().map(|n| n.text().to_string()).unwrap_or_default());
        } else if let Some(m) = ast::Module::cast(ancestor.clone()) {
            parts.push(m.name().map(|n| n.text().to_string()).unwrap_or_default());
        } else if let Some(f) = ast::Fn::cast(ancestor) {
            parts.push(f.name().map(|n| n.text().to_string()).unwrap_or_default());
        }
    }
    parts.reverse();
    let mut path: Vec<String> = module
        .split("::")
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect();
    path.extend(parts);
    path.push(name.to_string());
    path.join("::")
}

#[cfg(test)]
mod tests {
    use super::*;

    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    fn found(source: &str) -> Crate {
        let dir = std::env::temp_dir().join(format!(
            "mutastic-sites-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let found = find(&dir, "sample", &[PathBuf::from("src/lib.rs")], STRONGER);
        std::fs::remove_dir_all(&dir).ok();
        found
    }

    fn changes(source: &str) -> Vec<(String, &'static str, String)> {
        found(source)
            .mutants
            .iter()
            .map(|m| {
                (
                    source[m.site.clone()].to_string(),
                    m.operator,
                    m.replacement.clone(),
                )
            })
            .collect()
    }

    #[test]
    fn a_comparison_gets_its_boundary_both_constants_and_its_reverse() {
        let expected: Vec<(String, &str, String)> = vec![
            ("total >= 100".into(), BOUNDARY, "(total) > (100)".into()),
            ("total >= 100".into(), REMOVE_CONDITIONAL, "true".into()),
            ("total >= 100".into(), REMOVE_CONDITIONAL, "false".into()),
            (
                "total >= 100".into(),
                OTHER_COMPARISON,
                "(total) < (100)".into(),
            ),
        ];
        assert_eq!(
            changes("pub fn big(total: u32) { if total >= 100 { work() } }"),
            expected
        );
    }

    #[test]
    fn a_number_returned_becomes_zero_and_zero_becomes_one() {
        let found =
            changes("pub fn d(total: u32) -> u32 { if total > 1 { total / 10 } else { 0 } }");
        let returns: Vec<_> = found.iter().filter(|m| m.1 == RETURN_VALUE).collect();
        assert_eq!(returns[0].0, "total / 10");
        assert_eq!(returns[0].2, "0");
        assert_eq!(returns[1].0, "0");
        assert_eq!(returns[1].2, "1");
    }

    #[test]
    fn test_code_constants_and_macros_are_left_alone() {
        let found = changes(
            "const fn c(a: u32) -> u32 { a + 1 }\n\
             pub fn f(a: u32) { let x = [0u8; 2 + 1]; assert!(a + 1 > a); g::<{ 1 + 1 }>(); }\n\
             #[cfg(test)] mod tests { fn t() { let _ = 1 + 2; } }\n\
             #[test] fn t() { let _ = 1 + 2; }",
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn ids_do_not_move_when_code_above_them_moves() {
        let one = found("pub fn d(t: u32) -> bool { t >= 100 }");
        let two = found("\n\n// a comment\npub fn d(t: u32) -> bool {\n    t >= 100\n}");
        let ids = |c: &Crate| c.mutants.iter().map(|m| m.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&one), ids(&two));
        assert_eq!(two.mutants[0].line, 5);
    }

    #[test]
    fn a_copy_of_code_never_adds_a_line() {
        let found = changes("pub fn f(a: u32, b: u32) -> bool {\n    a // left\n    < b\n}");
        let boundary = found.iter().find(|m| m.1 == BOUNDARY).unwrap();
        assert_eq!(boundary.2, "(a) <= (b)");
    }
}
