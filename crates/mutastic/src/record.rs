//! What a run found, kept for the next run to compare with, as `mutastic-core`'s
//! [`record`](mutastic_core::record) keeps it for every edition: here, a Rust package's run, and
//! the hash of the mutastic that ran it.

pub use mutastic_core::record::*;

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::engine::Report;
use crate::workspace::Project;

/// The record of `report`, run on code whose hash is `code` by a mutastic whose hash is `tool`.
pub fn of(report: &Report, code: &str, tool: &str) -> RunRecord {
    let results = report
        .results
        .iter()
        .map(|result| {
            let entry = Entry {
                status: result.status,
                caught_by: result.killed_by.clone(),
            };
            (report.found.mutants[result.mutant].id.clone(), entry)
        })
        .collect();
    RunRecord::new(
        project_of(&report.project),
        &report.operators,
        code,
        tool,
        report.score(),
        results,
    )
}

/// What a run of `project` under `operators` will be compared by, before it runs: its code and
/// mutastic, with no results yet. What an earlier run found is then known before this one's
/// mutants run.
pub fn before(project: &Project, operators: &[&str], code: &str, tool: &str) -> RunRecord {
    RunRecord::new(
        project_of(project),
        operators,
        code,
        tool,
        None,
        BTreeMap::new(),
    )
}

/// `/code/shop`: which package a record is of, by its directory.
fn project_of(project: &Project) -> String {
    // Rebuilt from its parts, so a package at the workspace's root has no `/` at the end.
    project
        .workspace
        .join(&project.package_dir)
        .components()
        .collect::<std::path::PathBuf>()
        .display()
        .to_string()
}

/// The hash of the mutastic that's running, so a rebuilt mutastic counts as a different one even
/// at the same version; its version, where its file can't be read.
pub fn tool() -> String {
    let bytes = std::env::current_exe().and_then(std::fs::read);
    match bytes {
        Ok(bytes) => Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        Err(_) => env!("CARGO_PKG_VERSION").to_string(),
    }
}
