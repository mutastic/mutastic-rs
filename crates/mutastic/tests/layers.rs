//! mutastic's modules keep to their layers. All of mutastic is one crate, so nothing in Cargo stops
//! a module reaching up: the engine into the report, the project's model into the engine. The
//! layers are the table in ARCHITECTURE.md's "Layers", top first; each module may use the modules
//! in its own layer and those below it.
//!
//! This reads every `crate::` path in each module's files (`src/<module>.rs` and `src/<module>/`)
//! with rust-analyzer's parser, comments left out, macros' tokens in, and fails on each one that
//! names a module in a higher layer, with its file and line. It fails too on a module `lib.rs`
//! declares that the table doesn't place, and on one the table names that isn't there.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ra_ap_syntax::ast::{HasName, Module};
use ra_ap_syntax::{AstNode, Edition, SourceFile, SyntaxKind, SyntaxToken, T};

/// The heading the layers' table is under.
const HEADING: &str = "## Layers";

#[test]
fn each_module_uses_only_its_own_layer_and_those_below() {
    let layers = layers();
    let layer_of: BTreeMap<&str, usize> = layers
        .iter()
        .enumerate()
        .flat_map(|(depth, (_, modules))| {
            modules.iter().map(move |module| (module.as_str(), depth))
        })
        .collect();
    let declared = declared();
    let placed: BTreeSet<&str> = layer_of.keys().copied().collect();
    let declared_names: BTreeSet<&str> = declared.iter().map(String::as_str).collect();
    let unplaced: Vec<&&str> = declared_names.difference(&placed).collect();
    let unknown: Vec<&&str> = placed.difference(&declared_names).collect();
    assert!(
        unplaced.is_empty() && unknown.is_empty(),
        "ARCHITECTURE.md's layers place every module lib.rs declares, and only those. Not placed \
         (put each in the lowest layer that has everything it uses): {unplaced:?}. Not modules: \
         {unknown:?}"
    );
    let mut upward = Vec::new();
    for module in &declared {
        let own = layer_of[module.as_str()];
        for file in files(module) {
            let text = std::fs::read_to_string(&file).expect("a module's file reads");
            for (used, line) in crate_paths(&text) {
                let Some(&theirs) = layer_of.get(used.as_str()) else {
                    continue;
                };
                if theirs < own {
                    let shown = file.strip_prefix(src()).unwrap_or(&file).display();
                    upward.push(format!(
                        "src/{shown}:{line}: `{module}` ({}) uses `{used}` ({})",
                        layers[own].0, layers[theirs].0
                    ));
                }
            }
        }
    }
    assert!(
        upward.is_empty(),
        "a module uses one in a layer above its own: move what it uses down, or the module up, \
         as ARCHITECTURE.md's layers say\n{}",
        upward.join("\n")
    );
}

#[test]
fn a_path_from_the_crate_names_its_first_module() {
    let text = "use crate::engine::{self, Status};\nuse crate::{report, sites::{Crate, Mutant}};\n\
                // crate::comment\nfn f() { format!(\"{}\", crate::words::INPUTS_SHOWN); }\n\
                pub(crate) fn g() {}\n";
    let expected = [("engine", 1), ("report", 2), ("sites", 2), ("words", 4)];
    assert_eq!(
        crate_paths(text),
        expected.map(|(module, line)| (module.to_string(), line))
    );
}

/// The layers, top first, each its name and its modules, from ARCHITECTURE.md's table.
fn layers() -> Vec<(String, Vec<String>)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ARCHITECTURE.md");
    let text = std::fs::read_to_string(path).expect("ARCHITECTURE.md reads");
    let (_, section) = text
        .split_once(&format!("\n{HEADING}\n"))
        .expect("ARCHITECTURE.md has the layers");
    let rows: Vec<(String, Vec<String>)> = section
        .lines()
        .skip_while(|line| !line.starts_with('|'))
        .take_while(|line| line.starts_with('|'))
        .skip(2)
        .map(|row| {
            let cells: Vec<&str> = row.split('|').map(str::trim).collect();
            let modules = cells[2]
                .split(',')
                .map(|module| module.trim().trim_matches('`').to_string())
                .collect();
            (cells[1].to_string(), modules)
        })
        .collect();
    assert!(rows.len() > 1, "the layers' table has its rows");
    rows
}

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// The modules `lib.rs` declares, `pub mod engine;`.
fn declared() -> Vec<String> {
    let text = std::fs::read_to_string(src().join("lib.rs")).expect("lib.rs reads");
    SourceFile::parse(&text, Edition::CURRENT)
        .tree()
        .syntax()
        .children()
        .filter_map(Module::cast)
        .filter_map(|module| module.name())
        .map(|name| name.text().to_string())
        .collect()
}

/// A module's files: `src/<module>.rs`, and every file under `src/<module>/`.
fn files(module: &str) -> Vec<PathBuf> {
    let mut found = vec![src().join(format!("{module}.rs"))];
    let mut dirs = vec![src().join(module)];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for path in entries.map(|entry| entry.expect("a directory lists").path()) {
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
    }
    found.retain(|file| file.is_file());
    found.sort();
    found
}

/// The paths in `text` that start `crate::`, each as the module it names first and the line it's
/// on: for `use crate::{a, b::c}`, `a` and `b`. Inside a macro's call the parser leaves `::` as two
/// `:`, and the paths there count too.
fn crate_paths(text: &str) -> Vec<(String, usize)> {
    let tokens: Vec<SyntaxToken> = SourceFile::parse(text, Edition::CURRENT)
        .tree()
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .collect();
    let line = |token: &SyntaxToken| {
        let offset = usize::from(token.text_range().start());
        text[..offset].matches('\n').count() + 1
    };
    let mut named = Vec::new();
    for (at, token) in tokens.iter().enumerate() {
        if token.text() != "crate" {
            continue;
        }
        let after: Vec<SyntaxKind> = tokens[at + 1..]
            .iter()
            .take(2)
            .map(SyntaxToken::kind)
            .collect();
        let next = match after.as_slice() {
            [T![::], ..] => at + 2,
            [T![:], T![:]] => at + 3,
            _ => continue,
        };
        let firsts = match tokens.get(next) {
            Some(first) if first.kind() == SyntaxKind::IDENT => vec![first],
            Some(open) if open.kind() == T!['{'] => group_heads(&tokens[next..]),
            _ => vec![],
        };
        named.extend(
            firsts
                .into_iter()
                .map(|first| (first.text().to_string(), line(first))),
        );
    }
    named
}

/// The first name of each path in a use group, `{a, b::{c, d}}`: `a` and `b`.
fn group_heads(tokens: &[SyntaxToken]) -> Vec<&SyntaxToken> {
    let mut heads = Vec::new();
    let mut depth = 0;
    let mut starts_path = false;
    for token in tokens {
        match token.kind() {
            T!['{'] => depth += 1,
            T!['}'] if depth == 1 => break,
            T!['}'] => depth -= 1,
            _ => {}
        }
        if depth == 1 && starts_path && token.kind() == SyntaxKind::IDENT {
            heads.push(token);
        }
        starts_path = depth == 1 && matches!(token.kind(), T!['{'] | T![,]);
    }
    heads
}
