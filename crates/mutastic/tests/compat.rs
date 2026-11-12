//! From 1.0.0, what a project gives mutastic keeps working: a command line, a `mutastic.yaml` and
//! a rules file that worked with a 1.0+ release work with every later one of the same major
//! version. Each such release keeps what it took in `tests/released/<version>/`: its
//! `commands.json` (every command, option, variable and exit status it declared), and the
//! `mutastic.yaml` and rules files of `fixtures/configured` as they were, each beside how it read
//! (`<file>.read`). This build is held to every one kept.
//!
//! Before 1.0.0 anything may change, so there's nothing to hold it to, and no earlier release may
//! stand in. From 1.0.0 the crate's version must be the newest release kept, so a release can't
//! ship without its copy.
//!
//! A command, an option, a choice, a variable or an exit status may be added; none may be taken
//! away, and an option keeps its kind and the commands it's on. A key may be added to either file;
//! none may be taken away or read another way.
//!
//! To keep a release's copy: `MUTASTIC_RELEASED_WRITE=1 cargo test --test compat`, at the release's
//! version (RELEASING.md).

#![allow(
    clippy::unwrap_used,
    reason = "a helper here asserts as the tests it serves do: a value that isn't there fails the test"
)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use mutastic::{callrules, commands, config};
use serde_json::Value;

const CONFIGURED: &str = "../../fixtures/configured";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn released() -> PathBuf {
    root().join("tests/released")
}

/// `1.2.3` as numbers; `None` for anything but a plain release.
fn numbers(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.split('.').map(|part| part.parse::<u64>().ok());
    let found = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(found)
}

/// The releases kept, oldest first.
fn kept() -> Vec<String> {
    let mut versions: Vec<String> = std::fs::read_dir(released())
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    versions.sort_by_key(|version| numbers(version));
    versions
}

/// The inputs a release keeps, by the name they're kept under: `.mutastic.yaml` for a
/// configuration, `.rules.yaml` for a rules file.
fn inputs() -> Vec<(String, PathBuf)> {
    let configured = root().join(CONFIGURED);
    vec![
        (
            "configured.mutastic.yaml".to_string(),
            configured.join(config::FILE),
        ),
        (
            "trims.rules.yaml".to_string(),
            configured.join("rules/trims.yaml"),
        ),
    ]
}

/// How a kept file reads, field by field: what a later release must read the same.
fn reading(name: &str, text: &str) -> Result<String, String> {
    if name.ends_with(".mutastic.yaml") {
        let read = config::parse(text)?;
        let mut lines: Vec<String> = read
            .rules
            .iter()
            .map(|rule| {
                format!(
                    "rule: mutate={} target={} reason={}",
                    rule.mutate, rule.target, rule.reason
                )
            })
            .collect();
        lines.extend(
            read.plugins
                .iter()
                .map(|p| format!("plugin: {}", p.display())),
        );
        lines.extend(read.operators.iter().map(|o| format!("operators: {o}")));
        Ok(lines.join("\n") + "\n")
    } else {
        let read = callrules::parse(text, name)?;
        let rules: Vec<String> = read
            .iter()
            .map(|rule| {
                format!(
                    "id: {}\nexplanation: {}\ntier: {}\nowners: {:?}\nmethods: {:?}\nrewrite: {:?}\n\
                     changed: {}\nheadline: {}\nhelp: {}\n",
                    rule.id,
                    rule.explanation,
                    rule.tier,
                    rule.owners,
                    rule.methods,
                    rule.rewrite,
                    rule.changed,
                    rule.headline,
                    rule.help
                )
            })
            .collect();
        Ok(rules.join("\n"))
    }
}

/// Writes this build's copy, as release `version` keeps it, into `dir`.
fn keep(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("commands.json"), commands::json()).unwrap();
    for (name, from) in inputs() {
        let text = std::fs::read_to_string(&from).unwrap();
        std::fs::write(dir.join(&name), &text).unwrap();
        let read = reading(&name, &text).unwrap();
        std::fs::write(dir.join(format!("{name}.read")), read).unwrap();
    }
}

fn names(list: &Value) -> BTreeSet<String> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry["name"].as_str().or(entry.as_str()))
        .map(str::to_string)
        .collect()
}

fn missing(kept: &BTreeSet<String>, now: &BTreeSet<String>) -> Vec<String> {
    kept.difference(now).cloned().collect()
}

/// What the commands this build declares break of those `kept` declared.
fn command_breaks(kept: &Value, now: &Value) -> Vec<String> {
    let mut breaks = Vec::new();
    for gone in missing(&names(&kept["commands"]), &names(&now["commands"])) {
        breaks.push(format!("the command `{gone}` is gone"));
    }
    for command in kept["commands"].as_array().into_iter().flatten() {
        let name = command["name"].as_str().unwrap_or_default();
        let current = now["commands"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| c["name"] == name);
        if let Some(current) = current {
            for gone in missing(&names(&command["options"]), &names(&current["options"])) {
                breaks.push(format!("`{name}` no longer takes `{gone}`"));
            }
        }
    }
    for option in kept["options"].as_array().into_iter().flatten() {
        let name = option["name"].as_str().unwrap_or_default();
        let Some(current) = now["options"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|o| o["name"] == name)
        else {
            breaks.push(format!("the option `{name}` is gone"));
            continue;
        };
        if current["kind"] != option["kind"] {
            breaks.push(format!(
                "`{name}` was {}, and is {}",
                option["kind"], current["kind"]
            ));
        }
        for gone in missing(&names(&option["choices"]), &names(&current["choices"])) {
            breaks.push(format!("`{name}` no longer takes `{gone}`"));
        }
        for gone in missing(&names(&option["commands"]), &names(&current["commands"])) {
            breaks.push(format!("`{name}` is no longer an option of `{gone}`"));
        }
    }
    for (list, what) in [("environment", "variable"), ("exitCodes", "exit status")] {
        for gone in missing(&names(&kept[list]), &names(&now[list])) {
            breaks.push(format!("the {what} `{gone}` is gone"));
        }
    }
    breaks
}

/// What this build breaks of the copy kept in `dir`.
fn breaks(dir: &Path) -> Vec<String> {
    let kept: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("commands.json")).unwrap()).unwrap();
    let mut breaks = command_breaks(&kept, &commands::model());
    for (name, _) in inputs() {
        let file = dir.join(&name);
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let was = std::fs::read_to_string(dir.join(format!("{name}.read"))).unwrap_or_default();
        match reading(&name, &text) {
            Err(error) => breaks.push(format!("{name} no longer reads: {error}")),
            Ok(now) if now != was => breaks.push(format!("{name} reads differently:\n{now}")),
            Ok(_) => {}
        }
    }
    breaks
}

#[test]
fn the_kept_releases_are_1_0_or_later_and_end_at_this_version() {
    let own = env!("CARGO_PKG_VERSION");
    if std::env::var_os("MUTASTIC_RELEASED_WRITE").is_some() {
        assert!(
            numbers(own).is_some_and(|(major, _, _)| major >= 1),
            "only a 1.0+ release keeps a copy, and this is {own}"
        );
        keep(&released().join(own));
    }
    let kept = kept();
    let stable = numbers(own).is_some_and(|(major, _, _)| major >= 1);
    assert!(
        kept.iter()
            .all(|v| numbers(v).is_some_and(|(major, _, _)| major >= 1)),
        "tests/released keeps only 1.0+ releases: {kept:?}"
    );
    if stable {
        assert_eq!(
            kept.last().map(String::as_str),
            Some(own),
            "{own} is a release: keep its copy, MUTASTIC_RELEASED_WRITE=1 cargo test --test compat"
        );
    } else {
        assert!(
            kept.is_empty(),
            "no 1.0+ release yet, so nothing to hold this to: {kept:?}"
        );
    }
}

#[test]
fn every_kept_release_still_works() {
    for version in kept() {
        let found = breaks(&released().join(&version));
        assert!(
            found.is_empty(),
            "this build breaks what {version} took:\n{}",
            found.join("\n")
        );
    }
}

/// The check itself, before there's a release to hold to: this build keeps its own copy's
/// promises, and breaks those of a copy that took more.
#[test]
fn a_copy_of_this_build_holds_and_one_that_took_more_breaks() {
    let dir = std::env::temp_dir().join(format!("mutastic-compat-{}", std::process::id()));
    keep(&dir);
    assert_eq!(breaks(&dir), Vec::<String>::new());

    let mut more: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("commands.json")).unwrap()).unwrap();
    more["commands"][0]["options"]
        .as_array_mut()
        .unwrap()
        .push("--gone".into());
    more["options"][0]["choices"]
        .as_array_mut()
        .unwrap()
        .push("gone".into());
    more["environment"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({ "name": "MUTASTIC_GONE" }));
    std::fs::write(dir.join("commands.json"), more.to_string()).unwrap();
    let rules = dir.join("trims.rules.yaml.read");
    std::fs::write(&rules, std::fs::read_to_string(&rules).unwrap() + "x").unwrap();

    let found = breaks(&dir);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(found.len(), 4, "{found:#?}");
    assert!(found[0].ends_with("no longer takes `--gone`"), "{found:#?}");
    assert!(found[1].ends_with("no longer takes `gone`"), "{found:#?}");
    assert!(found[2].contains("`MUTASTIC_GONE` is gone"), "{found:#?}");
    assert!(
        found[3].starts_with("trims.rules.yaml reads differently"),
        "{found:#?}"
    );
}
