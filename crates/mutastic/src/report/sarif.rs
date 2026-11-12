//! The run as SARIF 2.1.0, as the Java edition writes it, which code scanning reads: GitHub's
//! alerts on a pull request's lines, Azure DevOps, and SARIF viewers in an editor.
//!
//! One rule per operator that made a result, and one result per missed or not-covered mutant: a
//! missed one a warning, as a test runs the code and doesn't check it, a not-covered one a note.
//! Each result's `partialFingerprints` carries the mutant's id, which stays the same when code
//! above it moves, so an alert follows its mutant. A result never has a `fixes` entry: the fix is
//! a test, never a change to the code.
//!
//! Paths are relative to the git repository the workspace is in, as code scanning resolves them
//! against the repository's root, or to the workspace where there's no repository.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::{Status, View};
use crate::engine::{MutantResult, Report};

pub const SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";

/// The key the mutant's id goes under in `partialFingerprints`.
pub const FINGERPRINT: &str = "mutasticId/v1";

/// Where each operator is described: the table of every operator, at its row.
const OPERATORS: &str =
    "https://github.com/mutastic/mutastic-rs/blob/main/docs/operators.md#every-operator";

pub fn document(report: &Report) -> Value {
    let view = View { report };
    let flagged: Vec<&MutantResult> = report
        .results
        .iter()
        .filter(|result| matches!(result.status, Status::Survived | Status::NoCoverage))
        .collect();
    let rules: BTreeMap<&str, Value> = flagged
        .iter()
        .map(|result| {
            let operator = view.mutant(result).operator;
            (operator, rule(operator))
        })
        .collect();
    let rule_ids: Vec<&str> = rules.keys().copied().collect();
    let base = repository(&report.project.workspace);
    let results: Vec<Value> = flagged
        .iter()
        .map(|result| {
            let operator = view.mutant(result).operator;
            let index = rule_ids
                .iter()
                .position(|id| *id == operator)
                .unwrap_or_default();
            result_entry(&view, result, index, base.as_deref())
        })
        .collect();
    let root = base.unwrap_or_else(|| report.project.workspace.clone());
    json!({
        "$schema": SCHEMA,
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": "mutastic",
                "version": env!("CARGO_PKG_VERSION"),
                "informationUri": "https://mutastic.io",
                "rules": rules.into_values().collect::<Vec<_>>(),
            }},
            "originalUriBaseIds": { "%SRCROOT%": { "uri": directory_uri(&root) } },
            "results": results,
        }],
    })
}

/// An operator as a rule: a rules file's own explanation, or where mutastic's own are described.
fn rule(operator: &str) -> Value {
    let explained = crate::callrules::find(operator).map(|rule| rule.explanation.clone());
    let short = format!("A mutant of {operator} that no test catches");
    let help = explained.clone().unwrap_or_else(|| {
        format!("What {operator} changes, and the bug it stands for: {OPERATORS}")
    });
    json!({
        "id": operator,
        "shortDescription": { "text": short },
        "fullDescription": { "text": explained.unwrap_or_else(|| short.clone()) },
        "helpUri": OPERATORS,
        "help": { "text": help },
        "defaultConfiguration": { "level": "warning" },
        "properties": { "tags": ["mutation-testing", "tests"] },
    })
}

fn result_entry(
    view: &View,
    result: &MutantResult,
    rule_index: usize,
    base: Option<&Path>,
) -> Value {
    let mutant = view.mutant(result);
    let advice = view.advice(result);
    let text = view.text(mutant);
    let (range, _) = mutant.claim();
    let start = super::files::position(text, range.start);
    let end = super::files::position(text, range.end);
    let mut region = json!({ "startLine": start["line"], "startColumn": start["column"] });
    if end["line"] == start["line"] {
        region["endColumn"] = end["column"].clone();
    } else {
        region["endLine"] = end["line"].clone();
        region["endColumn"] = end["column"].clone();
    }
    let uri = from_base(&view.report.project.workspace, &mutant.file, base);
    json!({
        "ruleId": mutant.operator,
        "ruleIndex": rule_index,
        "level": if result.status == Status::Survived { "warning" } else { "note" },
        // The card's headline: plain as text, with its code marked as Markdown, which viewers
        // render.
        "message": { "text": crate::words::plain(&advice.headline), "markdown": advice.headline },
        "locations": [{ "physicalLocation": {
            "artifactLocation": { "uri": uri, "uriBaseId": "%SRCROOT%" },
            "region": region,
        }}],
        "partialFingerprints": { FINGERPRINT: mutant.id },
        "properties": {
            "id": mutant.id,
            "status": result.status.schema_name(),
            "change": advice.change,
        },
    })
}

/// `src/lib.rs`, relative to the workspace, as it's relative to the base: the repository's root,
/// where the workspace may be a folder down.
fn from_base(workspace: &Path, file: &Path, base: Option<&Path>) -> String {
    let whole = workspace.join(file);
    let relative = base
        .and_then(|base| whole.strip_prefix(base).ok())
        .unwrap_or(file);
    crate::workspace::slashed(relative).replace('\\', "/")
}

/// The git repository the workspace is in: the nearest directory, from the workspace up, with a
/// `.git` in it.
fn repository(workspace: &Path) -> Option<PathBuf> {
    workspace
        .ancestors()
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf)
}

/// `file:///code/shop/`: a directory as a URI, ending in `/`, as a base must.
fn directory_uri(dir: &Path) -> String {
    let path = crate::workspace::slashed(dir).replace('\\', "/");
    let path = if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    };
    if path.ends_with('/') {
        format!("file://{path}")
    } else {
        format!("file://{path}/")
    }
}
