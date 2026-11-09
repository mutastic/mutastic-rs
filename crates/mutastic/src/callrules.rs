//! Rules files: call operators as data, in the Java edition's format, listed under `plugins:` in
//! `mutastic.yaml`. Each list item is one operator:
//!
//! ```yaml
//! - id: acme/remove-trim
//!   explanation: A call that trims a string is removed. It goes unnoticed when no test passes text with spaces around it.
//!   owners: str, String
//!   methods: trim, trim_start, trim_end
//!   rewrite: remove
//!   changed: `.{name}()` removed
//!   headline: no test passes {method} text with spaces for `.{name}()` to remove
//!   help: add a test whose input has spaces around it, and assert they're gone
//! ```
//!
//! The Rust edition reads source, without types, so a rule matches a method call by its name, and
//! `owners` (Rust types, where Java names JVM classes) says what the rule is for rather than being
//! checked: a rewrite that doesn't fit the receiver's type doesn't compile, and the build withdraws
//! it as unviable. Of the rewrites, `remove` and `rename a->b, c->d` are in the Rust edition;
//! `descriptors` and `since` are the JVM's, and an error here.

use std::path::Path;
use std::sync::OnceLock;

/// One call operator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallRule {
    pub id: &'static str,
    pub explanation: String,
    pub tier: String,
    pub owners: Vec<String>,
    pub methods: Vec<String>,
    pub rewrite: Rewrite,
    pub changed: String,
    pub headline: String,
    pub help: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rewrite {
    /// `recv.name(..)` becomes `recv`.
    Remove,
    /// `recv.a(..)` becomes `recv.b(..)`.
    Rename(Vec<(String, String)>),
}

impl CallRule {
    /// What a rename calls instead of `name`: `None` when the call is removed.
    pub fn to(&self, name: &str) -> Option<&str> {
        match &self.rewrite {
            Rewrite::Remove => None,
            Rewrite::Rename(pairs) => pairs
                .iter()
                .find(|(from, _)| from == name)
                .map(|(_, to)| to.as_str()),
        }
    }

    /// `text` with `{name}`, `{to}` and `{method}` filled in.
    pub fn fill(&self, text: &str, name: &str, method: &str) -> String {
        text.replace("{name}", name)
            .replace("{to}", self.to(name).unwrap_or(""))
            .replace("{method}", method)
    }

    /// Whether the set `tier` (`defaults`, `stronger`, `extended`) has it.
    pub fn in_tier(&self, tier: &str) -> bool {
        match tier {
            "defaults" => self.tier == "defaults",
            "stronger" => self.tier != "extended",
            _ => true,
        }
    }
}

/// The rules files' operators, once read for this run.
static LOADED: OnceLock<Vec<CallRule>> = OnceLock::new();

/// Keeps the operators read for this run, for the operators and the cards to find. The first call
/// wins.
pub fn load(rules: Vec<CallRule>) {
    let _ = LOADED.set(rules);
}

pub fn loaded() -> &'static [CallRule] {
    LOADED.get().map_or(&[], Vec::as_slice)
}

/// The rule with operator id `id`, if a rules file has it.
pub fn find(id: &str) -> Option<&'static CallRule> {
    loaded().iter().find(|rule| rule.id == id)
}

/// Reads a rules file.
pub fn read(file: &Path) -> Result<Vec<CallRule>, String> {
    let name = file.display().to_string();
    let text = std::fs::read_to_string(file)
        .map_err(|_| format!("mutastic.yaml names the plugin {name}, which isn't there"))?;
    parse(&text, &name)
}

pub fn parse(text: &str, name: &str) -> Result<Vec<CallRule>, String> {
    let mut rules = Vec::new();
    let mut fields: Vec<(String, String, usize)> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = crate::config::without_comment(raw);
        if line.trim().is_empty() {
            continue;
        }
        let mut item = line.trim();
        if let Some(rest) = item.strip_prefix("- ") {
            if !fields.is_empty() {
                rules.push(rule(std::mem::take(&mut fields), name)?);
            }
            item = rest;
        } else if fields.is_empty() {
            return Err(format!(
                "{name} line {}: each operator starts with `- id: …`",
                index + 1
            ));
        }
        let Some((key, value)) = item.split_once(':') else {
            return Err(format!(
                "{name} line {}: expected `field: value`",
                index + 1
            ));
        };
        fields.push((
            key.trim().to_string(),
            crate::config::value(value),
            index + 1,
        ));
    }
    if !fields.is_empty() {
        rules.push(rule(fields, name)?);
    }
    Ok(rules)
}

fn rule(fields: Vec<(String, String, usize)>, name: &str) -> Result<CallRule, String> {
    let first = fields.first().map_or(0, |(_, _, line)| *line);
    let error = |line: usize, what: String| format!("{name} line {line}: {what}");
    let get = |key: &str| {
        fields
            .iter()
            .find(|(k, _, _)| k == key)
            .map(|(_, value, _)| value.clone())
    };
    for (key, _, line) in &fields {
        match key.as_str() {
            "id" | "explanation" | "tier" | "owners" | "methods" | "rewrite" | "source"
            | "changed" | "headline" | "help" => {}
            "descriptors" | "since" => {
                return Err(error(
                    *line,
                    format!("{key} is the JVM's: the Rust edition matches a call by its name"),
                ))
            }
            other => return Err(error(*line, format!("unknown field {other}"))),
        }
    }
    let required = |value: Option<String>, key: &str| {
        value
            .filter(|value| !value.is_empty())
            .ok_or_else(|| error(first, format!("an operator needs `{key}:`")))
    };
    let id = required(get("id"), "id")?;
    let valid = id.split_once('/').is_some_and(|(space, op)| {
        !space.is_empty()
            && !op.is_empty()
            && id.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '/' | '-' | '_' | '.')
            })
    });
    if !valid {
        return Err(error(
            first,
            format!("{id}: an id is `namespace/name`, lower case"),
        ));
    }
    let list = |text: String| -> Vec<String> {
        text.split(',')
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect()
    };
    let rewrite_text = required(get("rewrite"), "rewrite")?;
    let rewrite = if rewrite_text == "remove" {
        Rewrite::Remove
    } else if let Some(pairs) = rewrite_text.strip_prefix("rename ") {
        let pairs: Option<Vec<(String, String)>> = pairs
            .split(',')
            .map(|pair| {
                let (from, to) = pair.split_once("->")?;
                Some((from.trim().to_string(), to.trim().to_string()))
            })
            .collect();
        Rewrite::Rename(
            pairs.ok_or_else(|| error(first, format!("{id}: a rename is `rename a->b, c->d`")))?,
        )
    } else {
        return Err(error(
            first,
            format!(
                "{id}: the rewrite `{rewrite_text}` isn't in the Rust edition yet (remove, rename a->b)"
            ),
        ));
    };
    let tier = get("tier").unwrap_or_else(|| "defaults".into());
    if !matches!(tier.as_str(), "defaults" | "stronger" | "extended") {
        return Err(error(
            first,
            format!("{id}: tier is defaults, stronger or extended"),
        ));
    }
    let methods = match &rewrite {
        Rewrite::Rename(pairs) => pairs.iter().map(|(from, _)| from.clone()).collect(),
        Rewrite::Remove => list(required(get("methods"), "methods")?),
    };
    Ok(CallRule {
        id: Box::leak(id.into_boxed_str()),
        explanation: required(get("explanation"), "explanation")?,
        tier,
        owners: list(get("owners").unwrap_or_default()),
        methods,
        rewrite,
        changed: get("changed").unwrap_or_else(|| "`.{name}()` changed".into()),
        headline: required(get("headline"), "headline")?,
        help: required(get("help"), "help")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRIM: &str = "# trims\n- id: acme/remove-trim\n  explanation: A call that trims is removed.\n  \
        owners: str, String\n  methods: trim, trim_start\n  rewrite: remove\n  \
        changed: `.{name}()` removed\n  headline: no test passes {method} text with spaces for `.{name}()`\n  \
        help: add a test with spaces\n- id: acme/swap-trim\n  explanation: e\n  methods: trim_start\n  \
        rewrite: rename trim_start->trim_end\n  tier: stronger\n  headline: h {to}\n  help: x\n";

    #[test]
    fn a_rules_file_is_read_in_the_java_editions_shape() {
        let rules = parse(TRIM, "trims.yaml").unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].id, "acme/remove-trim");
        assert_eq!(rules[0].methods, ["trim", "trim_start"]);
        assert_eq!(rules[0].owners, ["str", "String"]);
        assert_eq!(
            rules[0].fill(&rules[0].headline, "trim", "`clean()`"),
            "no test passes `clean()` text with spaces for `.trim()`"
        );
        assert_eq!(rules[1].to("trim_start"), Some("trim_end"));
        assert_eq!(
            rules[1].fill(&rules[1].headline, "trim_start", ""),
            "h trim_end"
        );
        assert!(rules[1].in_tier("stronger") && !rules[1].in_tier("defaults"));
    }

    #[test]
    fn what_the_rust_edition_cant_do_is_an_error() {
        for (text, error) in [
            ("- id: a/b\n  descriptors: (I)V\n", "r.yaml line 2: descriptors is the JVM's: the Rust edition matches a call by its name"),
            ("- id: a/b\n  explanation: e\n  methods: m\n  rewrite: keep argument\n  headline: h\n  help: x\n", "r.yaml line 1: a/b: the rewrite `keep argument` isn't in the Rust edition yet (remove, rename a->b)"),
            ("- id: A\n  rewrite: remove\n", "r.yaml line 1: A: an id is `namespace/name`, lower case"),
            ("- id: a/b\n  colour: red\n", "r.yaml line 2: unknown field colour"),
        ] {
            assert_eq!(parse(text, "r.yaml").unwrap_err(), error);
        }
    }
}
