//! Where a test is written: the file that holds it and the line and column of its name, found from
//! the name a run shows it by (`tests::small_orders_pay_full_price` in the library,
//! `pricing::big_orders_get_ten_percent_off` in `tests/pricing.rs`) by following its modules from
//! its target's crate root, as the compiler does. `mutation.json` keys its tests by file, as the
//! Java edition's are, and an HTML report shows each test where it is.

use std::collections::HashMap;
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
/// For one test; [`Places`] finds many, reading each file once.
pub fn place(project: &Project, display: &str) -> Option<Place> {
    Places::new(project).place(display)
}

/// Where a package's tests are, each file read and indexed once however many tests it holds: a
/// report places every test that covers a mutant, thousands in a big suite, and reading and
/// parsing a file again for each of its tests made writing it quadratic.
pub struct Places<'a> {
    project: &'a Project,
    files: std::cell::RefCell<HashMap<PathBuf, Option<std::rc::Rc<Indexed>>>>,
}

/// What a file declares, by the inline modules it's within (`tests`, or `` for the file itself):
/// each function's name with the line and column of it, and each module with where it's written.
#[derive(Default)]
struct Indexed {
    functions: HashMap<(String, String), (usize, usize)>,
    modules: HashMap<(String, String), Declared>,
}

/// A `mod name` a file declares: inline, with its items, or in a file of its own, with the path
/// its `#[path]` names.
enum Declared {
    Inline,
    File(Option<String>),
}

impl<'a> Places<'a> {
    pub fn new(project: &'a Project) -> Places<'a> {
        Places {
            project,
            files: std::cell::RefCell::default(),
        }
    }

    /// Where the test a run shows as `display` is, as [`place`] says.
    pub fn place(&self, display: &str) -> Option<Place> {
        let library = self
            .project
            .targets
            .iter()
            .filter(|target| target.lib)
            .map(|target| (target.root.as_path(), display));
        let others = self
            .project
            .targets
            .iter()
            .filter(|target| !target.lib)
            .filter_map(|target| {
                let path = display.strip_prefix(&target.name)?.strip_prefix("::")?;
                Some((target.root.as_path(), path))
            });
        let found: Vec<Place> = library
            .chain(others)
            .filter_map(|(root, path)| self.find(root, path))
            .collect();
        // Where its function is declared, before a module that only holds it.
        found
            .iter()
            .find(|place| place.at.is_some())
            .or_else(|| found.first())
            .cloned()
    }

    /// `path` (`tests::small_orders_pay_full_price`) from the crate root `root`: each module in
    /// turn, inline or in its own file, then the function in the last.
    fn find(&self, root: &Path, path: &str) -> Option<Place> {
        let workspace = &self.project.workspace;
        let (modules, name) = path.rsplit_once("::").unwrap_or(("", path));
        let mut file = root.to_path_buf();
        let mut indexed = self.indexed(&file)?;
        // The directory a `mod name;` here is looked for in, and the inline modules it's within.
        let mut dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut inline: Vec<String> = Vec::new();
        for module in modules.split("::").filter(|module| !module.is_empty()) {
            let declared = indexed
                .modules
                .get(&(inline.join("::"), module.to_string()))?;
            let written = match declared {
                Declared::Inline => {
                    inline.push(module.to_string());
                    continue;
                }
                Declared::File(written) => written,
            };
            let mut base = dir.clone();
            base.extend(&inline);
            let next = match written {
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
            indexed = self.indexed(&next)?;
            dir = if next.file_name().is_some_and(|name| name == "mod.rs") {
                next.parent().map(Path::to_path_buf).unwrap_or_default()
            } else {
                next.with_extension("")
            };
            file = next;
            inline.clear();
        }
        let at = indexed
            .functions
            .get(&(inline.join("::"), name.to_string()))
            .copied();
        Some(Place { file, at })
    }

    /// The file at `file`, relative to the workspace, read and indexed the first time it's asked
    /// for; `None` when it can't be read.
    fn indexed(&self, file: &Path) -> Option<std::rc::Rc<Indexed>> {
        if let Some(known) = self.files.borrow().get(file) {
            return known.clone();
        }
        let indexed = std::fs::read_to_string(self.project.workspace.join(file))
            .ok()
            .map(|text| std::rc::Rc::new(index(&text)));
        self.files
            .borrow_mut()
            .insert(file.to_path_buf(), indexed.clone());
        indexed
    }
}

/// Every function and module `text` declares, at any depth of inline modules; the first of a name
/// in a module, as a search from the top finds it.
fn index(text: &str) -> Indexed {
    let tree = SourceFile::parse(text, Edition::CURRENT).tree();
    let starts = crate::sites::line_starts(text);
    let mut indexed = Indexed::default();
    let mut todo: Vec<(String, Vec<ast::Item>)> = vec![(String::new(), tree.items().collect())];
    while let Some((within, items)) = todo.pop() {
        for item in items {
            match item {
                ast::Item::Fn(function) => {
                    if let Some(name) = function.name() {
                        let offset = usize::from(name.syntax().text_range().start());
                        let line = crate::sites::line_of(&starts, offset);
                        let column = text[starts[line - 1]..offset].chars().count() + 1;
                        indexed
                            .functions
                            .entry((within.clone(), name.text().to_string()))
                            .or_insert((line, column));
                    }
                }
                ast::Item::Module(module) => {
                    let Some(name) = module.name().map(|name| name.text().to_string()) else {
                        continue;
                    };
                    let key = (within.clone(), name.clone());
                    if indexed.modules.contains_key(&key) {
                        continue;
                    }
                    match module.item_list() {
                        Some(list) => {
                            indexed.modules.insert(key, Declared::Inline);
                            let inner = if within.is_empty() {
                                name
                            } else {
                                format!("{within}::{name}")
                            };
                            todo.push((inner, list.items().collect()));
                        }
                        None => {
                            let written = crate::sites::path_attribute(&module);
                            indexed.modules.insert(key, Declared::File(written));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    indexed
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
            version: "0.1.0".into(),
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
            properties: Default::default(),
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
