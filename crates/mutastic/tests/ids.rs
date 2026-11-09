//! Mutant ids are a promise: a `verify` in a script, a baseline or an ignore names a mutant by its
//! id, so an id must name the same mutant in every later version while its function's code is
//! unchanged. This finds every mutant of every fixture, under every operator mutastic has, and holds
//! them to the list kept in `tests/ids/ids.tsv`. Only finding: nothing is built or run.
//!
//! An id that's gone, or that now names another mutant, fails it: that breaks what users have kept.
//! So does a new mutant of an operator already listed, until the list is written again, so the list
//! keeps holding every mutant. Only an operator that's new adds ids freely.
//!
//! To write the list: `MUTASTIC_IDS_WRITE=1 cargo test --test ids`, then read the diff: lines may
//! be added, never changed or removed.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mutastic::{engine, sites, workspace};

const FIXTURES: &[&str] = &[
    "loops",
    "operators",
    "pricing",
    "repeats",
    "suggestions",
    "truth",
    "unsafety",
];

fn kept_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ids/ids.tsv")
}

/// Every fixture's mutants, by id: fixture, function, operator, and the code before and after. Not
/// the line, which moves when a fixture grows, nor the card's words, while the id mustn't.
fn found() -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    for fixture in FIXTURES {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(fixture);
        let project = workspace::read(&dir.display().to_string()).unwrap();
        let crate_found = engine::find_all(&project);
        for mutant in &crate_found.mutants {
            let function = &crate_found.functions[mutant.function];
            let file = crate_found
                .files
                .iter()
                .find(|f| f.path == mutant.file)
                .unwrap();
            let original = &file.text[mutant.site.clone()];
            found.insert(
                mutant.id.clone(),
                format!(
                    "{fixture}	{}	{}	{} → {}",
                    function.path,
                    mutant.operator,
                    one_line(original),
                    one_line(&mutant.replacement)
                ),
            );
        }
    }
    found
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn read(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| line.split_once('\t'))
        .map(|(id, rest)| (id.to_string(), rest.to_string()))
        .collect()
}

fn operator_of(description: &str) -> &str {
    description.split('\t').nth(2).unwrap_or_default()
}

#[test]
fn every_kept_id_still_names_its_mutant() {
    let found = found();
    if std::env::var_os("MUTASTIC_IDS_WRITE").is_some() {
        let text: String = found
            .iter()
            .map(|(id, description)| format!("{id}\t{description}\n"))
            .collect();
        std::fs::write(kept_path(), text).unwrap();
    }
    let kept = read(&std::fs::read_to_string(kept_path()).unwrap());
    let moved: Vec<String> = kept
        .iter()
        .filter(|(id, description)| found.get(*id) != Some(description))
        .map(|(id, description)| format!("{id}\t{description} → {:?}", found.get(id)))
        .collect();
    assert!(
        moved.is_empty(),
        "ids that are gone or name another mutant (users' scripts and baselines name these):\n{}",
        moved.join("\n")
    );
    let operators: BTreeSet<&str> = kept.values().map(|d| operator_of(d)).collect();
    let added: Vec<String> = found
        .iter()
        .filter(|(id, _)| !kept.contains_key(*id))
        .filter(|(_, description)| operators.contains(operator_of(description)))
        .map(|(id, description)| format!("{id}\t{description}"))
        .collect();
    assert!(
        added.is_empty(),
        "new mutants of operators already kept: MUTASTIC_IDS_WRITE=1 cargo test --test ids\n{}",
        added.join("\n")
    );
    assert!(kept.len() > 50, "{} ids kept", kept.len());
    assert!(sites::ALL.len() >= sites::STRONGER.len());
}
