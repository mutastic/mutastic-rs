//! `mutastic.yaml`, the nearest at or above the package's directory: the project's own say over
//! which mutants run, in the Java edition's format, read as strictly: a line it doesn't understand
//! is an error that names it, never a guess.
//!
//! ```yaml
//! extends: ../mutastic.yaml
//! rules:
//!   - skip: shop::generated::**
//!     reason: protobuf output
//! plugins:
//!   - rules/strings.yaml
//! operators:
//!   - stronger
//!   - -stream/swap-match
//! ```
//!
//! Patterns name Rust paths where the Java edition names classes: `**` matches any depth, `*` one
//! name, and a function is its path, `shop::Money::add`, where Java writes `com.acme.Money#add`.
//! A plugin is a rules file of call operators; the Rust edition has no compiled plugins yet, so
//! `filters:`, which names a compiled plugin's filters, is an error.

use std::path::{Path, PathBuf};

pub const FILE: &str = "mutastic.yaml";

/// A project's decision: skip the mutants in some code, or mutate code a filter would skip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub mutate: bool,
    pub target: String,
    pub reason: String,
}

impl Rule {
    /// Whether the rule covers the function at `path` (`shop::Money::add`), or the item holding it
    /// (`shop::Money`), as a Java rule covers a method or its class.
    pub fn covers(&self, path: &str) -> bool {
        glob(&self.target, path)
            || path
                .rsplit_once("::")
                .is_some_and(|(holder, _)| glob(&self.target, holder))
    }

    /// `skip: shop::generated::**`, as the rule was written.
    pub fn written(&self) -> String {
        format!(
            "{}: {}",
            if self.mutate { "mutate" } else { "skip" },
            self.target
        )
    }

    /// `user: protobuf output (rule `skip: shop::generated::**`)`: the skip reason a mutant gets.
    pub fn skip_reason(&self) -> String {
        format!("user: {} (rule `{}`)", self.reason, self.written())
    }
}

/// `**` matches across `::`, `*` within one name.
fn glob(pattern: &str, text: &str) -> bool {
    fn matches(pattern: &[u8], text: &[u8]) -> bool {
        match pattern {
            [] => text.is_empty(),
            [b'*', b'*', rest @ ..] => (0..=text.len()).any(|at| matches(rest, &text[at..])),
            [b'*', rest @ ..] => (0..=text.len())
                .take_while(|&at| at == 0 || text[at - 1] != b':')
                .any(|at| matches(rest, &text[at..])),
            [first, rest @ ..] => text.first() == Some(first) && matches(rest, &text[1..]),
        }
    }
    matches(pattern.as_bytes(), text.as_bytes())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Configuration {
    pub rules: Vec<Rule>,
    /// Rules files, each found from the file that names it: once read, the path from where the
    /// first file was read.
    pub plugins: Vec<PathBuf>,
    pub operators: Vec<String>,
}

impl Configuration {
    /// The configuration in `dir`, or none when it has no `mutastic.yaml`: the file's own, then
    /// what it `extends:`, in turn. A file adds another's only when it says so, so one file and the
    /// ones it names are all that apply, never a cascade nobody wrote down.
    pub fn read(dir: &Path) -> Result<Configuration, String> {
        let base = normalized(dir);
        let first = base.join(FILE);
        if !first.is_file() {
            return Ok(Configuration::default());
        }
        let shown = |file: &Path| relative(&base, file).display().to_string();
        // The files read so far, in order, so one that comes back round is an error, not a loop.
        let mut chain = vec![first.clone()];
        let mut file = first;
        let mut config = Configuration::default();
        loop {
            let text = std::fs::read_to_string(&file)
                .map_err(|error| format!("{}: {error}", shown(&file)))?;
            let written = parse_as(&text, &shown(&file))?;
            let from = file.parent().map(Path::to_path_buf).unwrap_or_default();
            config = config.over(written.own.found_from(&from));
            let Some(extends) = written.extends else {
                return Ok(config);
            };
            let named = normalized(&from.join(&extends.path));
            if !named.is_file() {
                return Err(format!(
                    "{} line {}: extends {}, which isn't there",
                    shown(&file),
                    extends.line,
                    extends.path
                ));
            }
            if chain.contains(&named) {
                let names: Vec<String> = chain.iter().chain([&named]).map(|f| shown(f)).collect();
                return Err(format!(
                    "extends: goes round in a loop: {}",
                    names.join(" extends ")
                ));
            }
            chain.push(named.clone());
            file = named;
        }
    }

    /// This file's say over `parent`'s: its rules first, so a package's own reason is the one
    /// shown, then the parent's; the plugins of both; and its operators, if it chooses any, in
    /// place of the parent's, as `--operators` replaces them.
    fn over(self, parent: Configuration) -> Configuration {
        Configuration {
            rules: joined(self.rules, parent.rules),
            plugins: joined(self.plugins, parent.plugins),
            operators: if self.operators.is_empty() {
                parent.operators
            } else {
                self.operators
            },
        }
    }

    /// The same configuration, its plugins found from `dir`, the directory of the file that
    /// names them.
    fn found_from(self, dir: &Path) -> Configuration {
        let plugins = self
            .plugins
            .iter()
            .map(|plugin| normalized(&dir.join(plugin)))
            .collect();
        Configuration { plugins, ..self }
    }
}

/// `first` then `then`, each once.
fn joined<T: PartialEq>(first: Vec<T>, then: Vec<T>) -> Vec<T> {
    let mut both = first;
    for each in then {
        if !both.contains(&each) {
            both.push(each);
        }
    }
    both
}

/// Where a package's `mutastic.yaml` is: the nearest at or above the package's directory, no
/// higher than the top of its git repository, or, outside one, than the directory the run is on,
/// so a stray file above the project is never read. Both are absolute. The same package gets the
/// same rules from the workspace's top as from its own directory.
pub fn nearest(package: &Path, run: &Path) -> Option<PathBuf> {
    let top = package
        .ancestors()
        .find(|at| at.join(".git").exists())
        .unwrap_or(run);
    package
        .ancestors()
        .take_while(|at| at.starts_with(top))
        .find(|at| at.join(FILE).is_file())
        .map(Path::to_path_buf)
}

/// The path without `.` and with each `..` taking away the name before it, as Java's `normalize`
/// does: a file named two ways is one file.
fn normalized(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir
                if matches!(out.components().next_back(), Some(Component::Normal(_))) =>
            {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// The path to `to` from the directory `from`, both normalized: `../mutastic.yaml`.
pub fn relative(from: &Path, to: &Path) -> PathBuf {
    let from: Vec<_> = from.components().collect();
    let to: Vec<_> = to.components().collect();
    let shared = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut path: PathBuf = from[shared..].iter().map(|_| "..").collect();
    path.extend(&to[shared..]);
    path
}

/// One file as written: its own say, and the file it `extends:`, if any.
struct Written {
    own: Configuration,
    extends: Option<Extends>,
}

/// `extends: <path>`, on its line.
struct Extends {
    line: usize,
    path: String,
}

/// Reads the file a line at a time, in the shape above, as `mutastic.yaml` in the directory it's
/// read from.
pub fn parse(text: &str) -> Result<Configuration, String> {
    parse_as(text, FILE).map(|written| written.own)
}

/// Reads a file that its errors name as `shown`.
fn parse_as(text: &str, shown: &str) -> Result<Written, String> {
    let mut config = Configuration::default();
    let mut extends: Option<Extends> = None;
    let mut section = "";
    let mut pending: Option<(bool, String)> = None;
    let mut last = 0;
    for (index, raw) in text.lines().enumerate() {
        let error = |what: &str| format!("{shown} line {}: {what}", index + 1);
        let line = without_comment(raw);
        if line.trim().is_empty() {
            continue;
        }
        last = index + 1;
        if !line.starts_with(' ') {
            if let Some((mutate, target)) = &pending {
                return Err(error(&needs_reason(*mutate, target)));
            }
            if let Some(path) = line.trim().strip_prefix("extends:") {
                if extends.is_some() {
                    return Err(error("a file extends one other file, once"));
                }
                let path = value(path);
                if path.is_empty() {
                    return Err(error("`extends:` needs a path, such as ../mutastic.yaml"));
                }
                extends = Some(Extends {
                    line: index + 1,
                    path,
                });
                section = "";
                continue;
            }
            section = match line.trim() {
                "rules:" => "rules",
                "plugins:" => "plugins",
                "filters:" => "filters",
                "operators:" => "operators",
                _ => {
                    return Err(error(
                        "expected extends:, rules:, plugins:, filters: or operators:",
                    ))
                }
            };
            continue;
        }
        let item = line.trim();
        if section == "rules" {
            if let (Some(reason), Some((mutate, target))) =
                (item.strip_prefix("reason:"), pending.take())
            {
                config.rules.push(Rule {
                    mutate,
                    target,
                    reason: value(reason),
                });
                continue;
            }
            let listed = listed(item).ok_or_else(|| error("expected a list item, `- …`"))?;
            let (verb, target) = listed.split_once(':').unwrap_or(("", ""));
            if verb != "skip" && verb != "mutate" {
                return Err(error(
                    "a rule is `- skip: <pattern>` or `- mutate: <pattern>`, then `reason: …`",
                ));
            }
            if let Some((mutate, target)) = &pending {
                return Err(error(&needs_reason(*mutate, target)));
            }
            pending = Some((verb == "mutate", value(target)));
            continue;
        }
        let listed = listed(item).ok_or_else(|| error("expected a list item, `- …`"))?;
        match section {
            "plugins" => config.plugins.push(PathBuf::from(listed)),
            "operators" => config.operators.push(listed),
            "filters" => {
                return Err(error(&format!(
                    "the filter {listed}: filters come from compiled plugins, which the Rust edition doesn't have yet"
                )))
            }
            _ => return Err(error("a list item needs a section above it")),
        }
    }
    if let Some((mutate, target)) = &pending {
        return Err(format!(
            "{shown} line {last}: {}",
            needs_reason(*mutate, target)
        ));
    }
    Ok(Written {
        own: config,
        extends,
    })
}

/// Every skip says why: a rule without a reason is an error, not "no reason given".
fn needs_reason(mutate: bool, target: &str) -> String {
    format!(
        "the rule `{}: {target}` needs a `reason:` under it",
        if mutate { "mutate" } else { "skip" }
    )
}

fn listed(item: &str) -> Option<String> {
    item.strip_prefix("- ").map(value)
}

/// A value, without the quotes around it.
pub fn value(text: &str) -> String {
    let value = text.trim();
    let quoted = value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')));
    if quoted {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

/// A `#` at the start or after a space starts a comment, as in YAML.
pub fn without_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    for at in 0..bytes.len() {
        if bytes[at] == b'#' && (at == 0 || bytes[at - 1] == b' ') {
            return &line[..at];
        }
    }
    line
}

/// The operators `entries` choose, as `--operators` and `operators:` read them: a set
/// (`defaults`, `stronger`, `extended`, `all`), a namespace (`stream`) or an operator's id, each
/// adding to the ones before, or after `-` taking away; a rules file's operators count too.
pub fn resolve(entries: &[String], from: &str) -> Result<Vec<&'static str>, String> {
    use crate::sites;
    let rules = crate::callrules::loaded();
    let mut chosen: Vec<&'static str> = Vec::new();
    for entry in entries {
        let (remove, name) = match entry.strip_prefix('-') {
            Some(name) => (true, name),
            None => (false, entry.as_str()),
        };
        let matched: Vec<&'static str> = match name {
            "defaults" | "stronger" | "extended" | "all" => {
                let set = match name {
                    "defaults" => sites::DEFAULTS,
                    "stronger" => sites::STRONGER,
                    _ => sites::ALL,
                };
                let tier = if name == "all" { "extended" } else { name };
                set.iter()
                    .copied()
                    .chain(rules.iter().filter(|r| r.in_tier(tier)).map(|r| r.id))
                    .collect()
            }
            _ => sites::ALL
                .iter()
                .copied()
                .chain(rules.iter().map(|r| r.id))
                .filter(|id| {
                    *id == name || (!name.contains('/') && id.starts_with(&format!("{name}/")))
                })
                .collect(),
        };
        if matched.is_empty() {
            return Err(format!("{from}: {name} isn't an operator mutastic has"));
        }
        if remove {
            chosen.retain(|id| !matched.contains(id));
        } else {
            for id in matched {
                if !chosen.contains(&id) {
                    chosen.push(id);
                }
            }
        }
    }
    Ok(chosen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_java_editions_shape_is_read_with_rust_paths() {
        let config = parse(
            "# the project's say\nrules:\n  - skip: shop::generated::**\n    reason: protobuf output\n\
             \x20 - mutate: 'shop::Money::fmt'\n    reason: shown to customers # why\n\
             plugins:\n  - rules/strings.yaml\noperators:\n  - stronger\n  - -stream/swap-match\n",
        )
        .unwrap();
        assert_eq!(config.rules.len(), 2);
        assert_eq!(
            config.rules[0].skip_reason(),
            "user: protobuf output (rule `skip: shop::generated::**`)"
        );
        assert!(config.rules[1].mutate);
        assert_eq!(config.plugins, [PathBuf::from("rules/strings.yaml")]);
        assert_eq!(config.operators, ["stronger", "-stream/swap-match"]);
    }

    #[test]
    fn what_it_doesnt_understand_is_an_error_that_names_the_line() {
        for (text, error) in [
            ("rule:\n", "mutastic.yaml line 1: expected extends:, rules:, plugins:, filters: or operators:"),
            ("extends: a.yaml\nextends: b.yaml\n", "mutastic.yaml line 2: a file extends one other file, once"),
            ("extends: ''\n", "mutastic.yaml line 1: `extends:` needs a path, such as ../mutastic.yaml"),
            ("extends: a.yaml\n  - x\n", "mutastic.yaml line 2: a list item needs a section above it"),
            ("rules:\n  - skip: a::**\n", "mutastic.yaml line 2: the rule `skip: a::**` needs a `reason:` under it"),
            ("rules:\n  - drop: a\n", "mutastic.yaml line 2: a rule is `- skip: <pattern>` or `- mutate: <pattern>`, then `reason: …`"),
            ("operators:\n  stronger\n", "mutastic.yaml line 2: expected a list item, `- …`"),
        ] {
            assert_eq!(parse(text).unwrap_err(), error);
        }
    }

    /// A directory of its own for one test, with `files` written in it.
    fn written(label: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mutastic-config-{label}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        for (path, text) in files {
            let file = dir.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        }
        dir
    }

    #[test]
    fn a_file_adds_what_it_extends_after_its_own() {
        let top =
            "rules:\n  - skip: shop::**\n    reason: top\n  - skip: a::**\n    reason: both\n\
                   plugins:\n  - rules/top.yaml\noperators:\n  - stronger\n";
        let dir = written(
            "extends",
            &[
                ("mutastic.yaml", top),
                (
                    "shop/mutastic.yaml",
                    "extends: ../mutastic.yaml\nrules:\n  - skip: shop::legacy::**\n    reason: own\n\
                     \x20 - skip: a::**\n    reason: both\nplugins:\n  - rules/shop.yaml\n  - ../rules/top.yaml\n",
                ),
                (
                    "billing/mutastic.yaml",
                    "extends: ../mutastic.yaml\noperators:\n  - defaults\n",
                ),
            ],
        );
        let shop = Configuration::read(&dir.join("shop")).unwrap();
        let reasons: Vec<&str> = shop.rules.iter().map(|r| r.reason.as_str()).collect();
        assert_eq!(reasons, ["own", "both", "top"]);
        assert_eq!(
            shop.plugins,
            [dir.join("shop/rules/shop.yaml"), dir.join("rules/top.yaml")]
        );
        assert_eq!(shop.operators, ["stronger"]);
        let billing = Configuration::read(&dir.join("billing")).unwrap();
        assert_eq!(billing.operators, ["defaults"]);
        assert_eq!(billing.rules.len(), 2);
        // Only what a file names applies: a package's file without extends: is all there is.
        std::fs::write(
            dir.join("billing/mutastic.yaml"),
            "operators:\n  - defaults\n",
        )
        .unwrap();
        assert!(Configuration::read(&dir.join("billing"))
            .unwrap()
            .rules
            .is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn extends_names_a_file_that_is_there_and_never_comes_round() {
        let dir = written(
            "extends-errors",
            &[
                ("missing/mutastic.yaml", "extends: ../top.yaml\n"),
                ("a/mutastic.yaml", "extends: ../mutastic.yaml\n"),
                ("mutastic.yaml", "extends: a/mutastic.yaml\n"),
                ("deep/mutastic.yaml", "extends: ../bad.yaml\n"),
                ("bad.yaml", "rules:\n  - skip: x::**\n"),
            ],
        );
        let error = |sub: &str| Configuration::read(&dir.join(sub)).unwrap_err();
        assert_eq!(
            error("missing"),
            "mutastic.yaml line 1: extends ../top.yaml, which isn't there"
        );
        assert_eq!(
            error("a"),
            "extends: goes round in a loop: mutastic.yaml extends ../mutastic.yaml extends mutastic.yaml"
        );
        // An error in the file extended names that file, from the first.
        assert_eq!(
            error("deep"),
            "../bad.yaml line 2: the rule `skip: x::**` needs a `reason:` under it"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_nearest_file_is_looked_for_up_to_the_repository_or_the_run() {
        let dir = written(
            "nearest",
            &[("mutastic.yaml", "rules:\n"), ("shop/src/lib.rs", "")],
        );
        let dir = std::fs::canonicalize(dir).unwrap();
        let shop = dir.join("shop");
        // Outside a repository, no higher than the run's directory.
        assert_eq!(nearest(&shop, &dir), Some(dir.clone()));
        assert_eq!(nearest(&shop, &shop), None);
        // In one, up to its top, however it's run.
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        assert_eq!(nearest(&shop, &shop), Some(dir.clone()));
        // The package's own file is nearer.
        std::fs::write(shop.join("mutastic.yaml"), "rules:\n").unwrap();
        assert_eq!(nearest(&shop, &dir), Some(shop.clone()));
        // Never above the repository's top.
        std::fs::remove_file(shop.join("mutastic.yaml")).unwrap();
        std::fs::create_dir_all(shop.join(".git")).unwrap();
        assert_eq!(nearest(&shop, &shop), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn paths_are_named_from_the_first_files_directory() {
        assert_eq!(
            relative(Path::new("/a/b"), Path::new("/a/mutastic.yaml")),
            Path::new("../mutastic.yaml")
        );
        assert_eq!(relative(Path::new("/a"), Path::new("/a")), Path::new(""));
        assert_eq!(normalized(Path::new("./a/b/../c")), Path::new("a/c"));
        assert_eq!(normalized(Path::new("../a")), Path::new("../a"));
    }

    #[test]
    fn patterns_match_paths_as_the_java_editions_match_classes() {
        let rule = |target: &str| Rule {
            mutate: false,
            target: target.into(),
            reason: "r".into(),
        };
        assert!(rule("shop::generated::**").covers("shop::generated::proto::Order::new"));
        assert!(rule("shop::*::Money").covers("shop::money::Money::add"));
        assert!(rule("shop::*").covers("shop::total"));
        assert!(!rule("shop::*").covers("shop::money::Money::add"));
        assert!(rule("shop::Money::fmt").covers("shop::Money::fmt"));
        assert!(!rule("shop::Money::fmt").covers("shop::Money::add"));
        assert!(!rule("shop::*::Money").covers("shop::a::b::Money::add"));
    }

    #[test]
    fn operators_are_chosen_as_the_java_edition_chooses_them() {
        let chosen = resolve(&["defaults".into(), "-mutastic/boundary".into()], "t").unwrap();
        assert!(!chosen.contains(&crate::sites::BOUNDARY));
        assert!(chosen.contains(&crate::sites::MATH));
        let stream = resolve(&["stream".into()], "t").unwrap();
        assert!(!stream.is_empty() && stream.iter().all(|id| id.starts_with("stream/")));
        assert_eq!(
            resolve(&["nope".into()], "--operators").unwrap_err(),
            "--operators: nope isn't an operator mutastic has"
        );
    }
}
