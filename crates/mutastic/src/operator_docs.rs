//! Where docs.mutastic.io describes each of mutastic's own operators: its section on its family's
//! page in the Rust docs, `https://docs.mutastic.io/rust/operators/conditions/#boundary`, with what
//! it changes, the bug it stands for, a card and the test that catches it. `explain` gives it, a
//! card's change mark links to it when links are on, and a mutant's event carries it as `docs`. A
//! rules file's operator has no section there, so no link.
//!
//! Each section's heading is its operator's name, `## Swap index of` for `library/swap-index-of`,
//! so its anchor is the id after its namespace; only the family is said here.

use crate::commands::DOCS;
use crate::sites::{
    ALWAYS_DEFAULT, ARITHMETIC_REPLACEMENT, BITWISE, BOOLEAN_CONSTANT, BOUNDARY, COMPARISON,
    CONSTANT_VALUE, DEFAULT_PARAMETER, DROP_VARARG, KEEP_OPERAND, LOGICAL_CONNECTOR, MATH,
    NO_DEFAULT, NULL_PARAMETER, OMIT_CALL, OPTIONAL_EMPTY, OTHER_ARITHMETIC, OTHER_COMPARISON,
    PLAIN_DIVISION, REGEX_CONSTANT, REMOVE_ASSIGNMENT, REMOVE_CONDITIONAL, REMOVE_DISCARDED_CALL,
    REMOVE_FIELD_WRITE, REMOVE_FILTER, REMOVE_NEGATION, REMOVE_OTHER_STAGE, REMOVE_STAGE,
    REMOVE_STEP, REMOVE_STRUCT_FIELD, REMOVE_SWITCH_ARM, REMOVE_THROW, REMOVE_VOID_CALL,
    REPLACE_BODY, RETURN_VALUE, REVERSE_ORDERING, STRING_CONSTANT, SWAP_ARGUMENTS, SWAP_BREAK,
    SWAP_ENDS, SWAP_INDEX_OF, SWAP_MATCH, SWAP_MATH, SWAP_PRESENT, SWAP_STRING,
    UNCHECKED_ARITHMETIC, UNWRAP_TRY,
};

/// The page under `operators/` that has `operator`'s section, for each of mutastic's own.
fn family(operator: &str) -> Option<&'static str> {
    Some(match operator {
        BOUNDARY | COMPARISON | OTHER_COMPARISON | REMOVE_CONDITIONAL | LOGICAL_CONNECTOR => {
            "conditions"
        }
        MATH
        | ARITHMETIC_REPLACEMENT
        | OTHER_ARITHMETIC
        | KEEP_OPERAND
        | REMOVE_NEGATION
        | CONSTANT_VALUE
        | BITWISE
        | UNCHECKED_ARITHMETIC
        | PLAIN_DIVISION => "arithmetic",
        RETURN_VALUE | UNWRAP_TRY | REPLACE_BODY | REMOVE_THROW => "returns",
        REMOVE_VOID_CALL
        | REMOVE_DISCARDED_CALL
        | REMOVE_ASSIGNMENT
        | REMOVE_FIELD_WRITE
        | REMOVE_STRUCT_FIELD => "calls",
        OMIT_CALL | SWAP_ARGUMENTS | NULL_PARAMETER | DROP_VARARG | DEFAULT_PARAMETER
        | BOOLEAN_CONSTANT => "arguments",
        SWAP_STRING | SWAP_MATH | SWAP_INDEX_OF | REVERSE_ORDERING | SWAP_ENDS
        | STRING_CONSTANT | REGEX_CONSTANT | REMOVE_STEP => "library",
        SWAP_MATCH | REMOVE_STAGE | REMOVE_OTHER_STAGE => "iterators",
        SWAP_PRESENT | OPTIONAL_EMPTY | ALWAYS_DEFAULT | NO_DEFAULT | REMOVE_FILTER => "options",
        REMOVE_SWITCH_ARM | SWAP_BREAK => "control",
        _ => return None,
    })
}

/// `https://docs.mutastic.io/rust/operators/conditions/#boundary`; `None` for a rules file's
/// operator.
pub fn url(operator: &str) -> Option<String> {
    let family = family(operator)?;
    let name = operator.split_once('/').map_or(operator, |(_, name)| name);
    Some(format!("{DOCS}operators/{family}/#{name}"))
}

/// Each family's page, as the binary was built with it.
fn page(family: &str) -> Option<&'static str> {
    Some(match family {
        "conditions" => include_str!("../../../docs/operators/conditions.md"),
        "arithmetic" => include_str!("../../../docs/operators/arithmetic.md"),
        "returns" => include_str!("../../../docs/operators/returns.md"),
        "calls" => include_str!("../../../docs/operators/calls.md"),
        "arguments" => include_str!("../../../docs/operators/arguments.md"),
        "library" => include_str!("../../../docs/operators/library.md"),
        "iterators" => include_str!("../../../docs/operators/iterators.md"),
        "options" => include_str!("../../../docs/operators/options.md"),
        "control" => include_str!("../../../docs/operators/control.md"),
        _ => return None,
    })
}

/// What one of mutastic's own operators changes and when a test misses it, as its section in the
/// docs opens, after the sentence naming it and its set: "`<` becomes `<=`, `>=` becomes `>`, and
/// the other way round. … never notice." `explain`, SARIF's rules and the docs say the same.
/// `None` for a rules file's operator, whose explanation is its own.
pub fn explanation(operator: &str) -> Option<String> {
    let text = page(family(operator)?)?;
    let opening = format!("`{operator}`, ");
    let mut lines = text.lines().skip_while(|line| !line.starts_with(&opening));
    let paragraph: Vec<&str> = lines
        .by_ref()
        .take_while(|line| !line.trim().is_empty())
        .map(str::trim)
        .collect();
    let paragraph = paragraph.join(" ");
    let (_, rest) = paragraph.split_once(". ")?;
    // A paragraph that opens a table of what becomes what ends as a sentence here.
    Some(match rest.strip_suffix(':') {
        Some(before) => format!("{before}."),
        None => rest.to_string(),
    })
}

/// The first sentence of an explanation, for SARIF's `shortDescription`, as the Java edition cuts
/// it.
pub fn first_sentence(text: &str) -> &str {
    text.find(". ").map_or(text, |end| &text[..=end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sites::RETURN_VALUE_EXTENDED;

    /// Every operator with a section has an explanation from it, a sentence or more that says
    /// what it changes.
    #[test]
    fn each_operator_is_explained_as_its_section_opens() {
        assert_eq!(
            explanation(BOUNDARY).as_deref().map(first_sentence),
            Some("`<` becomes `<=`, `>=` becomes `>`, and the other way round.")
        );
        for operator in crate::sites::ALL {
            if url(operator).is_none() {
                continue;
            }
            let explained = explanation(operator).unwrap_or_default();
            assert!(
                explained.ends_with('.') && explained.len() > 8,
                "{operator}: {explained:?}"
            );
        }
        assert_eq!(explanation("acme/remove-trim"), None);
    }

    #[test]
    fn an_operator_links_to_its_section_and_a_rules_files_to_none() {
        assert_eq!(
            url(BOUNDARY).as_deref(),
            Some("https://docs.mutastic.io/rust/operators/conditions/#boundary")
        );
        assert_eq!(
            url(SWAP_INDEX_OF).as_deref(),
            Some("https://docs.mutastic.io/rust/operators/library/#swap-index-of")
        );
        assert_eq!(url("acme/remove-trim"), None);
        assert_eq!(url(RETURN_VALUE_EXTENDED), None);
    }
}
