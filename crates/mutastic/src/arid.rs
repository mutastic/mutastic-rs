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
    /// What it skips, in a sentence, in the words of the contract.
    pub description: &'static str,
    pub reasons: &'static [&'static str],
    pub example: &'static str,
}

/// The arid skip of an output macro's mutant, in the Java edition's words.
pub const ONLY_WRITES_OUTPUT: &str = "arid: only writes output";

/// The arid skip of a statement that only checks a value it's given, `x.unwrap();`, as the Java
/// edition skips a removed `Objects.requireNonNull(x)`.
pub const ONLY_CHECKS_A_VALUE: &str = "arid: only checks the value is there";

/// The arid skip of a thread's name, as the Java edition words it.
pub const NAMES: &str = "arid: only names a thread";

/// The arid skip of a thread's stack size.
pub const STACK: &str = "arid: only sizes a thread's stack";

/// The arid skip of how long a pool's idle threads live.
pub const KEEP_ALIVE: &str = "arid: only sets how long idle threads live";

/// The rules this edition has.
pub const RULES: &[Rule] = &[
    Rule {
        id: "mutastic:output",
        scope: Scope::Site,
        description: "Code that only writes output: a log line, or text printed to the console.",
        reasons: &[ONLY_WRITES_OUTPUT],
        example: "println!(\"saved {id}\");",
    },
    Rule {
        id: "mutastic:null-checks",
        scope: Scope::Site,
        description: "A call that only checks a value is there, its result unused, removed: `Objects.requireNonNull(x)` or `x.getClass()` in Java, `x.unwrap()` or `x.expect(…)` in Rust.",
        reasons: &[ONLY_CHECKS_A_VALUE],
        example: "order.unwrap();",
    },
    Rule {
        id: "mutastic:thread-names",
        scope: Scope::Site,
        description: "A thread's or a pool's name, which only people and their tools read: a thread dump, a profiler, a log pattern.",
        reasons: &[NAMES],
        example: "let builder = thread::Builder::new().name(\"squarer\".into());",
    },
    Rule {
        id: "mutastic:keep-alive",
        scope: Scope::Site,
        description: "How long a pool's idle threads live before they stop, while it stays above zero.",
        reasons: &[KEEP_ALIVE],
        example: "let runtime = Builder::new_multi_thread().thread_keep_alive(Duration::from_secs(5 * 60));",
    },
    Rule {
        id: "mutastic:stack-sizes",
        scope: Scope::Site,
        description: "The size of a thread's stack, while it stays above zero.",
        reasons: &[STACK],
        example: "let builder = thread::Builder::new().stack_size(64 * 1024);",
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

    /// The `mutastic.yaml` text that runs this rule's mutants in the function at `path`
    /// (`Money::add`), as the Java edition writes it: a `rules:` list of one `mutate:` rule, with
    /// a reason to fill in.
    pub fn bring_back(&self, path: &str) -> String {
        format!(
            "rules:\n  - mutate: {path}\n    arid: [{}]\n    reason: <why a test should check it>\n",
            self.id
        )
    }
}

/// The rule that gives `reason` (`arid: only writes output`), if one of mutastic's own does.
pub fn rule(reason: &str) -> Option<&'static Rule> {
    RULES.iter().find(|rule| rule.reasons.contains(&reason))
}

/// Every rule's id, in order.
pub fn ids() -> Vec<&'static str> {
    RULES.iter().map(|rule| rule.id).collect()
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
