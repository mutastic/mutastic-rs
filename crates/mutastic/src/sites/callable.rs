//! Whether a test can call a function with whole numbers alone, and how it's written: the call a
//! suggested test makes, and the probe mutastic builds beside the function to find its inputs.
//!
//! Only what can be read from the syntax for certain: a free function, an associated function or
//! a method of an inherent `impl`, whose parameters are all whole numbers and whose result can be
//! written as a literal. A method's receiver must be made with whole numbers too: the type's own
//! `new`, or, when it has none, a struct literal. Generics, trait objects, `impl Trait`, `async`
//! and `unsafe` are left out: no suggestion is better than a wrong one.

use std::collections::BTreeMap;

use ra_ap_syntax::ast::{self, HasAttrs, HasGenericParams, HasModuleItem, HasName};
use ra_ap_syntax::{AstNode, SourceFile, SyntaxKind, SyntaxNode};

use super::{is_test_only, plain, WHOLE_NUMBERS};

/// What a function's result is, when it can be written as a literal: `bool` is asserted with
/// `assert!`, the rest with `assert_eq!` against what `{:?}` prints.
const WRITABLE: &[&str] = &["bool", "char", "String", "&str", "&'static str"];

/// How a receiver is made: `Pricing::new()`, `Window { start: 1, end: 10 }`, `Pair(1, 2)` or
/// `Marker`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Made {
    New,
    Fields(Vec<String>),
    Tuple,
    Unit,
}

/// A receiver made with whole numbers: the type, how it's made, and the numbers it takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receiver {
    pub type_name: String,
    pub made: Made,
    /// By name (a field's or `new`'s parameter's) and type.
    pub parameters: Vec<(String, String)>,
}

/// The test module a suggested test goes in: its name, and whether it imports its parent's items
/// (`use super::*;`), so a call needs no `super::`. `text` is its body, to keep a test's name
/// free; empty for one in a file of its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestModule {
    pub name: String,
    pub glob: bool,
    pub text: String,
}

/// How a test calls a function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Callable {
    pub name: String,
    /// A method's receiver.
    pub receiver: Option<Receiver>,
    /// The type of an associated function without `self`, called as `Type::name(..)`.
    pub owner: Option<String>,
    /// The function's own parameters, by name and type: all whole numbers.
    pub parameters: Vec<(String, String)>,
    pub returns: String,
    /// Where the probe goes in the original text: after the function, or the `impl` it's in.
    pub after: usize,
    /// The function's and its `impl`'s `cfg` attributes, which the probe takes too.
    pub cfgs: Vec<String>,
    /// The module the function is in, `rates`; empty for the crate root.
    pub scope: String,
    /// Whether the module is the file's own, rather than one written inline in it.
    pub file_scope: bool,
    pub tests: Option<TestModule>,
}

impl Callable {
    /// Every number a call takes: the receiver's first, then the function's.
    pub fn types(&self) -> Vec<&str> {
        self.receiver
            .iter()
            .flat_map(|receiver| receiver.parameters.iter())
            .chain(&self.parameters)
            .map(|(_, ty)| ty.as_str())
            .collect()
    }

    /// The call with `values` (one for each of `types`) written in, as it reads in the function's
    /// module, or in a module inside it with `prefix` `super::`: `Pricing::new().discount(100)`.
    pub fn call(&self, values: &[String], prefix: &str) -> String {
        let taken = self
            .receiver
            .as_ref()
            .map_or(0, |receiver| receiver.parameters.len());
        let (made, own) = values.split_at(taken.min(values.len()));
        let arguments = own.join(", ");
        match (&self.receiver, &self.owner) {
            (Some(receiver), _) => {
                format!("{}.{}({arguments})", receiver.make(made, prefix), self.name)
            }
            (None, Some(owner)) => format!("{prefix}{owner}::{}({arguments})", self.name),
            (None, None) => format!("{prefix}{}({arguments})", self.name),
        }
    }

    /// The probe's libtest name: `rates::__mutastic_probe_3`.
    pub fn probe_name(&self, ordinal: u32) -> String {
        let name = format!("{PROBE}{ordinal}");
        if self.scope.is_empty() {
            name
        } else {
            format!("{}::{name}", self.scope)
        }
    }
}

/// The start of every probe's name, which no test list shows.
pub const PROBE: &str = "__mutastic_probe_";

impl Receiver {
    pub fn make(&self, values: &[String], prefix: &str) -> String {
        let ty = format!("{prefix}{}", self.type_name);
        match &self.made {
            Made::New => format!("{ty}::new({})", values.join(", ")),
            Made::Fields(names) => {
                let fields: Vec<String> = names
                    .iter()
                    .zip(values)
                    .map(|(name, value)| {
                        if name == value {
                            name.clone()
                        } else {
                            format!("{name}: {value}")
                        }
                    })
                    .collect();
                format!("{ty} {{ {} }}", fields.join(", "))
            }
            Made::Tuple => format!("{ty}({})", values.join(", ")),
            Made::Unit => ty,
        }
    }
}

/// The receivers each type in `tree` can be made with, by its module and name: its `new`, when it
/// has one taking whole numbers, else a literal of its whole-number fields. A type whose `new`
/// takes anything else has none: a literal could make what `new` wouldn't allow.
pub fn receivers(module: &str, tree: &SourceFile, into: &mut BTreeMap<(String, String), Receiver>) {
    let mut news: BTreeMap<(String, String), Option<Receiver>> = BTreeMap::new();
    for node in tree.syntax().descendants() {
        if is_test_only(&node) {
            continue;
        }
        if let Some(imp) = ast::Impl::cast(node.clone()) {
            let Some(ty) = plain_type(&imp) else {
                continue;
            };
            let functions = imp
                .assoc_item_list()
                .into_iter()
                .flat_map(|list| list.assoc_items())
                .filter_map(|item| match item {
                    ast::AssocItem::Fn(function) => Some(function),
                    _ => None,
                });
            for function in functions {
                if function.name().is_some_and(|name| name.text() == "new") {
                    let key = (scope_of(module, imp.syntax()), ty.clone());
                    news.insert(key, constructor(&function, &ty));
                }
            }
        } else if let Some(definition) = ast::Struct::cast(node) {
            let (Some(name), None) = (definition.name(), definition.generic_param_list()) else {
                continue;
            };
            let type_name = name.text().to_string();
            if let Some((made, parameters)) = literal(&definition) {
                into.insert(
                    (scope_of(module, definition.syntax()), type_name.clone()),
                    Receiver {
                        type_name,
                        made,
                        parameters,
                    },
                );
            }
        }
    }
    for (key, receiver) in news {
        match receiver {
            Some(receiver) => into.insert(key, receiver),
            None => into.remove(&key),
        };
    }
}

/// The type an inherent, non-generic `impl` is for, when it's a plain name.
fn plain_type(imp: &ast::Impl) -> Option<String> {
    if imp.trait_().is_some() || imp.generic_param_list().is_some() || imp.unsafe_token().is_some()
    {
        return None;
    }
    let ty = plain(imp.self_ty()?.syntax());
    is_identifier(&ty).then_some(ty)
}

fn is_identifier(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// `new`, as a receiver, when it takes only whole numbers and gives the type.
fn constructor(function: &ast::Fn, ty: &str) -> Option<Receiver> {
    let list = function.param_list()?;
    if list.self_param().is_some() || !plain_signature(function) {
        return None;
    }
    let returns = plain(function.ret_type()?.ty()?.syntax());
    if returns != "Self" && returns != ty {
        return None;
    }
    Some(Receiver {
        type_name: ty.to_string(),
        made: Made::New,
        parameters: whole_numbers(&list)?,
    })
}

/// A struct's fields, when every one is a whole number.
fn literal(definition: &ast::Struct) -> Option<(Made, Vec<(String, String)>)> {
    match definition.field_list() {
        None => Some((Made::Unit, Vec::new())),
        Some(ast::FieldList::RecordFieldList(list)) => {
            let fields: Option<Vec<(String, String)>> = list
                .fields()
                .map(|field| {
                    let ty = plain(field.ty()?.syntax());
                    let name = field.name()?.text().to_string();
                    WHOLE_NUMBERS.contains(&ty.as_str()).then_some((name, ty))
                })
                .collect();
            let fields = fields?;
            let names = fields.iter().map(|(name, _)| name.clone()).collect();
            Some((Made::Fields(names), fields))
        }
        Some(ast::FieldList::TupleFieldList(list)) => {
            let fields: Option<Vec<(String, String)>> = list
                .fields()
                .enumerate()
                .map(|(index, field)| {
                    let ty = plain(field.ty()?.syntax());
                    WHOLE_NUMBERS
                        .contains(&ty.as_str())
                        .then_some((format!("field{index}"), ty))
                })
                .collect();
            Some((Made::Tuple, fields?))
        }
    }
}

/// Not async, unsafe, const or generic.
fn plain_signature(function: &ast::Fn) -> bool {
    function.async_token().is_none()
        && function.unsafe_token().is_none()
        && function.const_token().is_none()
        && function.generic_param_list().is_none()
        && function.where_clause().is_none()
}

/// The parameters, by name and type, when every one is a whole number bound to a plain name.
fn whole_numbers(list: &ast::ParamList) -> Option<Vec<(String, String)>> {
    list.params()
        .map(|param| {
            let ast::Pat::IdentPat(pat) = param.pat()? else {
                return None;
            };
            let ty = plain(param.ty()?.syntax());
            let name = pat.name()?.text().to_string();
            WHOLE_NUMBERS.contains(&ty.as_str()).then_some((name, ty))
        })
        .collect()
}

/// The module a node is in: the file's, then each inline module around it.
pub fn scope_of(module: &str, node: &SyntaxNode) -> String {
    let mut inline: Vec<String> = node
        .ancestors()
        .skip(1)
        .filter_map(ast::Module::cast)
        .filter_map(|outer| outer.name().map(|name| name.text().to_string()))
        .collect();
    inline.reverse();
    module
        .split("::")
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .chain(inline)
        .collect::<Vec<_>>()
        .join("::")
}

/// How a test calls `function`, if it can; `receivers` are what `receivers` found in the crate.
pub fn of(
    function: &ast::Fn,
    module: &str,
    receivers: &BTreeMap<(String, String), Receiver>,
) -> Option<Callable> {
    let list = function.param_list()?;
    if !plain_signature(function) || function.abi().is_some() {
        return None;
    }
    let returns = plain(function.ret_type()?.ty()?.syntax());
    if !WHOLE_NUMBERS.contains(&returns.as_str()) && !WRITABLE.contains(&returns.as_str()) {
        return None;
    }
    let parameters = whole_numbers(&list)?;
    let parent = function.syntax().parent()?;
    let (holder, imp) = match ast::AssocItemList::cast(parent.clone()) {
        Some(items) => {
            let imp = ast::Impl::cast(items.syntax().parent()?)?;
            (imp.syntax().clone(), Some(imp))
        }
        None => (function.syntax().clone(), None),
    };
    if !at_module_level(&holder) {
        return None;
    }
    let scope = scope_of(module, &holder);
    let (receiver, owner) = match &imp {
        None => (None, None),
        Some(imp) => {
            let ty = plain_type(imp)?;
            match list.self_param() {
                None => (None, Some(ty)),
                Some(me) if me.ty().is_some() => return None,
                Some(_) => (Some(receivers.get(&(scope.clone(), ty))?.clone()), None),
            }
        }
    };
    let cfgs = function
        .attrs()
        .chain(imp.iter().flat_map(|imp| imp.attrs()))
        .map(|attr| plain(attr.syntax()))
        .filter(|attr| attr.starts_with("#[cfg(") || attr.starts_with("#[cfg ("))
        .collect();
    Some(Callable {
        name: function.name()?.text().to_string(),
        receiver,
        owner,
        parameters,
        returns,
        after: usize::from(holder.text_range().end()),
        cfgs,
        file_scope: !holder
            .ancestors()
            .skip(1)
            .any(|a| ast::Module::can_cast(a.kind())),
        tests: test_module(&holder),
        scope,
    })
}

/// Whether an item is one of its module's own: in the file, or in an inline module's braces, not
/// inside a function or a block.
fn at_module_level(item: &SyntaxNode) -> bool {
    match item.parent() {
        Some(parent) if parent.kind() == SyntaxKind::SOURCE_FILE => true,
        Some(parent) => {
            parent.kind() == SyntaxKind::ITEM_LIST
                && parent
                    .parent()
                    .is_some_and(|module| ast::Module::can_cast(module.kind()))
        }
        None => false,
    }
}

/// The test module beside `item`: one named `tests` first, else the first under `cfg(test)`.
fn test_module(item: &SyntaxNode) -> Option<TestModule> {
    let parent = item.parent()?;
    let modules: Vec<ast::Module> = match ast::SourceFile::cast(parent.clone()) {
        Some(file) => file.items().filter_map(module_item).collect(),
        None => ast::ItemList::cast(parent)?
            .items()
            .filter_map(module_item)
            .collect(),
    };
    let tests: Vec<&ast::Module> = modules
        .iter()
        .filter(|module| is_test_only(module.syntax()))
        .collect();
    let chosen = tests
        .iter()
        .find(|module| module.name().is_some_and(|name| name.text() == "tests"))
        .or(tests.first())?;
    let text = chosen
        .item_list()
        .map(|items| items.syntax().text().to_string())
        .unwrap_or_default();
    Some(TestModule {
        name: chosen.name()?.text().to_string(),
        glob: text.contains("use super::*;"),
        text,
    })
}

fn module_item(item: ast::Item) -> Option<ast::Module> {
    match item {
        ast::Item::Module(module) => Some(module),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ra_ap_syntax::Edition;

    fn callables(source: &str) -> Vec<Option<Callable>> {
        let tree = SourceFile::parse(source, Edition::CURRENT).tree();
        let mut made = BTreeMap::new();
        receivers("", &tree, &mut made);
        tree.syntax()
            .descendants()
            .filter_map(ast::Fn::cast)
            .filter(|function| !is_test_only(function.syntax()))
            .map(|function| of(&function, "", &made))
            .collect()
    }

    fn calls(source: &str) -> Vec<Option<String>> {
        callables(source)
            .into_iter()
            .map(|callable| {
                callable.map(|callable| {
                    let values: Vec<String> = (1..=callable.types().len())
                        .map(|n| n.to_string())
                        .collect();
                    callable.call(&values, "")
                })
            })
            .collect()
    }

    #[test]
    fn free_functions_and_methods_on_types_made_with_numbers_are_called() {
        let source = "pub fn fee(days: i64) -> i64 { days }\n\
             pub struct P { from: u32 }\n\
             impl P { pub fn new() -> Self { P { from: 5 } } pub fn d(&self, t: u32) -> u32 { t } }\n\
             pub struct W { start: i32, end: i64 }\n\
             impl W { fn open(&mut self, day: i32) -> bool { true } fn make(n: u8) -> u8 { n } }\n\
             struct T(u8, u16);\n\
             impl T { fn first(self) -> u8 { self.0 } }\n";
        assert_eq!(
            calls(source),
            [
                Some("fee(1)".into()),
                None, // `new` returns a type, which isn't written as a literal
                Some("P::new().d(1)".into()),
                Some("W { start: 1, end: 2 }.open(3)".into()),
                Some("W::make(1)".into()),
                Some("T(1, 2).first()".into()),
            ]
        );
    }

    #[test]
    fn what_cant_be_written_for_certain_is_left_out() {
        let source = "pub fn g<T>(x: T) -> u8 { 0 }\n\
             pub fn i(x: impl Into<u8>) -> u8 { 0 }\n\
             pub fn d(x: &dyn Fn() -> u8) -> u8 { 0 }\n\
             pub async fn a(x: u8) -> u8 { x }\n\
             pub unsafe fn u(x: u8) -> u8 { x }\n\
             pub fn v(x: u8) { }\n\
             pub fn f(x: f64) -> u8 { 0 }\n\
             pub fn o(x: u8) -> Option<u8> { None }\n\
             pub struct S { v: Vec<u8> }\n\
             impl S { pub fn len(&self) -> usize { 0 } }\n\
             pub struct N { n: u8 }\n\
             impl N { pub fn new(s: &str) -> Self { N { n: 0 } } pub fn n(&self) -> u8 { 0 } }\n\
             impl<T> Q<T> { pub fn q(&self) -> u8 { 0 } }\n\
             trait Z { fn z(&self, x: u8) -> u8 { x } }\n\
             pub fn outer(s: &str) -> u8 { fn inner(x: u8) -> u8 { x } inner(1) }\n";
        let found: Vec<Option<Callable>> = callables(source);
        assert!(found.iter().all(Option::is_none), "{found:?}");
    }

    #[test]
    fn a_call_in_a_test_module_without_the_glob_import_says_super() {
        let source = "mod rates { pub fn fee(d: i64) -> i64 { d }\n\
             #[cfg(test)] mod checks { use super::fee; fn fee_is_free() {} } }\n";
        let callable = callables(source)[0].clone().unwrap();
        assert_eq!(callable.scope, "rates");
        assert!(!callable.file_scope);
        let tests = callable.tests.clone().unwrap();
        assert_eq!(tests.name, "checks");
        assert!(!tests.glob);
        assert_eq!(callable.call(&["7".into()], "super::"), "super::fee(7)");
        assert_eq!(callable.probe_name(3), "rates::__mutastic_probe_3");
    }
}
