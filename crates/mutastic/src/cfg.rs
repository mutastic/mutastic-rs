//! Which `cfg` predicates hold in the build mutastic runs, so code this build doesn't have isn't
//! mutated: its mutants could never be reached, and would count against the score as not covered.
//!
//! The target's settings come from rustc itself (`rustc --print cfg`, for the target and flags
//! Cargo would use), the features from the package's own `[features]` table (the default ones,
//! followed through what each enables), and `debug_assertions` is on because mutastic's profile
//! turns it on. A predicate is judged three ways: built, not built, or not known. Only a predicate
//! that is surely off leaves code out: a name rustc doesn't define for every target (one a build
//! script or `RUSTFLAGS` sets, such as `docsrs` or `tokio_unstable`) is not known, and its code is
//! mutated as it always was. Wrongly leaving code out would hide a gap, where wrongly keeping it
//! only shows it as not covered.

use std::collections::BTreeSet;

use ra_ap_syntax::ast::{self, HasAttrs};
use ra_ap_syntax::{AstNode, SyntaxNode};
use serde_json::Value;

/// Names whose every value rustc prints for a target: one it doesn't print is off.
const FROM_TARGET: &[&str] = &[
    "unix",
    "windows",
    "target_os",
    "target_family",
    "target_arch",
    "target_pointer_width",
    "target_endian",
    "target_env",
    "target_vendor",
    "target_abi",
    "target_has_atomic",
    "panic",
];

/// Names that are off in every build mutastic runs: documentation, doc tests, tools, and the
/// proc-macro crates mutastic doesn't mutate.
const NEVER: &[&str] = &["doc", "doctest", "miri", "rustfmt", "clippy", "proc_macro"];

/// Whether a predicate holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Truth {
    Yes,
    No,
    Unknown,
}

/// What a build has on: the target's `cfg` values and the enabled features. Either can be not
/// known, and then every predicate on it is [`Truth::Unknown`].
#[derive(Clone, Debug, Default)]
pub struct Cfg {
    target: Option<BTreeSet<(String, Option<String>)>>,
    features: Option<BTreeSet<String>>,
}

impl Cfg {
    /// Nothing known: every piece of code counts as built.
    pub fn unknown() -> Cfg {
        Cfg::default()
    }

    /// From `rustc --print cfg`'s lines and the enabled features.
    pub fn new(printed: &str, features: BTreeSet<String>) -> Cfg {
        let target = printed
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| match line.split_once('=') {
                Some((name, value)) => (
                    name.trim().to_string(),
                    Some(value.trim().trim_matches('"').to_string()),
                ),
                None => (line.to_string(), None),
            })
            .collect::<BTreeSet<_>>();
        Cfg {
            target: (!target.is_empty()).then_some(target),
            features: Some(features),
        }
    }

    /// This machine's: asks rustc, as Cargo would run it, and takes `features` as on. When rustc
    /// can't be asked, the target is not known.
    pub fn here(features: BTreeSet<String>) -> Cfg {
        Cfg::new(&print_cfg().unwrap_or_default(), features)
    }

    /// Whether `predicate`, the inside of a `cfg(..)`, holds.
    pub fn holds(&self, predicate: &str) -> Truth {
        let predicate = predicate.trim();
        for (combinator, empty) in [("all(", Truth::Yes), ("any(", Truth::No)] {
            if let Some(args) = predicate
                .strip_prefix(combinator)
                .and_then(|rest| rest.strip_suffix(')'))
            {
                let parts = split(args);
                return parts.into_iter().map(|part| self.holds(part)).fold(
                    empty,
                    |sofar, next| match (empty, sofar, next) {
                        (Truth::Yes, Truth::No, _) | (Truth::Yes, _, Truth::No) => Truth::No,
                        (Truth::No, Truth::Yes, _) | (Truth::No, _, Truth::Yes) => Truth::Yes,
                        (_, Truth::Unknown, _) | (_, _, Truth::Unknown) => Truth::Unknown,
                        _ => empty,
                    },
                );
            }
        }
        if let Some(inner) = predicate
            .strip_prefix("not(")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            return match self.holds(inner) {
                Truth::Yes => Truth::No,
                Truth::No => Truth::Yes,
                Truth::Unknown => Truth::Unknown,
            };
        }
        let (name, value) = match predicate.split_once('=') {
            Some((name, value)) => (name.trim(), Some(value.trim().trim_matches('"'))),
            None => (predicate, None),
        };
        self.name_holds(name, value)
    }

    fn name_holds(&self, name: &str, value: Option<&str>) -> Truth {
        let known = |yes: bool| if yes { Truth::Yes } else { Truth::No };
        match name {
            "feature" => match (&self.features, value) {
                (Some(features), Some(value)) => known(features.contains(value)),
                _ => Truth::Unknown,
            },
            "debug_assertions" => Truth::Yes,
            _ if NEVER.contains(&name) => Truth::No,
            _ if FROM_TARGET.contains(&name) => match &self.target {
                Some(target) => {
                    known(target.contains(&(name.to_string(), value.map(str::to_string))))
                }
                None => Truth::Unknown,
            },
            _ => Truth::Unknown,
        }
    }

    /// The innermost `cfg(..)` on `node` or around it that is surely off, as written
    /// (`cfg(windows)`), or `None` when this build has the code, as far as can be told.
    pub fn off_at(&self, node: &SyntaxNode) -> Option<String> {
        node.ancestors().find_map(|ancestor| {
            ast::AnyHasAttrs::cast(ancestor)?
                .attrs()
                .find_map(|attr| self.off_attribute(&attr))
        })
    }

    fn off_attribute(&self, attr: &ast::Attr) -> Option<String> {
        let text = crate::sites::plain(attr.syntax());
        let inner = text
            .strip_prefix("#![")
            .or_else(|| text.strip_prefix("#["))?
            .strip_suffix(']')?
            .trim();
        let predicate = inner
            .strip_prefix("cfg")?
            .trim_start()
            .strip_prefix('(')?
            .strip_suffix(')')?;
        (self.holds(predicate) == Truth::No).then(|| inner.to_string())
    }
}

/// `a, f(b, c), d` split at its top-level commas, without an empty last part.
fn split(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut start, mut quoted) = (0i32, 0, false);
    for (at, c) in text.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth -= 1,
            ',' if depth == 0 && !quoted => {
                parts.push(&text[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts.retain(|part| !part.trim().is_empty());
    parts
}

/// `rustc --print cfg` for the target Cargo builds for, with the flags it passes.
fn print_cfg() -> Option<String> {
    let mut command =
        std::process::Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()));
    command.args(["--print", "cfg"]);
    if let Ok(target) = std::env::var("CARGO_BUILD_TARGET") {
        command.args(["--target", &target]);
    }
    if let Ok(encoded) = std::env::var("CARGO_ENCODED_RUSTFLAGS") {
        command.args(encoded.split('\x1f').filter(|flag| !flag.is_empty()));
    } else if let Ok(flags) = std::env::var("RUSTFLAGS") {
        command.args(flags.split_whitespace());
    }
    let output = command.output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The features a plain build of a package turns on: `default`, and everything it enables, from
/// the package's `features` in `cargo metadata`.
pub fn default_features(table: &Value) -> BTreeSet<String> {
    let mut on = BTreeSet::new();
    let mut queue = vec!["default".to_string()];
    while let Some(feature) = queue.pop() {
        let Some(enables) = table.get(&feature).and_then(Value::as_array) else {
            continue;
        };
        if !on.insert(feature) {
            continue;
        }
        for enabled in enables.iter().filter_map(Value::as_str) {
            // `dep:x` turns on a dependency, not a feature; `x/f` turns on `x` too, `x?/f` doesn't.
            if enabled.starts_with("dep:") || enabled.contains("?/") {
                continue;
            }
            let name = enabled.split('/').next().unwrap_or(enabled);
            queue.push(name.to_string());
        }
    }
    on
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAC: &str = "debug_assertions\npanic=\"unwind\"\ntarget_arch=\"aarch64\"\n\
        target_family=\"unix\"\ntarget_os=\"macos\"\ntarget_pointer_width=\"64\"\nunix\n";

    fn mac(features: &[&str]) -> Cfg {
        Cfg::new(MAC, features.iter().map(|f| f.to_string()).collect())
    }

    #[test]
    fn a_predicate_is_built_not_built_or_not_known() {
        let cfg = mac(&["std"]);
        for (predicate, truth) in [
            ("unix", Truth::Yes),
            ("windows", Truth::No),
            ("target_os = \"macos\"", Truth::Yes),
            ("target_os=\"linux\"", Truth::No),
            ("feature = \"std\"", Truth::Yes),
            ("feature = \"simd\"", Truth::No),
            ("debug_assertions", Truth::Yes),
            ("doc", Truth::No),
            ("docsrs", Truth::Unknown),
            ("test", Truth::Unknown),
            ("not(windows)", Truth::Yes),
            ("not(docsrs)", Truth::Unknown),
            ("all(unix, feature = \"simd\")", Truth::No),
            ("all(unix, docsrs)", Truth::Unknown),
            ("all(windows, docsrs)", Truth::No),
            ("any(windows, docsrs)", Truth::Unknown),
            ("any(unix, docsrs)", Truth::Yes),
            ("any(windows, target_os = \"linux\")", Truth::No),
            ("all()", Truth::Yes),
            ("any()", Truth::No),
            ("all(unix,)", Truth::Yes),
        ] {
            assert_eq!(cfg.holds(predicate), truth, "{predicate}");
        }
    }

    #[test]
    fn nothing_is_left_out_when_nothing_is_known() {
        let cfg = Cfg::unknown();
        for predicate in ["windows", "feature = \"simd\"", "not(unix)"] {
            assert_eq!(cfg.holds(predicate), Truth::Unknown, "{predicate}");
        }
        let no_target = Cfg::new("", BTreeSet::new());
        assert_eq!(no_target.holds("windows"), Truth::Unknown);
        assert_eq!(no_target.holds("feature = \"simd\""), Truth::No);
    }

    #[test]
    fn this_machine_is_asked() {
        let here = Cfg::here(BTreeSet::new());
        assert_eq!(here.holds("all(unix, windows)"), Truth::No);
        assert_eq!(here.holds("any(unix, windows)"), Truth::Yes);
    }

    #[test]
    fn default_features_follow_what_they_enable() {
        let table = serde_json::json!({
            "default": ["std", "dep:log", "serde?/std"],
            "std": ["alloc", "regex/unicode"],
            "alloc": [],
            "regex": ["dep:regex"],
            "simd": [],
            "serde": ["dep:serde"],
        });
        let on: Vec<String> = default_features(&table).into_iter().collect();
        assert_eq!(on, ["alloc", "default", "regex", "std"]);
        assert!(default_features(&serde_json::json!({ "simd": [] })).is_empty());
    }
}
