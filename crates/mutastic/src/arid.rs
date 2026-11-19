//! mutastic's own arid rules: each a kind of code a test could check but shouldn't have to, by the
//! id output, `mutastic.yaml` and the docs name it by. The ids are the Java edition's, shared in
//! the editions' contract (`arid`): this edition has the rules it makes and skips a mutant for. An
//! id never changes once shipped, and docs.mutastic.io has a section for each, one page for both
//! editions.

/// What a rule judges: a whole method, or one site in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Method,
    Site,
}

impl Scope {
    /// `method`, `site`: as the contract says it.
    pub fn written(self) -> &'static str {
        match self {
            Scope::Method => "method",
            Scope::Site => "site",
        }
    }
}

/// One rule: its id, what it judges, the reasons it gives as a skipped mutant shows them, and a
/// line of Rust it skips a mutant of.
#[derive(Debug)]
pub struct Rule {
    pub id: &'static str,
    pub scope: Scope,
    pub reasons: &'static [&'static str],
    pub example: &'static str,
}

/// The arid skip of an output macro's mutant, in the Java edition's words.
pub const ONLY_WRITES_OUTPUT: &str = "arid: only writes output";

/// The arid skip of a statement that only checks a value it's given, `x.unwrap();`, as the Java
/// edition skips a removed `Objects.requireNonNull(x)`.
pub const ONLY_CHECKS_A_VALUE: &str = "arid: only checks the value is there";

/// The rules this edition has.
pub const RULES: &[Rule] = &[
    Rule {
        id: "mutastic:output",
        scope: Scope::Site,
        reasons: &[ONLY_WRITES_OUTPUT],
        example: "println!(\"saved {id}\");",
    },
    Rule {
        id: "mutastic:null-checks",
        scope: Scope::Site,
        reasons: &[ONLY_CHECKS_A_VALUE],
        example: "order.unwrap();",
    },
];

/// The page with a section for each rule, shared by both editions.
pub const PAGE: &str = "https://docs.mutastic.io/reference/arid/";

impl Rule {
    /// `output`: the id after its namespace, its section's anchor.
    pub fn anchor(&self) -> &'static str {
        self.id.split_once(':').map_or(self.id, |(_, name)| name)
    }

    /// `https://docs.mutastic.io/reference/arid/#output`.
    pub fn url(&self) -> String {
        format!("{PAGE}#{}", self.anchor())
    }
}

/// The rule that gives `reason` (`arid: only writes output`), if one of mutastic's own does.
pub fn rule(reason: &str) -> Option<&'static Rule> {
    RULES.iter().find(|rule| rule.reasons.contains(&reason))
}

/// The rule with id `id`, `mutastic:output`.
pub fn named(id: &str) -> Option<&'static Rule> {
    RULES.iter().find(|rule| rule.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sites::{find, ALL};
    use std::path::PathBuf;

    /// Each rule's example, as the docs show it, is skipped by that rule.
    #[test]
    fn each_rules_example_is_skipped_by_it() {
        for rule in RULES {
            let source = format!(
                "pub fn example(id: u32, order: Option<u32>) {{\n    {}\n}}\n",
                rule.example
            );
            let dir = std::env::temp_dir().join(format!(
                "mutastic-arid-example-{}-{}",
                rule.anchor(),
                std::process::id()
            ));
            std::fs::create_dir_all(dir.join("src")).unwrap();
            std::fs::write(dir.join("src/lib.rs"), &source).unwrap();
            let found = find(&dir, "sample", &[PathBuf::from("src/lib.rs")], ALL);
            std::fs::remove_dir_all(&dir).ok();
            assert!(
                found
                    .mutants
                    .iter()
                    .filter(|mutant| mutant.line == 2)
                    .any(|mutant| mutant
                        .skipped
                        .as_deref()
                        .and_then(super::rule)
                        .map(|r| r.id)
                        == Some(rule.id)),
                "{}: {}",
                rule.id,
                rule.example
            );
        }
    }

    #[test]
    fn a_rule_is_found_by_its_reason_and_its_id() {
        assert_eq!(
            rule("arid: only writes output").map(|r| r.id),
            Some("mutastic:output")
        );
        assert!(rule("arid: only waits a bit").is_none());
        assert_eq!(
            named("mutastic:output").map(Rule::url).as_deref(),
            Some("https://docs.mutastic.io/reference/arid/#output")
        );
        assert!(named("output").is_none());
    }
}
