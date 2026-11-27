//! mutastic's modules keep to their layers. All of mutastic is one crate, so nothing in Cargo stops
//! a module reaching up: the engine into the report, the project's model into the engine. The
//! layers are the table in ARCHITECTURE.md's "Layers", top first; each module may use the modules
//! in its own layer and those below it.
//!
//! This reads every `crate::` path in each module's files (`src/<module>.rs` and `src/<module>/`)
//! with rust-analyzer's parser, comments left out, macros' tokens in, and fails on each one that
//! names a module in a higher layer, with its file and line. It fails too on a module `lib.rs`
//! declares that the table doesn't place, and on one the table names that isn't there.
//!
//! The output kit, `crates/mutastic-output`, is a crate of its own so that it can't reach up at
//! all: `lib.rs` re-exports its modules (`pub use mutastic_output::{…}`), which count as declared,
//! and the table places them in the lowest layer, `kit`. This holds the kit to knowing nothing of
//! mutastic's: no dependency on another of the workspace's crates, and no path into one.
//!
//! The editions' core, `crates/mutastic-core`, is a crate of its own for the same reason, and for
//! another edition to depend on: what it holds every edition shares, so it knows no language. The
//! modules `lib.rs` re-exports from it (`pub use mutastic_core::{…}`) are the `core` layer, just
//! above the kit. This holds it to depending on the kit alone of the workspace's crates, and on
//! nothing of Cargo's, rustc's or Rust's own: no parser, no build tool run, no Rust source file.

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
#[expect(
    clippy::expect_used,
    reason = "a test's helper: its expect is the test's assertion"
)]
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
/// The modules `lib.rs` declares, and those it re-exports from the kit.
#[expect(
    clippy::expect_used,
    reason = "a test's helper: its expect is the test's assertion"
)]
fn declared() -> Vec<String> {
    let text = std::fs::read_to_string(src().join("lib.rs")).expect("lib.rs reads");
    let mut declared: Vec<String> = SourceFile::parse(&text, Edition::CURRENT)
        .tree()
        .syntax()
        .children()
        .filter_map(Module::cast)
        .filter_map(|module| module.name())
        .map(|name| name.text().to_string())
        .collect();
    declared.extend(reexported(&text, KIT));
    declared.extend(reexported(&text, CORE));
    declared
}

/// The output kit's crate, as `lib.rs` names it.
const KIT: &str = "mutastic_output";

/// The editions' core's crate, as `lib.rs` names it.
const CORE: &str = "mutastic_core";

/// The modules `text` re-exports from `from`, a crate: `pub use mutastic_output::{similar,
/// untrusted};`.
fn reexported(text: &str, from: &str) -> Vec<String> {
    let lead = format!("pub use {from}::");
    text.lines()
        .filter_map(|line| line.trim().strip_prefix(lead.as_str()))
        .flat_map(|names| {
            names
                .trim_end_matches(';')
                .trim_matches(|c| c == '{' || c == '}')
                .split(',')
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn the_kit_is_the_lowest_layer() {
    let layers = layers();
    let text = std::fs::read_to_string(src().join("lib.rs")).expect("lib.rs reads");
    let kit = reexported(&text, KIT);
    assert!(!kit.is_empty(), "lib.rs re-exports the kit's modules");
    let (lowest, modules) = layers.last().expect("the layers' table has its rows");
    assert_eq!(lowest, "kit", "the kit is the layers' table's last row");
    assert_eq!(
        modules.iter().collect::<BTreeSet<_>>(),
        kit.iter().collect::<BTreeSet<_>>(),
        "the kit's row places what lib.rs re-exports from it, and only that"
    );
}

#[test]
fn the_core_is_the_layer_above_the_kit() {
    let layers = layers();
    let text = std::fs::read_to_string(src().join("lib.rs")).expect("lib.rs reads");
    let core = reexported(&text, CORE);
    assert!(!core.is_empty(), "lib.rs re-exports the core's modules");
    let (name, modules) = &layers[layers.len() - 2];
    assert_eq!(
        name, "core",
        "the core is the layers' table's row above the kit's"
    );
    assert_eq!(
        modules.iter().collect::<BTreeSet<_>>(),
        core.iter().collect::<BTreeSet<_>>(),
        "the core's row places what lib.rs re-exports from it, and only that"
    );
}

/// What the core may depend on: the kit, of the workspace's crates, and crates that know no
/// language.
const CORE_DEPENDENCIES: &[&str] = &["mutastic-output", "sha2", "serde_json"];

/// Words in the core's code that only one language's edition has: Cargo, rustc and its tests, the
/// Rust parser, and Rust source files.
const LANGUAGE_WORDS: &[&str] = &[
    "mutastic::",
    "mutastic_terminal",
    "ra_ap_syntax",
    "Cargo.toml",
    "rustc",
    "RUST_",
    "libtest",
    "\"cargo\"",
    "\"*.rs\"",
];

#[test]
fn the_core_knows_no_language() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR")).join("../mutastic-core");
    let manifest = std::fs::read_to_string(core.join("Cargo.toml")).expect("the core's manifest");
    let mut table = String::new();
    let mut taken = Vec::new();
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            table = line.to_string();
        } else if table.ends_with("dependencies]") && !line.is_empty() && !line.starts_with('#') {
            let name = line.split(['=', ' ', '.']).next().unwrap_or_default();
            if !CORE_DEPENDENCIES.contains(&name) {
                taken.push(format!("{table} {line}"));
            }
        }
    }
    assert!(
        taken.is_empty(),
        "the core depends only on the kit and crates that know no language, {CORE_DEPENDENCIES:?}, \
         so another edition can take it: {taken:?}"
    );
    let mut said = Vec::new();
    let mut dirs = vec![core.join("src")];
    while let Some(dir) = dirs.pop() {
        for path in std::fs::read_dir(&dir)
            .expect("the core's directory lists")
            .map(|entry| entry.expect("a directory lists").path())
        {
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let text = std::fs::read_to_string(&path).expect("a core file reads");
                for (at, line) in text.lines().enumerate() {
                    for word in LANGUAGE_WORDS.iter().filter(|word| line.contains(*word)) {
                        said.push(format!("{}:{}: {word}", path.display(), at + 1));
                    }
                }
            }
        }
    }
    assert!(
        said.is_empty(),
        "the core names what only the Rust edition knows; an edition gives its own words to the \
         core instead (see `since::changed_since_in`): {said:?}"
    );
}

#[test]
fn the_kit_knows_nothing_of_mutastic() {
    let kit = Path::new(env!("CARGO_MANIFEST_DIR")).join("../mutastic-output");
    let manifest = std::fs::read_to_string(kit.join("Cargo.toml")).expect("the kit's manifest");
    let mut tables = Vec::new();
    let mut table = String::new();
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            table = line.to_string();
        } else if table.ends_with("dependencies]")
            && (line.starts_with("mutastic") || line.contains("path"))
        {
            tables.push(format!("{table} {line}"));
        }
    }
    assert!(
        tables.is_empty(),
        "the kit depends on none of the workspace's crates, so it can't reach mutastic or carry \
         it: {tables:?}"
    );
    let mut paths = Vec::new();
    let mut dirs = vec![kit.join("src"), kit.join("tests")];
    while let Some(dir) = dirs.pop() {
        for path in std::fs::read_dir(&dir)
            .expect("the kit's directory lists")
            .map(|entry| entry.expect("a directory lists").path())
        {
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let text = std::fs::read_to_string(&path).expect("a kit file reads");
                for (at, line) in text.lines().enumerate() {
                    if line.contains("mutastic::") || line.contains("mutastic_terminal::") {
                        paths.push(format!("{}:{}", path.display(), at + 1));
                    }
                }
            }
        }
    }
    assert!(
        paths.is_empty(),
        "the kit names mutastic's crates: {paths:?}"
    );
}

/// A module's files: `src/<module>.rs`, and every file under `src/<module>/`.
#[expect(
    clippy::expect_used,
    reason = "a test's helper: its expect is the test's assertion"
)]
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
