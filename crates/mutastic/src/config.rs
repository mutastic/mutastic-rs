//! `mutastic.yaml`, in the package's directory: the project's own say over which mutants run, in
//! the Java edition's format, read as strictly: a line it doesn't understand is an error that names
//! it, never a guess.
//!
//! ```yaml
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
    pub plugins: Vec<PathBuf>,
    pub operators: Vec<String>,
}

impl Configuration {
    /// The package's configuration, or none when it has no `mutastic.yaml`.
    pub fn read(dir: &Path) -> Result<Configuration, String> {
        match std::fs::read_to_string(dir.join(FILE)) {
            Ok(text) => parse(&text),
            Err(_) => Ok(Configuration::default()),
        }
    }
}

/// Reads the file a line at a time, in the shape above.
pub fn parse(text: &str) -> Result<Configuration, String> {
    let mut config = Configuration::default();
    let mut section = "";
    let mut pending: Option<(bool, String)> = None;
    let mut last = 0;
    for (index, raw) in text.lines().enumerate() {
        let error = |what: &str| format!("{FILE} line {}: {what}", index + 1);
        let line = without_comment(raw);
        if line.trim().is_empty() {
            continue;
        }
        last = index + 1;
        if !line.starts_with(' ') {
            if let Some((mutate, target)) = &pending {
                return Err(error(&needs_reason(*mutate, target)));
            }
            section = match line.trim() {
                "rules:" => "rules",
                "plugins:" => "plugins",
                "filters:" => "filters",
                "operators:" => "operators",
                _ => return Err(error("expected rules:, plugins:, filters: or operators:")),
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
            "{FILE} line {last}: {}",
            needs_reason(*mutate, target)
        ));
    }
    Ok(config)
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
            ("rule:\n", "mutastic.yaml line 1: expected rules:, plugins:, filters: or operators:"),
            ("rules:\n  - skip: a::**\n", "mutastic.yaml line 2: the rule `skip: a::**` needs a `reason:` under it"),
            ("rules:\n  - drop: a\n", "mutastic.yaml line 2: a rule is `- skip: <pattern>` or `- mutate: <pattern>`, then `reason: …`"),
            ("operators:\n  stronger\n", "mutastic.yaml line 2: expected a list item, `- …`"),
        ] {
            assert_eq!(parse(text).unwrap_err(), error);
        }
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
