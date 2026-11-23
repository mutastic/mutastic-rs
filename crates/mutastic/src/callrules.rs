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
//! it as unviable. The rewrites are the Java edition's, on Rust's syntax: `remove`, `rename a->b,
//! c->d`, `keep argument`, `unchecked arithmetic` and `call name(TYPES) with A, B`, whose `TYPES`
//! are for the reader, as `owners` is. `omit last argument` calls a shorter overload, and Rust has
//! none; `descriptors` and `since` are the JVM's. Each is an error here.

use std::path::Path;
use std::sync::OnceLock;

use crate::failure::{Code, Failure};

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
    /// `recv.name(arg)` becomes `arg`.
    KeepArgument,
    /// `recv.checked_add(n)` becomes `Some(recv.wrapping_add(n))`, and `recv.saturating_add(n)`
    /// `recv.wrapping_add(n)`.
    Unchecked,
    /// `recv.name(..)` becomes `recv.other(A, B)`.
    Call {
        name: String,
        arguments: Vec<String>,
    },
}

/// The wrapping method a checked or saturating one becomes, and whether its result goes in `Some`:
/// `checked_add` is `wrapping_add` in `Some`, `saturating_add` is `wrapping_add`. `None` for any
/// other name, and for an operation with no wrapping form of the same arguments.
pub fn unchecked(name: &str) -> Option<(String, bool)> {
    /// The operations a `checked_` call can be made unchecked for: `Some(a.wrapping_add(b))`.
    const CHECKED: &[&str] = &[
        "add", "sub", "mul", "div", "rem", "pow", "neg", "abs", "shl", "shr",
    ];
    /// The operations a `saturating_` call can wrap for instead.
    const SATURATING: &[&str] = &["add", "sub", "mul", "div", "pow", "neg", "abs"];
    let (operation, in_some) = match name.strip_prefix("checked_") {
        Some(operation) => (operation, true),
        None => (name.strip_prefix("saturating_")?, false),
    };
    let known = if in_some { CHECKED } else { SATURATING };
    known
        .contains(&operation)
        .then(|| (format!("wrapping_{operation}"), in_some))
}

impl CallRule {
    /// What is called instead of `name`, a rename's other method or a `call` rewrite's
    /// `first(u64::MAX)`: `None` when nothing is, the call removed or its argument kept.
    pub fn to(&self, name: &str) -> Option<String> {
        match &self.rewrite {
            Rewrite::Remove | Rewrite::KeepArgument => None,
            Rewrite::Rename(pairs) => pairs
                .iter()
                .find(|(from, _)| from == name)
                .map(|(_, to)| to.clone()),
            Rewrite::Unchecked => unchecked(name).map(|(wrapping, _)| wrapping),
            Rewrite::Call { name, arguments } => Some(format!("{name}({})", arguments.join(", "))),
        }
    }

    /// `text` with `{name}`, `{to}` and `{method}` filled in.
    pub fn fill(&self, text: &str, name: &str, method: &str) -> String {
        text.replace("{name}", name)
            .replace("{to}", &self.to(name).unwrap_or_default())
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

/// Reads a rules file, named as a plugin `from` a place in a `mutastic.yaml`: `mutastic.yaml line 4`.
pub fn read(file: &Path, from: &str) -> Result<Vec<CallRule>, Failure> {
    let name = file.display().to_string();
    let text = std::fs::read_to_string(file).map_err(|_| {
        Code::Config.with(format!(
            "{from}: the plugin {name} isn't there. A plugin's path is from the directory of the \
             file that names it."
        ))
    })?;
    parse(&text, &name).map_err(|error| Code::Config.with(error))
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
    const FIELDS: &[&str] = &[
        "id",
        "explanation",
        "tier",
        "owners",
        "methods",
        "rewrite",
        "source",
        "changed",
        "headline",
        "help",
    ];
    for (key, _, line) in &fields {
        match key.as_str() {
            known if FIELDS.contains(&known) => {}
            "descriptors" | "since" => {
                return Err(error(
                    *line,
                    format!("{key} is the JVM's: the Rust edition matches a call by its name"),
                ))
            }
            other => {
                return Err(error(
                    *line,
                    format!(
                        "unknown field {other}.{} An operator's fields are {}.",
                        crate::similar::offered(other, FIELDS.iter().copied()),
                        crate::mistyped::listed(FIELDS)
                    ),
                ))
            }
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
    let rewrite = rewrite(&rewrite_text).map_err(|why| error(first, format!("{id}: {why}")))?;
    let tier = get("tier").unwrap_or_else(|| "defaults".into());
    if !matches!(tier.as_str(), "defaults" | "stronger" | "extended") {
        return Err(error(
            first,
            format!(
                "{id}: tier is defaults, stronger or extended, not {tier}.{}",
                crate::similar::offered(&tier, ["defaults", "stronger", "extended"])
            ),
        ));
    }
    let methods = match &rewrite {
        Rewrite::Rename(pairs) => pairs.iter().map(|(from, _)| from.clone()).collect(),
        _ => list(required(get("methods"), "methods")?),
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

/// What `rewrite:` asks for, or why it can't be done.
fn rewrite(text: &str) -> Result<Rewrite, String> {
    match text {
        "remove" => return Ok(Rewrite::Remove),
        "keep argument" => return Ok(Rewrite::KeepArgument),
        "unchecked arithmetic" => return Ok(Rewrite::Unchecked),
        "omit last argument" => {
            return Err(
                "the rewrite `omit last argument` calls a shorter overload, and Rust has none"
                    .into(),
            )
        }
        _ => {}
    }
    if let Some(pairs) = text.strip_prefix("rename ") {
        let pairs: Option<Vec<(String, String)>> = pairs
            .split(',')
            .map(|pair| {
                let (from, to) = pair.split_once("->")?;
                Some((from.trim().to_string(), to.trim().to_string()))
            })
            .collect();
        return pairs
            .map(Rewrite::Rename)
            .ok_or_else(|| "a rename is `rename a->b, c->d`".into());
    }
    if let Some(call) = text.strip_prefix("call ") {
        return called_instead(call);
    }
    Err(format!(
        "a rewrite is `remove`, `unchecked arithmetic`, `keep argument`, `rename a->b, …` or \
         `call name(TYPES) with A, B`, not {text}"
    ))
}

/// `take(usize) with usize::MAX`: the method to call, and what with. The types between the
/// brackets are the reader's, where the Java edition needs the JVM's to find the method.
fn called_instead(call: &str) -> Result<Rewrite, String> {
    let shape = || "a call is `call name(TYPES) with A, B`".to_string();
    let (name, rest) = call.split_once('(').ok_or_else(shape)?;
    let (_types, with) = rest.split_once(')').ok_or_else(shape)?;
    let named = !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_');
    let arguments = match with.trim() {
        "" => Vec::new(),
        given => given
            .strip_prefix("with ")
            .ok_or_else(shape)?
            .split(',')
            .map(|argument| argument.trim().to_string())
            .collect(),
    };
    if !named {
        return Err(shape());
    }
    if let Some(argument) = arguments.iter().find(|argument| !constant(argument)) {
        return Err(format!(
            "a call's argument is a number, `true`, `false`, `None`, a constant by its path \
             (`u64::MAX`) or a call of no arguments (`String::new()`), not `{argument}`"
        ));
    }
    Ok(Rewrite::Call {
        name: name.to_string(),
        arguments,
    })
}

/// Whether `argument` is one a `call` rewrite can pass: a number, or a path, called with no
/// arguments or not. So it reads no variable a call site may not have, but for a constant it names.
fn constant(argument: &str) -> bool {
    let path = argument.strip_suffix("()").unwrap_or(argument);
    let number = argument.strip_prefix('-').unwrap_or(argument);
    let word = |text: &str| {
        !text.is_empty()
            && text.chars().all(|c| c.is_alphanumeric() || c == '_')
            && !text.starts_with(|c: char| c.is_ascii_digit())
    };
    let is_number = number.starts_with(|c: char| c.is_ascii_digit())
        && number
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
    is_number || path.split("::").all(word)
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
        assert_eq!(rules[1].to("trim_start").as_deref(), Some("trim_end"));
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
            ("- id: a/b\n  explanation: e\n  methods: m\n  rewrite: omit last argument\n  headline: h\n  help: x\n", "r.yaml line 1: a/b: the rewrite `omit last argument` calls a shorter overload, and Rust has none"),
            ("- id: a/b\n  explanation: e\n  methods: m\n  rewrite: plain division\n  headline: h\n  help: x\n", "r.yaml line 1: a/b: a rewrite is `remove`, `unchecked arithmetic`, `keep argument`, `rename a->b, …` or `call name(TYPES) with A, B`, not plain division"),
            ("- id: a/b\n  explanation: e\n  methods: m\n  rewrite: rename a>b\n  headline: h\n  help: x\n", "r.yaml line 1: a/b: a rename is `rename a->b, c->d`"),
            ("- id: a/b\n  explanation: e\n  methods: m\n  rewrite: call take with 0\n  headline: h\n  help: x\n", "r.yaml line 1: a/b: a call is `call name(TYPES) with A, B`"),
            ("- id: a/b\n  explanation: e\n  methods: m\n  rewrite: call take(usize) with limit + 1\n  headline: h\n  help: x\n", "r.yaml line 1: a/b: a call's argument is a number, `true`, `false`, `None`, a constant by its path (`u64::MAX`) or a call of no arguments (`String::new()`), not `limit + 1`"),
            ("- id: A\n  rewrite: remove\n", "r.yaml line 1: A: an id is `namespace/name`, lower case"),
            ("- id: a/b\n  colour: red\n", "r.yaml line 2: unknown field colour. An operator's fields are id, explanation, tier, owners, methods, rewrite, source, changed, headline or help."),
            ("- id: a/b\n  metods: m\n", "r.yaml line 2: unknown field metods. There's one called methods. An operator's fields are id, explanation, tier, owners, methods, rewrite, source, changed, headline or help."),
        ] {
            assert_eq!(parse(text, "r.yaml").unwrap_err(), error);
        }
    }

    #[test]
    fn every_rewrite_rust_has_a_meaning_for_is_read() {
        let rule = |rewrite: &str| {
            let text = format!(
                "- id: a/b\n  explanation: e\n  methods: unwrap_or, checked_add\n  \
                 rewrite: {rewrite}\n  headline: h {{to}}\n  help: x\n"
            );
            parse(&text, "r.yaml").unwrap().remove(0)
        };
        assert_eq!(rule("keep argument").rewrite, Rewrite::KeepArgument);
        assert_eq!(rule("keep argument").to("unwrap_or"), None);
        let unchecked = rule("unchecked arithmetic");
        assert_eq!(unchecked.rewrite, Rewrite::Unchecked);
        assert_eq!(unchecked.methods, ["unwrap_or", "checked_add"]);
        assert_eq!(unchecked.to("checked_add").as_deref(), Some("wrapping_add"));
        assert_eq!(unchecked.to("unwrap_or"), None);
        let call = rule("call take(usize) with usize::MAX");
        assert_eq!(
            call.rewrite,
            Rewrite::Call {
                name: "take".into(),
                arguments: vec!["usize::MAX".into()]
            }
        );
        assert_eq!(call.fill(&call.headline, "take", ""), "h take(usize::MAX)");
        // The Java edition's own shapes: the JVM's types between the brackets, and none.
        assert_eq!(
            rule("call first(J) with 0, true").to("x").as_deref(),
            Some("first(0, true)")
        );
        assert_eq!(
            rule("call unwrap_or_default()").to("x").as_deref(),
            Some("unwrap_or_default()")
        );
        for argument in [
            "0",
            "-1",
            "1_000u64",
            "0.5",
            "None",
            "u64::MAX",
            "String::new()",
        ] {
            assert!(constant(argument), "{argument}");
        }
        for argument in ["", "a + b", "f(x)", "\"text\"", "a.b", "::", "1+1"] {
            assert!(!constant(argument), "{argument}");
        }
    }

    #[test]
    fn a_checked_or_saturating_method_has_a_wrapping_one() {
        assert_eq!(
            unchecked("checked_add"),
            Some(("wrapping_add".into(), true))
        );
        assert_eq!(
            unchecked("saturating_mul"),
            Some(("wrapping_mul".into(), false))
        );
        // No `wrapping_rem` of a saturating call, as there's no `saturating_rem`; and no other name.
        assert_eq!(unchecked("saturating_rem"), None);
        assert_eq!(unchecked("checked_next_power_of_two"), None);
        assert_eq!(unchecked("add"), None);
    }
}
