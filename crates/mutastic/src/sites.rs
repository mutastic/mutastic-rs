//! Finding mutation sites in a crate's source, with rust-analyzer's parser.
//!
//! Only function bodies are mutated, and only where a runtime switch can stand: never in a `const
//! fn`, a type, a pattern, a const argument, an array's length, a nested item or a macro's tokens.
//! Test code (`#[test]` functions and `#[cfg(test)]` modules) is never mutated.
//!
//! Unsafe code is fenced off from syntax alone. Code inside an `unsafe` block, an `unsafe fn` or an
//! `unsafe impl` isn't mutated, nor is a safe function whose body holds an `unsafe` block, unless
//! asked ([`UnsafeMode`]): a mutant there can cause undefined behaviour, which no test can be said
//! to catch or miss. Safe code near unsafe code (a function called from inside an `unsafe` block,
//! or anything else in a module that has one) is mutated but marked, so its mutants run isolated
//! and a crash among them isn't counted as caught.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::path::{Path, PathBuf};

use ra_ap_syntax::ast::{self, ArithOp, BinaryOp, CmpOp, HasAttrs, HasName, LogicOp, Ordering};
use ra_ap_syntax::{AstNode, Edition, SourceFile, SyntaxKind, SyntaxNode, TextRange};

use crate::cfg::Cfg;
use crate::id;

mod extended;
mod operators;
mod rollback;
mod stronger;

/// The operators, by the ids the Java edition uses.
pub const BOUNDARY: &str = "mutastic/boundary";
pub const REMOVE_CONDITIONAL: &str = "mutastic/remove-conditional";
pub const LOGICAL_CONNECTOR: &str = "mutastic/logical-connector";
pub const MATH: &str = "mutastic/math";
pub const OTHER_COMPARISON: &str = "mutastic/other-comparison";
pub const OTHER_ARITHMETIC: &str = "mutastic/other-arithmetic";
pub const RETURN_VALUE: &str = "mutastic/return-value";
pub const REMOVE_VOID_CALL: &str = "mutastic/remove-void-call";
pub const REMOVE_DISCARDED_CALL: &str = "mutastic/remove-discarded-call";
pub const REMOVE_ASSIGNMENT: &str = "mutastic/remove-assignment";
pub const REMOVE_FIELD_WRITE: &str = "mutastic/remove-field-write";
pub const SWAP_STRING: &str = "library/swap-string";
pub const SWAP_MATH: &str = "library/swap-math";
pub const SWAP_MATCH: &str = "stream/swap-match";
pub const REMOVE_STAGE: &str = "stream/remove-stage";
pub const COMPARISON: &str = "mutastic/comparison";
pub const REMOVE_NEGATION: &str = "mutastic/remove-negation";
pub const REMOVE_STRUCT_FIELD: &str = "mutastic/remove-struct-field";
pub const NULL_PARAMETER: &str = "mutastic/null-parameter";
pub const SWAP_PRESENT: &str = "optional/swap-present";
pub const OPTIONAL_EMPTY: &str = "optional/empty";
pub const ALWAYS_DEFAULT: &str = "optional/always-default";
pub const NO_DEFAULT: &str = "optional/no-default";
pub const UNCHECKED_ARITHMETIC: &str = "library/unchecked-arithmetic";
pub const BITWISE: &str = "mutastic/bitwise";
pub const STRING_CONSTANT: &str = "mutastic/string-constant";
pub const CONSTANT_VALUE: &str = "mutastic/constant-value";
pub const ARITHMETIC_REPLACEMENT: &str = "mutastic/arithmetic-replacement";
pub const KEEP_OPERAND: &str = "mutastic/keep-operand";
pub const OMIT_CALL: &str = "mutastic/omit-call";
pub const SWAP_ARGUMENTS: &str = "mutastic/swap-arguments";
pub const SWAP_INDEX_OF: &str = "library/swap-index-of";
pub const REVERSE_ORDERING: &str = "library/reverse-ordering";
pub const REMOVE_SWITCH_ARM: &str = "mutastic/remove-switch-arm";
pub const UNWRAP_TRY: &str = "mutastic/unwrap-try";
pub const REPLACE_BODY: &str = "mutastic/replace-body";
pub const REGEX_CONSTANT: &str = "mutastic/regex-constant";
pub const DROP_VARARG: &str = "mutastic/drop-vararg";
pub const REMOVE_OTHER_STAGE: &str = "stream/remove-other-stage";
pub const REMOVE_FILTER: &str = "optional/remove-filter";
pub const DEFAULT_PARAMETER: &str = "mutastic/default-parameter";
pub const SWAP_BREAK: &str = "mutastic/swap-break";
pub const REMOVE_STEP: &str = "bigdecimal/remove-step";
pub const REMOVE_THROW: &str = "mutastic/remove-throw";
/// Not an operator of its own: in a set, it turns on `mutastic/return-value`'s extended forms,
/// `Default::default()` for a type that visibly has one, and an `Err` the function returns
/// elsewhere.
pub const RETURN_VALUE_EXTENDED: &str = "mutastic/return-value(extended)";

/// The operators a run uses when none are named: the Java edition's `defaults`.
pub const DEFAULTS: &[&str] = &[
    BOUNDARY,
    REMOVE_CONDITIONAL,
    LOGICAL_CONNECTOR,
    MATH,
    RETURN_VALUE,
    REMOVE_VOID_CALL,
    REMOVE_DISCARDED_CALL,
    REMOVE_ASSIGNMENT,
    SWAP_STRING,
    SWAP_MATH,
    SWAP_MATCH,
    REMOVE_STAGE,
];
/// `stronger` adds changes a test that catches the defaults' mostly catches too, and changes
/// that are more often noise: the condition reversed (`==` to `!=`, `<` to `>=`), `%` to `*`,
/// and a write to a field removed.
pub const STRONGER: &[&str] = &[
    BOUNDARY,
    REMOVE_CONDITIONAL,
    LOGICAL_CONNECTOR,
    MATH,
    RETURN_VALUE,
    REMOVE_VOID_CALL,
    REMOVE_DISCARDED_CALL,
    REMOVE_ASSIGNMENT,
    SWAP_STRING,
    SWAP_MATH,
    SWAP_MATCH,
    REMOVE_STAGE,
    OTHER_COMPARISON,
    OTHER_ARITHMETIC,
    REMOVE_FIELD_WRITE,
    COMPARISON,
    REMOVE_NEGATION,
    REMOVE_STRUCT_FIELD,
    NULL_PARAMETER,
    SWAP_PRESENT,
    OPTIONAL_EMPTY,
    ALWAYS_DEFAULT,
    NO_DEFAULT,
    UNCHECKED_ARITHMETIC,
    BITWISE,
    STRING_CONSTANT,
    CONSTANT_VALUE,
    ARITHMETIC_REPLACEMENT,
    KEEP_OPERAND,
    OMIT_CALL,
    SWAP_ARGUMENTS,
    SWAP_INDEX_OF,
    REVERSE_ORDERING,
    REMOVE_SWITCH_ARM,
    UNWRAP_TRY,
];

/// `extended` adds operators that make more noise, or more unviable mutants, for the cost: a
/// function's whole body replaced, a parameter given its type's default, a regular expression or
/// an argument list changed, and the rest of the Java edition's extended set.
pub const EXTENDED: &[&str] = &[
    BOUNDARY,
    REMOVE_CONDITIONAL,
    LOGICAL_CONNECTOR,
    MATH,
    RETURN_VALUE,
    REMOVE_VOID_CALL,
    REMOVE_DISCARDED_CALL,
    REMOVE_ASSIGNMENT,
    SWAP_STRING,
    SWAP_MATH,
    SWAP_MATCH,
    REMOVE_STAGE,
    OTHER_COMPARISON,
    OTHER_ARITHMETIC,
    REMOVE_FIELD_WRITE,
    COMPARISON,
    REMOVE_NEGATION,
    REMOVE_STRUCT_FIELD,
    NULL_PARAMETER,
    SWAP_PRESENT,
    OPTIONAL_EMPTY,
    ALWAYS_DEFAULT,
    NO_DEFAULT,
    UNCHECKED_ARITHMETIC,
    BITWISE,
    STRING_CONSTANT,
    CONSTANT_VALUE,
    ARITHMETIC_REPLACEMENT,
    KEEP_OPERAND,
    OMIT_CALL,
    SWAP_ARGUMENTS,
    SWAP_INDEX_OF,
    REVERSE_ORDERING,
    REMOVE_SWITCH_ARM,
    UNWRAP_TRY,
    REPLACE_BODY,
    RETURN_VALUE_EXTENDED,
    REGEX_CONSTANT,
    DROP_VARARG,
    REMOVE_OTHER_STAGE,
    REMOVE_FILTER,
    DEFAULT_PARAMETER,
    SWAP_BREAK,
    REMOVE_STEP,
    REMOVE_THROW,
];

/// Every operator mutastic has, for listing every mutant (the stable-ids test).
pub const ALL: &[&str] = EXTENDED;

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
    /// A name or operator to its counterpart: `starts_with` to `ends_with`, `any` to `all`.
    Swapped { before: String, after: String },
    /// A statement removed: `v.push(x);`.
    Removed { statement: String },
    /// An expression to another: `xs.iter().filter(p).count()` to `xs.iter().count()`.
    Replaced { before: String, after: String },
    /// A comparison to the one that agrees with it at its edge and on one side: `<` to `!=`, which
    /// differs only where `a > b` (`differs`).
    Compared {
        before: String,
        after: String,
        differs: String,
    },
    /// A parameter given another value as the function starts.
    Parameter { name: String, value: String },
    /// A match arm that no longer matches: its values go to the next arm that does.
    ArmRemoved { pattern: String },
    /// A function's whole body replaced by a value, or by nothing (`value` empty).
    Body { value: String },
}

/// How the schema writes a mutant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Form {
    /// In place of an expression, inside parentheses.
    Expression,
    /// In place of a statement, or a block-like expression standing as one: no parentheses.
    Statement,
    /// Code added at the site's start, which is empty. `{n}` in it is the switch's number.
    Insert { text: String },
    /// In place of a function's whole body, which must stay a block.
    Body,
}

/// How much of the code in and around `unsafe` gets mutants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UnsafeMode {
    /// Neither unsafe code nor a safe function holding an `unsafe` block.
    #[default]
    Skip,
    /// Safe functions holding an `unsafe` block too, but not the code inside the block.
    Enclosing,
    /// Everything, the code inside `unsafe` included.
    All,
}

impl UnsafeMode {
    pub fn parse(name: &str) -> Option<UnsafeMode> {
        match name {
            "skip" => Some(UnsafeMode::Skip),
            "enclosing" => Some(UnsafeMode::Enclosing),
            "all" => Some(UnsafeMode::All),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            UnsafeMode::Skip => "skip",
            UnsafeMode::Enclosing => "enclosing",
            UnsafeMode::All => "all",
        }
    }
}

/// Where code stands relative to `unsafe`, from nearest to farthest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tier {
    /// Inside an `unsafe` block, an `unsafe fn` or a method of an `unsafe impl`.
    Unsafe,
    /// A safe function whose body holds an `unsafe` block.
    Enclosing,
    /// A function called from inside unsafe code somewhere in the crate.
    Callee,
    /// Anything else in a module with unsafe code, or a method of a type that is `unsafe impl`
    /// `Send` or `Sync`.
    Module,
    Safe,
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
    /// Why this mutant is near unsafe code, when it is: it runs isolated, and a crash isn't
    /// counted as caught.
    pub near_unsafe: Option<&'static str>,
    pub form: Form,
    /// The edit the card claims, when it isn't the change's own (an arm deleted, for an arm the
    /// schema guards).
    pub claimed: Option<(Range<usize>, String)>,
    /// Why it isn't run, by its kind (`equivalent: …`): it's made, so it keeps its id, but it's
    /// out of the score and needs no test.
    pub skipped: Option<&'static str>,
}

impl Mutant {
    /// The change the card claims, as an edit to the original text: the bytes to replace and
    /// what with. A truth test writes this into the source and checks the mutant behaves the same.
    pub fn claim(&self) -> (Range<usize>, String) {
        if let Some(claimed) = &self.claimed {
            return claimed.clone();
        }
        match &self.change {
            Change::Boundary { after, .. }
            | Change::Reversed { after, .. }
            | Change::Connector { after, .. }
            | Change::Swapped { after, .. }
            | Change::Compared { after, .. }
            | Change::Math { after, .. } => (self.caret.clone(), after.clone()),
            Change::Always { truth, .. } if self.replacement == truth.to_string() => {
                (self.site.clone(), truth.to_string())
            }
            Change::Always { .. } => (self.site.clone(), self.replacement.clone()),
            Change::Returns { value } => (self.site.clone(), value.clone()),
            // A block written where an expression starts would be read as a statement.
            Change::Replaced { .. } if self.replacement.starts_with('{') => {
                (self.site.clone(), format!("({})", self.replacement))
            }
            Change::Body { value } => (self.site.clone(), format!("{{ {value} }}")),
            Change::Removed { .. }
            | Change::Replaced { .. }
            | Change::Parameter { .. }
            | Change::ArmRemoved { .. } => (self.site.clone(), self.replacement.clone()),
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
    /// Mutants not made because they'd be in or around unsafe code.
    pub not_mutated_unsafe: usize,
    /// Mutants not made because this build doesn't have their code, by the `cfg` that's off
    /// (`cfg(windows)`).
    pub not_built: BTreeMap<String, usize>,
}

const WHOLE_NUMBERS: &[&str] = &[
    "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
];

/// Finds every mutant in the files reachable from `roots`, which are relative to `workspace`,
/// leaving unsafe code alone.
pub fn find(workspace: &Path, crate_name: &str, roots: &[PathBuf], operators: &[&str]) -> Crate {
    find_with(
        workspace,
        crate_name,
        roots,
        operators,
        UnsafeMode::Skip,
        &Cfg::unknown(),
    )
}

/// A source file, parsed.
struct Parsed {
    file: PathBuf,
    module: String,
    text: String,
    tree: SourceFile,
    damaged: Vec<TextRange>,
    /// The `cfg` that leaves the whole file out of this build, on its `mod` declaration.
    off: Option<String>,
}

/// What the whole crate says about its unsafe code.
#[derive(Default)]
struct Facts {
    /// Names of functions and methods called from inside unsafe code.
    unsafe_callees: BTreeSet<String>,
    /// Types that are `unsafe impl Send` or `Sync`.
    unsafe_send_sync: BTreeSet<String>,
    /// Names of the crate's functions and methods that return nothing.
    unit_fns: BTreeSet<String>,
    /// Methods that take `self` and give `Self`: builders, whose call can be left out.
    builders: BTreeSet<String>,
    /// Functions of one parameter whose type is the type they return.
    same_type_fns: BTreeSet<String>,
    /// Functions and methods whose last two parameters have one type, by whether they're methods.
    swappable: BTreeSet<(String, bool)>,
    /// Types that visibly have a `Default`: derived, or implemented in the crate.
    default_types: BTreeSet<String>,
}

impl Facts {
    /// The types with a `#[derive(Default)]` or an `impl Default for`.
    fn defaults(&mut self, tree: &SourceFile) {
        for node in tree.syntax().descendants() {
            if let Some(imp) = ast::Impl::cast(node.clone()) {
                let is_default = imp
                    .trait_()
                    .is_some_and(|tr| last_segment(&plain(tr.syntax())) == "Default");
                if let (true, Some(ty)) = (is_default, imp.self_ty()) {
                    self.default_types.insert(type_head(&plain(ty.syntax())));
                }
            } else if let Some(adt) = ast::Adt::cast(node) {
                let derives = adt.attrs().any(|attr| {
                    let text = plain(attr.syntax());
                    text.contains("derive") && text.contains("Default")
                });
                if let (true, Some(name)) = (derives, adt.name()) {
                    self.default_types.insert(name.text().to_string());
                }
            }
        }
    }

    /// What a function's signature says the operators can do with its calls.
    fn signature(&mut self, function: &ast::Fn) {
        let (Some(name), Some(list)) = (function.name(), function.param_list()) else {
            return;
        };
        let name = name.text().to_string();
        let returns = function
            .ret_type()
            .and_then(|ret| ret.ty())
            .map(|ty| plain(ty.syntax()));
        let method = list.self_param().is_some();
        let types: Vec<String> = list
            .params()
            .filter_map(|param| param.ty().map(|ty| plain(ty.syntax())))
            .collect();
        let by_value_self = list
            .self_param()
            .is_some_and(|me| me.amp_token().is_none() && me.ty().is_none());
        if by_value_self && returns.as_deref() == Some("Self") {
            self.builders.insert(name.clone());
        }
        if !method && types.len() == 1 && returns.as_ref() == types.first() {
            self.same_type_fns.insert(name.clone());
        }
        if let [.., before, last] = types.as_slice() {
            if before == last && !before.contains("impl ") {
                self.swappable.insert((name, method));
            }
        }
    }

    fn of(files: &[Parsed]) -> Facts {
        let mut facts = Facts::default();
        for parsed in files {
            for function in parsed.tree.syntax().descendants().filter_map(ast::Fn::cast) {
                let returns_nothing = function.ret_type().is_none_or(|ret| {
                    ret.ty()
                        .is_some_and(|ty| matches!(plain(ty.syntax()).as_str(), "()"))
                });
                if let (true, Some(name)) = (returns_nothing, function.name()) {
                    facts.unit_fns.insert(name.text().to_string());
                }
                facts.signature(&function);
            }
            facts.defaults(&parsed.tree);
            for node in parsed.tree.syntax().descendants() {
                if !opens_unsafe(&node) {
                    continue;
                }
                if let Some(imp) = ast::Impl::cast(node.clone()) {
                    let is_marker = imp
                        .trait_()
                        .map(|tr| plain(tr.syntax()))
                        .is_some_and(|tr| matches!(last_segment(&tr), "Send" | "Sync"));
                    if let (true, Some(ty)) = (is_marker, imp.self_ty()) {
                        facts
                            .unsafe_send_sync
                            .insert(type_head(&plain(ty.syntax())));
                    }
                }
                for inner in node.descendants() {
                    if let Some(name) = callee(&inner) {
                        facts.unsafe_callees.insert(name);
                    }
                }
            }
        }
        facts
    }
}

/// An `unsafe` block, `unsafe fn` or `unsafe impl`: what's inside is unsafe code.
fn opens_unsafe(node: &SyntaxNode) -> bool {
    if let Some(block) = ast::BlockExpr::cast(node.clone()) {
        return block.unsafe_token().is_some();
    }
    if let Some(function) = ast::Fn::cast(node.clone()) {
        return function.unsafe_token().is_some();
    }
    ast::Impl::cast(node.clone()).is_some_and(|imp| imp.unsafe_token().is_some())
}

/// The name a call calls: `f` in `a::f(x)`, `push` in `v.push(x)`.
fn callee(node: &SyntaxNode) -> Option<String> {
    if let Some(call) = ast::MethodCallExpr::cast(node.clone()) {
        return call.name_ref().map(|name| name.text().to_string());
    }
    let call = ast::CallExpr::cast(node.clone())?;
    let ast::Expr::PathExpr(path) = call.expr()? else {
        return None;
    };
    Some(path.path()?.segment()?.name_ref()?.text().to_string())
}

fn last_segment(path: &str) -> &str {
    let path = path.split('<').next().unwrap_or(path);
    path.rsplit("::").next().unwrap_or(path).trim()
}

/// `Buffer` for `Buffer<T>` or `crate::Buffer<T>`.
fn type_head(ty: &str) -> String {
    last_segment(ty).to_string()
}

/// Finds every mutant in the files reachable from `roots`, with unsafe code treated as `mode`
/// says, and code under a `cfg` that `cfg` says is off counted but not mutated.
pub fn find_with(
    workspace: &Path,
    crate_name: &str,
    roots: &[PathBuf],
    operators: &[&str],
    mode: UnsafeMode,
    cfg: &Cfg,
) -> Crate {
    let mut found = Crate::default();
    let mut parsed = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut queue: Vec<Queued> = roots
        .iter()
        .map(|root| (root.clone(), String::new(), true, None))
        .collect();
    while let Some((file, module, is_root, off)) = queue.pop() {
        if !seen.insert(file.clone()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(workspace.join(&file)) else {
            continue;
        };
        let parse = SourceFile::parse(&text, Edition::CURRENT);
        let tree = parse.tree();
        for (child, child_module, declared_off) in
            child_modules(workspace, &file, &module, is_root, &tree, cfg)
        {
            queue.push((child, child_module, false, off.clone().or(declared_off)));
        }
        let damaged = damaged_items(&tree, parse.errors().iter().map(|error| error.range()));
        parsed.push(Parsed {
            file,
            module,
            text,
            tree,
            damaged,
            off,
        });
    }
    let facts = Facts::of(&parsed);
    for Parsed {
        file,
        module,
        text,
        tree,
        damaged,
        off,
    } in parsed
    {
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
            facts: &facts,
            mode,
            cfg,
            file_off: off,
            module_unsafe: BTreeMap::new(),
            returned: std::collections::HashSet::new(),
            errors: Vec::new(),
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

/// A file still to read: its path, module path, whether it's a crate root, and the `cfg` that
/// leaves it out of this build.
type Queued = (PathBuf, String, bool, Option<String>);

/// The files of `mod name;` declarations in `tree`, with their module paths and the `cfg` that
/// leaves each out of this build, if one does.
fn child_modules(
    workspace: &Path,
    file: &Path,
    module: &str,
    is_root: bool,
    tree: &SourceFile,
    cfg: &Cfg,
) -> Vec<(PathBuf, String, Option<String>)> {
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
                let nested = base.join(&name).join("mod.rs");
                if workspace.join(&flat).exists() || !workspace.join(&nested).exists() {
                    flat
                } else {
                    nested
                }
            });
        let mut parts: Vec<String> = module
            .split("::")
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect();
        parts.extend(inline.into_iter().rev());
        parts.push(name);
        children.push((path, parts.join("::"), cfg.off_at(declared.syntax())));
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

/// Whether a node is test code: a `#[test]`-like function, or under a `cfg` that needs `test`.
fn is_test_only(node: &SyntaxNode) -> bool {
    node.ancestors().any(|ancestor| {
        ast::AnyHasAttrs::cast(ancestor)
            .is_some_and(|item| item.attrs().any(|attr| is_test_attribute(&attr)))
    })
}

fn is_test_attribute(attr: &ast::Attr) -> bool {
    let text: String = attr
        .syntax()
        .text()
        .to_string()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let Some(inner) = text
        .strip_prefix("#![")
        .or_else(|| text.strip_prefix("#["))
        .and_then(|rest| rest.strip_suffix(']'))
    else {
        return false;
    };
    let path_end = inner.find(['(', '=']).unwrap_or(inner.len());
    let path = &inner[..path_end];
    if matches!(
        last_segment(path),
        "test" | "bench" | "rstest" | "test_case"
    ) {
        return true;
    }
    path == "cfg"
        && inner[path_end..]
            .strip_prefix('(')
            .and_then(|rest| rest.strip_suffix(')'))
            .is_some_and(needs_test)
}

/// Whether a `cfg` predicate holds only, or also, when compiling tests: `test`, `all(test, unix)`,
/// `any(test, feature = "fuzzing")`, but not `not(test)`.
fn needs_test(predicate: &str) -> bool {
    let predicate = predicate.trim();
    if predicate == "test" {
        return true;
    }
    for combinator in ["all(", "any("] {
        if let Some(args) = predicate
            .strip_prefix(combinator)
            .and_then(|rest| rest.strip_suffix(')'))
        {
            return top_level_split(args).into_iter().any(needs_test);
        }
    }
    false
}

/// `a, f(b, c), d` split at its top-level commas.
fn top_level_split(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut start) = (0i32, 0);
    for (at, c) in text.char_indices() {
        match c {
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' | '>' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&text[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
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
    facts: &'a Facts,
    mode: UnsafeMode,
    cfg: &'a Cfg,
    /// The `cfg` that leaves this whole file out of the build, from its `mod` declaration.
    file_off: Option<String>,
    /// Whether each module (by its range in this file) has unsafe code.
    module_unsafe: BTreeMap<(u32, u32), bool>,
    /// The values the current function returns, by range: return-value's sites.
    returned: std::collections::HashSet<TextRange>,
    /// `Err` values the current function returns that could be returned anywhere in it.
    errors: Vec<String>,
}

/// What the walk knows about where it is.
#[derive(Clone, Copy)]
struct Context<'r> {
    function: usize,
    returns: Option<&'r str>,
    in_closure: bool,
    tier: Tier,
}

impl Finder<'_> {
    fn function(&mut self, function: &ast::Fn) {
        let range = function.syntax().text_range();
        if function.const_token().is_some()
            || is_test_only(function.syntax())
            || is_naked(function)
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
            tier: self.tier(function, &name, &body),
        };
        self.returned.clear();
        if let Some(returns) = context.returns {
            if let Some(tail) = body.tail_expr() {
                self.returned(&tail, returns, context);
            }
            self.mark_returns(body.syntax());
        }
        self.parameters(function, context);
        self.default_parameters(function, context);
        self.harvest_errors(body.syntax());
        self.replace_body(function, &body, context.returns.unwrap_or("()"), context);
        self.walk(body.syntax(), context);
    }

    /// Where a function stands relative to unsafe code.
    fn tier(&mut self, function: &ast::Fn, name: &str, body: &ast::BlockExpr) -> Tier {
        let in_unsafe_impl = function
            .syntax()
            .ancestors()
            .filter_map(ast::Impl::cast)
            .any(|imp| imp.unsafe_token().is_some());
        if function.unsafe_token().is_some() || in_unsafe_impl {
            return Tier::Unsafe;
        }
        let holds_unsafe = body
            .syntax()
            .descendants()
            .filter_map(ast::BlockExpr::cast)
            .any(|block| block.unsafe_token().is_some());
        if holds_unsafe {
            return Tier::Enclosing;
        }
        if self.facts.unsafe_callees.contains(name) {
            return Tier::Callee;
        }
        let of_marked_type = function
            .syntax()
            .ancestors()
            .filter_map(ast::Impl::cast)
            .filter_map(|imp| imp.self_ty())
            .any(|ty| {
                self.facts
                    .unsafe_send_sync
                    .contains(&type_head(&plain(ty.syntax())))
            });
        if of_marked_type || self.module_has_unsafe(function.syntax()) {
            return Tier::Module;
        }
        Tier::Safe
    }

    /// Whether the module a node is in, inline or this file, has unsafe code anywhere in it.
    fn module_has_unsafe(&mut self, node: &SyntaxNode) -> bool {
        let scope = node
            .ancestors()
            .find(|ancestor| ast::Module::can_cast(ancestor.kind()))
            .or_else(|| node.ancestors().last())
            .unwrap_or_else(|| node.clone());
        let range = scope.text_range();
        let key = (u32::from(range.start()), u32::from(range.end()));
        *self.module_unsafe.entry(key).or_insert_with(|| {
            scope
                .descendants_with_tokens()
                .any(|element| element.kind() == SyntaxKind::UNSAFE_KW)
        })
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
        self.visit(node, context);
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
            ast::Expr::LetExpr(binding) => {
                if let (Some(pat), Some(value)) = (binding.pat(), binding.expr()) {
                    self.never_some(&pat, &value, context);
                }
            }
            ast::Expr::Literal(_) | ast::Expr::BlockExpr(_) => {}
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
                self.negated(other, context);
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
            BinaryOp::ArithOp(ArithOp::Rem) => self.add(
                OTHER_ARITHMETIC,
                expr.syntax(),
                caret,
                swap("*"),
                Change::Math {
                    before,
                    after: "*".into(),
                },
                operands,
                context,
            ),
            BinaryOp::Assignment {
                op: Some(ArithOp::Rem),
            } => self.add(
                OTHER_ARITHMETIC,
                expr.syntax(),
                caret,
                format!("{} *= ({})", operands[0], operands[1]),
                Change::Math {
                    before,
                    after: "*=".into(),
                },
                operands,
                context,
            ),
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
                self.returned.insert(other.syntax().text_range());
                let text = plain(other.syntax());
                // A formatter's `Ok(())` writes nothing: arid, as `toString` is in the Java edition.
                let formats = self.found.functions[context.function].name == "fmt";
                for replacement in self.extended_returns(returns, &text, other) {
                    if formats && replacement.starts_with("Ok(") {
                        continue;
                    }
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
        self.make(
            Make {
                operator,
                node,
                site: node.text_range(),
                caret,
                replacement,
                change,
                operands,
                form: form_of(node),
                claimed: None,
            },
            context,
        );
    }

    fn make(&mut self, make: Make<'_>, context: Context<'_>) {
        let Make {
            operator,
            node,
            site: range,
            caret,
            replacement,
            change,
            operands,
            form,
            claimed,
        } = make;
        if !self.operators.contains(&operator) {
            return;
        }
        let ty = (operator == RETURN_VALUE)
            .then_some(context.returns)
            .flatten();
        if !literal_fits(&replacement, ty) {
            return;
        }
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
        if let Some(off) = self.file_off.clone().or_else(|| self.cfg.off_at(node)) {
            *self.found.not_built.entry(off).or_default() += 1;
            return;
        }
        let near_unsafe = match self.tier_at(node, context.tier) {
            Tier::Safe => None,
            Tier::Module => Some("in a module with unsafe code"),
            Tier::Callee => Some("called from unsafe code"),
            Tier::Enclosing if self.mode != UnsafeMode::Skip => {
                Some("in a function with an unsafe block")
            }
            Tier::Unsafe if self.mode == UnsafeMode::All => Some("inside unsafe code"),
            Tier::Enclosing | Tier::Unsafe => {
                self.found.not_mutated_unsafe += 1;
                return;
            }
        };
        let start = usize::from(range.start());
        let caret_start = usize::from(caret.start());
        // The line the change is on: an operator on its own line is shown there.
        let line = line_of(&self.lines, caret_start);
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
            near_unsafe,
            form,
            claimed,
            skipped: None,
        });
    }

    /// The tier of the code at `node`: its function's, or unsafe inside an `unsafe` block.
    fn tier_at(&self, node: &SyntaxNode, function: Tier) -> Tier {
        let in_block = node
            .ancestors()
            .take_while(|ancestor| !ast::Fn::can_cast(ancestor.kind()))
            .filter_map(ast::BlockExpr::cast)
            .any(|block| block.unsafe_token().is_some());
        if in_block {
            Tier::Unsafe
        } else {
            function
        }
    }
}

/// A mutant, before its id and place are worked out.
struct Make<'n> {
    operator: &'static str,
    /// What the mutant changes, for its id: its text before.
    node: &'n SyntaxNode,
    /// What the schema replaces: usually `node`'s range; empty for code added.
    site: TextRange,
    caret: TextRange,
    replacement: String,
    change: Change,
    operands: Vec<String>,
    form: Form,
    claimed: Option<(Range<usize>, String)>,
}

/// A statement, or a block-like expression standing as one, is replaced by a statement: a guard in
/// parentheses would end the statement early.
fn form_of(node: &SyntaxNode) -> Form {
    let standing = node
        .parent()
        .is_some_and(|parent| parent.kind() == SyntaxKind::EXPR_STMT);
    let block_like = matches!(
        node.kind(),
        SyntaxKind::FOR_EXPR
            | SyntaxKind::WHILE_EXPR
            | SyntaxKind::LOOP_EXPR
            | SyntaxKind::IF_EXPR
            | SyntaxKind::MATCH_EXPR
            | SyntaxKind::BLOCK_EXPR
    );
    if node.kind() == SyntaxKind::EXPR_STMT
        || node.kind() == SyntaxKind::LET_STMT
        || (standing && block_like)
    {
        Form::Statement
    } else {
        Form::Expression
    }
}

/// `node`'s text as `plain` gives it, with the tokens in each range replaced: `xs.iter().count()`
/// from `xs.iter().filter(p).count()` with `.filter(p)` replaced by nothing.
pub fn plain_replacing(node: &SyntaxNode, edits: &[(TextRange, &str)]) -> String {
    let mut out = String::new();
    let mut written = BTreeSet::new();
    for token in node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        let range = token.text_range();
        if let Some((index, (_, with))) = edits
            .iter()
            .enumerate()
            .find(|(_, (edit, _))| edit.contains_range(range))
        {
            if written.insert(index) {
                out.push_str(with);
            }
            continue;
        }
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

/// `#[naked]` functions hold only assembly: nothing of mutastic's can go in one.
fn is_naked(function: &ast::Fn) -> bool {
    function.attrs().any(|attr| {
        let text: String = attr
            .syntax()
            .text()
            .to_string()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        text == "#[naked]" || text == "#[unsafe(naked)]"
    })
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
        _ => None,
    }
}

/// An integer literal as written, `-12`, `0x1f`, `255u8`: its value and its suffix (`""` if none).
pub fn integer_literal(text: &str) -> Option<(i128, &str)> {
    let (negative, digits) = match text.trim().strip_prefix('-') {
        Some(rest) => (true, rest.trim()),
        None => (false, text.trim()),
    };
    let suffix_at = WHOLE_NUMBERS
        .iter()
        .filter(|suffix| digits.ends_with(*suffix))
        .map(|suffix| digits.len() - suffix.len())
        .min()
        .unwrap_or(digits.len());
    let (number, suffix) = digits.split_at(suffix_at);
    let number = number.trim_end_matches('_').replace('_', "");
    let value = match number.get(..2) {
        Some("0x") => i128::from_str_radix(&number[2..], 16).ok()?,
        Some("0o") => i128::from_str_radix(&number[2..], 8).ok()?,
        Some("0b") => i128::from_str_radix(&number[2..], 2).ok()?,
        _ => number.parse().ok()?,
    };
    Some((if negative { -value } else { value }, suffix))
}

/// Whether `value` fits the integer type `ty`. Lints are capped in mutastic's build, so a literal
/// that overflows its type (`256u8`) isn't rejected: it silently wraps, and is never made.
pub fn fits(value: i128, ty: &str) -> bool {
    let (min, max): (i128, i128) = match ty {
        "i8" => (i8::MIN.into(), i8::MAX.into()),
        "i16" => (i16::MIN.into(), i16::MAX.into()),
        "i32" => (i32::MIN.into(), i32::MAX.into()),
        "i64" | "isize" => (i64::MIN.into(), i64::MAX.into()),
        "u8" => (0, u8::MAX.into()),
        "u16" => (0, u16::MAX.into()),
        "u32" => (0, u32::MAX.into()),
        "u64" | "usize" => (0, u64::MAX.into()),
        "u128" => (0, i128::MAX),
        _ => (i128::MIN, i128::MAX),
    };
    (min..=max).contains(&value)
}

/// Whether a replacement that is an integer literal fits the type it will have: its suffix's, or
/// `ty` when the site's type is known. Anything else passes.
fn literal_fits(replacement: &str, ty: Option<&str>) -> bool {
    match integer_literal(replacement) {
        Some((value, "")) => ty.is_none_or(|ty| fits(value, ty.trim())),
        Some((value, suffix)) => fits(value, suffix),
        None => true,
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
        _ => wider_instead(base, text),
    }
}

/// Return values for more of std's types: empty collections and paths, an empty slice, a wrapped
/// value, and `Ok` of the value its inner type would get.
fn wider_instead(ty: &str, text: &str) -> Vec<String> {
    let (head, args) = generic_parts(ty);
    let name = last_segment(&head);
    let values: Vec<String> = match name {
        "HashMap" | "BTreeMap" | "HashSet" | "BTreeSet" | "VecDeque" | "BinaryHeap" | "PathBuf"
        | "OsString" => vec![format!("{head}::new()")],
        "Box" | "Rc" | "Arc" if args.len() == 1 => inner_values(&args[0], "")
            .into_iter()
            .map(|value| format!("{head}::new({value})"))
            .collect(),
        "Result" if args.len() <= 2 => {
            let inner = args.first().map_or("()", String::as_str);
            let given = text
                .strip_prefix("Ok(")
                .and_then(|rest| rest.strip_suffix(')'))
                .unwrap_or("");
            if inner == "()" {
                vec!["Ok(())".into()]
            } else {
                inner_values(inner, given)
                    .into_iter()
                    .map(|value| format!("Ok({value})"))
                    .collect()
            }
        }
        "Cow" if args.iter().any(|arg| arg == "str") => vec!["Cow::Borrowed(\"\")".into()],
        _ if is_slice_ref(ty) => vec!["&[]".into()],
        _ => Vec::new(),
    };
    values
        .into_iter()
        .filter(|value| value != text && format!("{head}::default()") != text)
        .collect()
}

/// The values a return of type `ty` gets, for a type inside another.
fn inner_values(ty: &str, text: &str) -> Vec<String> {
    let digits = text.trim_end_matches(|c: char| c.is_ascii_alphanumeric() && !c.is_ascii_digit());
    let is_zero = !digits.is_empty() && digits.chars().all(|c| matches!(c, '0' | '.' | '_'));
    let base = ty.trim();
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
        "String" if text != "String::new()" => vec!["String::new()".into()],
        _ if base.starts_with("Option<") && text != "None" => vec!["None".into()],
        _ if base.starts_with("Vec<") && text != "Vec::new()" => vec!["Vec::new()".into()],
        _ => wider_instead(base, text),
    }
}

/// `&[T]`, `&'a [T]` or `&mut [T]`, but not an array.
fn is_slice_ref(ty: &str) -> bool {
    let Some(rest) = ty.strip_prefix('&') else {
        return false;
    };
    let rest = rest.trim_start();
    let rest = match rest.strip_prefix('\'') {
        Some(lifetime) => lifetime.split_once(' ').map_or("", |(_, ty)| ty),
        None => rest,
    };
    let rest = rest.trim_start().trim_start_matches("mut ").trim();
    rest.starts_with('[') && rest.ends_with(']') && !rest.contains(';')
}

/// `HashMap<K, V>` as `("HashMap", ["K", "V"])`; lifetimes are left out of the arguments.
fn generic_parts(ty: &str) -> (String, Vec<String>) {
    let ty = ty.trim();
    let Some(open) = ty.find('<') else {
        return (ty.to_string(), Vec::new());
    };
    let head = ty[..open].trim().to_string();
    let inner = ty[open + 1..].strip_suffix('>').unwrap_or(&ty[open + 1..]);
    let args = top_level_split(inner)
        .into_iter()
        .map(|arg| arg.trim().to_string())
        .filter(|arg| !arg.starts_with('\'') && !arg.is_empty())
        .collect();
    (head, args)
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

    /// The first operators, which these tests count; each later one has its shapes in the truth
    /// fixture.
    const CORE: &[&str] = &[
        BOUNDARY,
        REMOVE_CONDITIONAL,
        LOGICAL_CONNECTOR,
        MATH,
        RETURN_VALUE,
        OTHER_COMPARISON,
    ];

    fn found(source: &str) -> Crate {
        let dir = std::env::temp_dir().join(format!(
            "mutastic-sites-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let found = find(&dir, "sample", &[PathBuf::from("src/lib.rs")], CORE);
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
    fn a_comparison_gets_its_boundary_both_constants_and_the_others() {
        let expected: Vec<(String, &str, String)> = vec![
            ("total >= 100".into(), BOUNDARY, "(total) > (100)".into()),
            ("total >= 100".into(), REMOVE_CONDITIONAL, "true".into()),
            ("total >= 100".into(), REMOVE_CONDITIONAL, "false".into()),
            (
                "total >= 100".into(),
                OTHER_COMPARISON,
                "(total) < (100)".into(),
            ),
            (
                "total >= 100".into(),
                OTHER_COMPARISON,
                "(total) <= (100)".into(),
            ),
            (
                "total >= 100".into(),
                OTHER_COMPARISON,
                "(total) != (100)".into(),
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
             pub fn f(a: u32) { let x = [0u8; 2 + 1]; assert!(a + 1 > a); let _y = g::<{ 1 + 1 }>(); }\n\
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
    fn modules_are_found_in_their_own_directories() {
        let dir = std::env::temp_dir().join(format!("mutastic-modules-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src/a")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), "mod a;\nmod b;").unwrap();
        std::fs::write(
            dir.join("src/a/mod.rs"),
            "mod c;\npub fn f(x: u8) -> u8 { x + 1 }",
        )
        .unwrap();
        std::fs::write(dir.join("src/a/c.rs"), "pub fn g(x: u8) -> u8 { x + 1 }").unwrap();
        std::fs::write(dir.join("src/b.rs"), "pub fn h(x: u8) -> u8 { x + 1 }").unwrap();
        let found = find(&dir, "s", &[PathBuf::from("src/lib.rs")], &[MATH]);
        std::fs::remove_dir_all(&dir).ok();
        let paths: Vec<_> = found.functions.iter().map(|f| f.path.clone()).collect();
        assert_eq!(paths.len(), 3, "{paths:?}");
        assert!(paths.contains(&"a::c::g".to_string()), "{paths:?}");
    }

    #[test]
    fn test_code_is_known_by_its_cfg_however_combined() {
        let found = changes(
            "#[cfg(all(test, unix))] mod a { fn f(x: u8) -> u8 { x + 1 } }\n\
             #[cfg(any(test, feature = \"fuzzing\"))] fn g(x: u8) -> u8 { x + 1 }\n\
             #[tokio::test(flavor = \"multi_thread\")] async fn h() { let _ = 1 + 2; }\n\
             #[cfg(not(test))] fn kept(x: u8) -> u8 { x + 1 }",
        );
        assert_eq!(found.len(), 2, "only kept()'s: {found:?}");
        assert!(needs_test("all(unix, any(windows, test))"));
        assert!(!needs_test("not(test)"));
        assert!(!needs_test("all(not(test), unix)"));
    }

    #[test]
    fn unsafe_code_is_left_alone_and_code_near_it_is_marked() {
        let source = "pub fn reads(v: &[u8]) -> u8 { unsafe { *v.get_unchecked(0) + 1 } }\n\
             pub unsafe fn raw(x: u8) -> u8 { x + 1 }\n\
             pub fn index(x: usize) -> usize { x + 1 }\n\
             pub fn uses(v: &[u8]) -> u8 { unsafe { *v.get_unchecked(index(0)) } }\n\
             mod clean { pub fn f(x: u8) -> u8 { x + 1 } }\n\
             mod risky { pub fn g(x: u8) -> u8 { x + 1 }\n\
             pub fn h(v: &[u8]) -> u8 { unsafe { *v.get_unchecked(0) } } }";
        let near = |found: &Crate| -> Vec<(String, Option<&'static str>)> {
            found
                .mutants
                .iter()
                .filter(|m| m.operator == MATH)
                .map(|m| (found.functions[m.function].path.clone(), m.near_unsafe))
                .collect()
        };
        let skipped = found(source);
        assert_eq!(
            near(&skipped),
            vec![
                ("index".to_string(), Some("called from unsafe code")),
                ("clean::f".to_string(), None),
                ("risky::g".to_string(), Some("in a module with unsafe code")),
            ]
        );
        assert_eq!(skipped.not_mutated_unsafe, 3, "reads() and raw()");
        let dir = std::env::temp_dir().join(format!("mutastic-unsafe-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let all = |mode| {
            find_with(
                &dir,
                "sample",
                &[PathBuf::from("src/lib.rs")],
                &[MATH],
                mode,
                &Cfg::unknown(),
            )
        };
        let (enclosing, everything) = (all(UnsafeMode::Enclosing), all(UnsafeMode::All));
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(
            enclosing.mutants.len(),
            3,
            "the `+ 1` in unsafe stays alone"
        );
        assert_eq!(everything.mutants.len(), 5);
        assert_eq!(
            everything.mutants[0].near_unsafe,
            Some("inside unsafe code")
        );
        let ids = |c: &Crate| {
            c.mutants
                .iter()
                .filter(|m| m.operator == MATH)
                .map(|m| m.id.clone())
                .collect::<BTreeSet<_>>()
        };
        assert!(
            ids(&skipped).is_subset(&ids(&everything)),
            "ids don't depend on the mode"
        );
    }

    #[test]
    fn code_this_build_doesnt_have_is_counted_not_mutated() {
        let dir = std::env::temp_dir().join(format!("mutastic-cfg-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("src/lib.rs"),
            "#[cfg(windows)]\nmod win;\n\
             #[cfg(unix)] pub fn a(x: u8) -> u8 { x + 1 }\n\
             #[cfg(windows)] pub fn b(x: u8) -> u8 { x + 1 }\n\
             #[cfg(docsrs)] pub fn c(x: u8) -> u8 { x + 1 }\n\
             pub fn d(x: u8) -> u8 { #[cfg(feature = \"simd\")] let x = x + 1; x + 1 }\n\
             #[cfg(feature = \"std\")] mod on { pub fn e(x: u8) -> u8 { x + 1 } }\n",
        )
        .unwrap();
        std::fs::write(dir.join("src/win.rs"), "pub fn f(x: u8) -> u8 { x - 1 }").unwrap();
        let with = |cfg: &Cfg| {
            find_with(
                &dir,
                "sample",
                &[PathBuf::from("src/lib.rs")],
                &[MATH],
                UnsafeMode::Skip,
                cfg,
            )
        };
        let unix = Cfg::new("unix\ntarget_os=\"linux\"\n", ["std".to_string()].into());
        let (here, everything) = (with(&unix), with(&Cfg::unknown()));
        std::fs::remove_dir_all(&dir).ok();
        let paths = |found: &Crate| -> Vec<String> {
            found
                .mutants
                .iter()
                .map(|m| found.functions[m.function].path.clone())
                .collect()
        };
        assert_eq!(paths(&here), ["a", "c", "d", "on::e"], "docsrs isn't known");
        assert_eq!(
            here.not_built.clone().into_iter().collect::<Vec<_>>(),
            [
                ("cfg(feature = \"simd\")".to_string(), 1),
                ("cfg(windows)".to_string(), 2),
            ],
            "b(), d()'s first `+`, and win.rs's f()"
        );
        assert_eq!(everything.mutants.len(), 7);
        let ids = |found: &Crate| {
            found
                .mutants
                .iter()
                .map(|m| m.id.clone())
                .collect::<BTreeSet<_>>()
        };
        assert!(
            ids(&here).is_subset(&ids(&everything)),
            "ids don't depend on what's built"
        );
    }

    #[test]
    fn a_question_mark_that_passes_on_only_a_formatters_error_is_skipped() {
        let source = "use std::fmt;\n\
             pub struct P(u8);\n\
             impl fmt::Display for P { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n\
             write!(f, \"{}\", self.0)?; let n: u8 = \"1\".parse().map_err(|_| fmt::Error)?; \
             f.write_str(\"x\") } }\n\
             impl fmt::LowerHex for P { fn fmt(&self, out: &mut fmt::Formatter) -> fmt::Result {\n\
             out.write_str(\"0x\")?; Ok(()) } }\n\
             fn item(f: &mut std::fmt::Formatter<'_>, n: u8) -> fmt::Result { f.write_str(\" \")?; \
             write!(f, \"{n}\")?; Ok(()) }\n\
             fn pad(formatter: &mut Formatter, n: u8) -> fmt::Result { item(formatter, n)?; Ok(()) }\n\
             pub fn parse(text: &str) -> Result<u8, std::num::ParseIntError> { let n = text.parse()?; Ok(n) }\n\
             pub fn log(out: &mut impl std::io::Write) -> std::io::Result<()> { write!(out, \"x\")?; Ok(()) }\n\
             impl P { pub fn fmt(&self, s: &str) -> Result<u8, std::num::ParseIntError> { s.parse() .map(|n: u8| n)?; Ok(1) } }\n";
        let dir = std::env::temp_dir().join(format!("mutastic-fmt-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let found = find(
            &dir,
            "sample",
            &[PathBuf::from("src/lib.rs")],
            &[UNWRAP_TRY],
        );
        std::fs::remove_dir_all(&dir).ok();
        let skipped: Vec<(String, String)> = found
            .mutants
            .iter()
            .map(|m| {
                (
                    found.functions[m.function].path.clone(),
                    m.skipped
                        .map_or("run", |reason| reason.split(':').next().unwrap())
                        .to_string(),
                )
            })
            .collect();
        let expected: Vec<(String, String)> = [
            ("<P as fmt::Display>::fmt", "equivalent"),
            ("<P as fmt::Display>::fmt", "equivalent"),
            ("<P as fmt::LowerHex>::fmt", "equivalent"),
            ("item", "equivalent"),
            ("item", "equivalent"),
            ("pad", "equivalent"),
            ("parse", "run"),
            ("log", "run"),
            ("P::fmt", "run"),
        ]
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
        assert_eq!(skipped, expected);
    }

    #[test]
    fn literals_out_of_their_types_range_are_never_made() {
        assert_eq!(integer_literal("255u8"), Some((255, "u8")));
        assert_eq!(integer_literal("-0x1_f"), Some((-31, "")));
        assert_eq!(integer_literal("1_000_i64"), Some((1000, "i64")));
        assert_eq!(integer_literal("total"), None);
        assert!(!literal_fits("256u8", None));
        assert!(!literal_fits("-1", Some("u32")));
        assert!(literal_fits("-1", None));
        assert!(literal_fits("127i8", None));
        assert!(!literal_fits("128i8", None));
    }

    #[test]
    fn a_borrowed_option_is_never_made_none() {
        let found = changes(
            "pub fn f(o: &Option<u8>, p: Option<u8>) -> u8 { if let Some(x) = &o { return *x; } \
             if let Some(y) = p { y } else { 0 } }",
        );
        let never: Vec<_> = found
            .iter()
            .filter(|m| m.2.ends_with("filter(|_| false)"))
            .collect();
        assert_eq!(never.len(), 1, "{never:?}");
        assert_eq!(never[0].0, "p");
    }

    #[test]
    fn what_rollback_withdrew_on_toml_edit_is_never_made() {
        let source = "pub struct S { span: Option<u8>, n: u8 }\n\
             impl S { pub fn f(&mut self, v: Option<u8>) -> u8 {\n\
             self.span = None;\n\
             #[cfg(feature = \"debug\")] trace(1);\n\
             match v { Some(x) => x, None => 0, _ => 1 } } }";
        let dir = std::env::temp_dir().join(format!("mutastic-toml-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let found = find(
            &dir,
            "s",
            &[PathBuf::from("src/lib.rs")],
            &[
                REMOVE_FIELD_WRITE,
                REMOVE_VOID_CALL,
                REMOVE_DISCARDED_CALL,
                REMOVE_SWITCH_ARM,
            ],
        );
        std::fs::remove_dir_all(&dir).ok();
        let made: Vec<(&str, &str)> = found
            .mutants
            .iter()
            .map(|m| (m.operator, m.replacement.as_str()))
            .collect();
        assert_eq!(
            made,
            vec![
                (REMOVE_FIELD_WRITE, ""),
                (REMOVE_SWITCH_ARM, "None if false")
            ],
            "no `let _ = None;`, nothing under an attribute, no guard on an arm that binds"
        );
    }

    #[test]
    fn a_copy_of_code_never_adds_a_line() {
        let found = changes("pub fn f(a: u32, b: u32) -> bool {\n    a // left\n    < b\n}");
        let boundary = found.iter().find(|m| m.1 == BOUNDARY).unwrap();
        assert_eq!(boundary.2, "(a) <= (b)");
    }
}
