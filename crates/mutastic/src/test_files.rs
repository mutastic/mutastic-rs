//! Where a test is written: the file that holds it and the line and column of its name, found from
//! the name a run shows it by (`tests::small_orders_pay_full_price` in the library,
//! `pricing::big_orders_get_ten_percent_off` in `tests/pricing.rs`) by following its modules from
//! its target's crate root, as the compiler does. `mutation.json` keys its tests by file, as the
//! Java edition's are, and an HTML report shows each test where it is.

use std::path::{Path, PathBuf};

use ra_ap_syntax::ast::{self, HasModuleItem, HasName};
use ra_ap_syntax::{AstNode, Edition, SourceFile};

use crate::workspace::Project;

/// Where a test is: its file, relative to the workspace, and the line and column (1-based, the
/// column in characters) of its function's name, when the file declares it there. A test a macro
/// writes has no declaration to point at: its file is the one its module is in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub file: PathBuf,
    pub at: Option<(usize, usize)>,
}

/// Where the test a run shows as `display` is, or `None` when no target of the package holds it.
/// A library's tests are named by their path in it; any other target's by its name, then the path.
pub fn place(project: &Project, display: &str) -> Option<Place> {
    let library = project
        .targets
        .iter()
        .filter(|target| target.lib)
        .map(|target| (target.root.as_path(), display));
    let others = project
        .targets
        .iter()
        .filter(|target| !target.lib)
        .filter_map(|target| {
            let path = display.strip_prefix(&target.name)?.strip_prefix("::")?;
            Some((target.root.as_path(), path))
        });
    let found: Vec<Place> = library
        .chain(others)
        .filter_map(|(root, path)| find(&project.workspace, root, path))
        .collect();
    // Where its function is declared, before a module that only holds it.
    found
        .iter()
        .find(|place| place.at.is_some())
        .or_else(|| found.first())
        .cloned()
}

/// `path` (`tests::small_orders_pay_full_price`) from the crate root `root`: each module in turn,
/// inline or in its own file, then the function in the last.
fn find(workspace: &Path, root: &Path, path: &str) -> Option<Place> {
    let (modules, name) = path.rsplit_once("::").unwrap_or(("", path));
    let mut file = root.to_path_buf();
    let mut text = std::fs::read_to_string(workspace.join(&file)).ok()?;
    // The directory a `mod name;` here is looked for in, and the inline modules it's within.
    let mut dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut inline: Vec<String> = Vec::new();
    for module in modules.split("::").filter(|module| !module.is_empty()) {
        let tree = SourceFile::parse(&text, Edition::CURRENT).tree();
        let declared = within(&tree, &inline)?
            .into_iter()
            .filter_map(|item| match item {
                ast::Item::Module(declared) => Some(declared),
                _ => None,
            })
            .find(|declared| declared.name().is_some_and(|n| n.text() == module))?;
        if declared.item_list().is_some() {
            inline.push(module.to_string());
            continue;
        }
        let mut base = dir.clone();
        base.extend(&inline);
        let next = match crate::sites::path_attribute(&declared) {
            Some(path) => base.join(path),
            None => {
                let flat = base.join(format!("{module}.rs"));
                let nested = base.join(module).join("mod.rs");
                if workspace.join(&flat).exists() || !workspace.join(&nested).exists() {
                    flat
                } else {
                    nested
                }
            }
        };
        text = std::fs::read_to_string(workspace.join(&next)).ok()?;
        dir = if next.file_name().is_some_and(|name| name == "mod.rs") {
            next.parent().map(Path::to_path_buf).unwrap_or_default()
        } else {
            next.with_extension("")
        };
        file = next;
        inline.clear();
    }
    let tree = SourceFile::parse(&text, Edition::CURRENT).tree();
    let at = within(&tree, &inline)?
        .into_iter()
        .find_map(|item| match item {
            ast::Item::Fn(function) => {
                let named = function.name().filter(|n| n.text() == name)?;
                Some(line_and_column(
                    &text,
                    usize::from(named.syntax().text_range().start()),
                ))
            }
            _ => None,
        });
    Some(Place { file, at })
}

/// The items of the inline module `inline` names in `tree`, or of the file itself.
fn within(tree: &SourceFile, inline: &[String]) -> Option<Vec<ast::Item>> {
    let mut items: Vec<ast::Item> = tree.items().collect();
    for name in inline {
        let module = items.into_iter().find_map(|item| match item {
            ast::Item::Module(module)
                if module.name().is_some_and(|n| n.text() == name.as_str()) =>
            {
                Some(module)
            }
            _ => None,
        })?;
        items = module.item_list()?.items().collect();
    }
    Some(items)
}

/// The 1-based line and column (in characters) of a byte offset.
fn line_and_column(text: &str, offset: usize) -> (usize, usize) {
    let starts = crate::sites::line_starts(text);
    let line = crate::sites::line_of(&starts, offset);
    (line, text[starts[line - 1]..offset].chars().count() + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> Project {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let workspace =
            std::env::temp_dir().join(format!("mutastic-test-files-{}-{n}", std::process::id()));
        for (path, text) in files {
            let file = workspace.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        }
        let target = |name: &str, lib: bool, root: &str| crate::workspace::Target {
            name: name.into(),
            lib,
            root: root.into(),
        };
        Project {
            given: ".".into(),
            workspace,
            target: PathBuf::new(),
            package: "shop".into(),
            package_dir: PathBuf::new(),
            roots: vec!["src/lib.rs".into()],
            targets: vec![
                target("shop", true, "src/lib.rs"),
                target("orders", false, "tests/orders.rs"),
            ],
            crate_name: "shop".into(),
            edition: "2021".into(),
            features: Default::default(),
            members: Default::default(),
        }
    }

    #[test]
    fn a_test_is_found_inline_in_its_own_file_and_in_another_target() {
        let project = project(&[
            (
                "src/lib.rs",
                "mod pricing;\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn small() {}\n}\n",
            ),
            ("src/pricing.rs", "#[cfg(test)]\nmod tests;\n"),
            ("src/pricing/tests.rs", "\n#[test]\nfn  edge() {}\n"),
            ("tests/orders.rs", "mod common;\n#[test]\nfn big() {}\n"),
            (
                "tests/common/mod.rs",
                "pub mod checks {\n    #[test]\n    fn shared() {}\n}\n",
            ),
        ]);
        let at = |display: &str| place(&project, display);
        let place = |file: &str, line, column| {
            Some(Place {
                file: file.into(),
                at: Some((line, column)),
            })
        };
        assert_eq!(at("tests::small"), place("src/lib.rs", 5, 8));
        assert_eq!(
            at("pricing::tests::edge"),
            place("src/pricing/tests.rs", 3, 5)
        );
        assert_eq!(at("orders::big"), place("tests/orders.rs", 3, 4));
        assert_eq!(
            at("orders::common::checks::shared"),
            place("tests/common/mod.rs", 3, 8)
        );
        // A test a macro wrote: its module's file, with nowhere in it to point at.
        assert_eq!(
            at("tests::written_by_a_macro"),
            Some(Place {
                file: "src/lib.rs".into(),
                at: None
            })
        );
        assert_eq!(at("elsewhere::nothing"), None);
        std::fs::remove_dir_all(&project.workspace).ok();
    }
}
