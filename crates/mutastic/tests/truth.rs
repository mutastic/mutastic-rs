//! A card says what its mutant does. For every mutant of `fixtures/truth`, which holds each shape
//! a mutated expression can take, this writes the card's claim (the `+` line's change) into a copy
//! of the source, builds all those copies in one crate, and checks each behaves as the mutant does
//! in mutastic's own build, over a range of inputs.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use mutastic::compile::{self, Binary};
use mutastic::{engine, sites, testrun, workspace};

fn fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
        .display()
        .to_string()
}

/// What the probe printed, `-3: 2 0 …`, and whether it finished: a mutant can make a loop run
/// until it overflows, or forever, and how far it gets before then depends on the build's speed.
#[derive(Debug, PartialEq)]
struct Probed {
    lines: Vec<String>,
    finished: bool,
}

impl Probed {
    /// The same behaviour: both finish with the same lines, or neither does and the lines both
    /// printed agree.
    fn agrees(&self, other: &Probed) -> bool {
        if self.finished && other.finished {
            return self.lines == other.lines;
        }
        self.finished == other.finished && self.lines.iter().zip(&other.lines).all(|(a, b)| a == b)
    }
}

/// Operators whose mutants can fail to compile, for the build to withdraw.
const ROLLBACK: &[&str] = &[
    sites::ARITHMETIC_REPLACEMENT,
    sites::OTHER_ARITHMETIC,
    sites::KEEP_OPERAND,
    sites::OMIT_CALL,
    sites::SWAP_ARGUMENTS,
    sites::SWAP_INDEX_OF,
    sites::REVERSE_ORDERING,
    sites::REMOVE_SWITCH_ARM,
    sites::UNWRAP_TRY,
    sites::DEFAULT_PARAMETER,
    sites::DROP_VARARG,
    sites::REMOVE_OTHER_STAGE,
    sites::REMOVE_FILTER,
    sites::REMOVE_STEP,
    sites::REMOVE_THROW,
    sites::SWAP_BREAK,
    sites::SWAP_PRESENT,
];

fn probe(binary: &Binary, cwd: &Path, test: &str, env: &[(&str, String)]) -> Probed {
    let outcome = testrun::run(
        binary,
        cwd,
        &[test, "--nocapture"],
        env,
        Duration::from_secs(3),
    );
    let lines = outcome
        .stdout
        .lines()
        .filter(|line| {
            line.split_once(": ")
                .is_some_and(|(x, _)| x.parse::<i32>().is_ok())
        })
        .map(str::to_string)
        .collect();
    Probed {
        lines,
        finished: outcome.passed,
    }
}

#[test]
fn every_card_claim_behaves_as_its_mutant_does() {
    std::env::set_var("MUTASTIC_EXE", env!("CARGO_BIN_EXE_mutastic"));
    let project = workspace::read(&fixture("truth")).unwrap();
    let found = engine::find(&project, sites::ALL);
    assert!(found.mutants.len() > 40, "{} mutants", found.mutants.len());
    let all: BTreeSet<usize> = (0..found.mutants.len()).collect();
    let built = compile::build(&project, &found, &all, &BTreeSet::new()).unwrap();
    // Only the operators that lean on the build may make a mutant that doesn't compile.
    let wrongly_unviable: Vec<String> = built
        .unviable
        .iter()
        .filter(|(&index, _)| !ROLLBACK.contains(&found.mutants[index].operator))
        .map(|(&index, error)| format!("{} {error}", found.mutants[index].operator))
        .collect();
    assert!(
        wrongly_unviable.is_empty(),
        "every mutant of these operators compiles: {wrongly_unviable:?}"
    );
    let viable: Vec<&sites::Mutant> = found
        .mutants
        .iter()
        .enumerate()
        .filter(|(index, _)| !built.unviable.contains_key(index))
        .map(|(_, mutant)| mutant)
        .collect();
    let schema = built.binaries.iter().find(|b| b.kind == "lib").unwrap();

    // The claims, each in a module of its own, in one crate.
    let file = &found.files[0];
    // Lints are capped in mutastic's build, so a claim that divides by a constant zero compiles
    // there and panics when run; here too.
    let allow = "#[allow(warnings, unconditional_panic, arithmetic_overflow)]";
    let mut claims = format!("{allow}\nmod v0 {{\n{}\n}}\n", file.text);
    for mutant in &viable {
        let (range, with) = mutant.claim();
        let mut text = file.text.clone();
        text.replace_range(range, &with);
        claims.push_str(&format!(
            "{allow}\nmod v{} {{\n{text}\n}}\n",
            mutant.ordinal
        ));
    }
    let dir = claims_dir();
    workspace::write_if_changed(
        &dir.join("Cargo.toml"),
        "[package]\nname = \"claims\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .unwrap();
    workspace::write_if_changed(&dir.join("src/lib.rs"), &claims).unwrap();
    let output = Command::new(workspace::cargo())
        .args(["test", "--no-run", "--lib", "--message-format=json"])
        .current_dir(&dir)
        .env("CARGO_TARGET_DIR", dir.join("target"))
        // Incremental builds of a crate this size keep gigabytes, more with each change.
        .env("CARGO_INCREMENTAL", "0")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let claims_binary = Binary {
        target: "claims".into(),
        kind: "lib".into(),
        path: PathBuf::from(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .find_map(|message| message["executable"].as_str().map(str::to_string))
                .unwrap(),
        ),
    };

    let cwd = project.copy_dir().join(&project.package_dir);
    let ordinals: Vec<u32> = std::iter::once(0)
        .chain(viable.iter().map(|m| m.ordinal))
        .collect();
    let compared = testrun::parallel(&ordinals, 4, |&ordinal| {
        let mutant = probe(
            schema,
            &cwd,
            "tests::probe",
            &[("MUTASTIC_ACTIVE", ordinal.to_string())],
        );
        let claim = probe(
            &claims_binary,
            &dir,
            &format!("v{ordinal}::tests::probe"),
            &[],
        );
        (ordinal, mutant, claim)
    });
    let original = &compared[0].1;
    assert!(
        original.finished && original.lines.len() == 16,
        "the probe ran: {original:?}"
    );
    let mut wrong = Vec::new();
    let mut changed = 0;
    for (ordinal, mutant, claim) in &compared {
        if mutant != original {
            changed += 1;
        }
        if !mutant.agrees(claim) {
            let m = &found.mutants[*ordinal as usize - 1];
            wrong.push(format!(
                "{} {} at line {}: the card says `{}`, but the mutant differs\n  mutant: {:?}\n  claim:  {:?}",
                m.id,
                m.operator,
                m.line,
                m.claim().1,
                mutant.lines.first(),
                claim.lines.first()
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    // Most mutants change something the probe sees, so the comparison means something.
    assert!(
        changed * 10 > compared.len() * 8,
        "{changed} of {}",
        compared.len()
    );
}

/// Where the claims crate is built: one place under the workspace's own `target`, written over
/// by each run, so repeated runs don't grow it and `cargo clean` removes it.
fn claims_dir() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("truth-claims")
}

/// A copy of a fixture in a directory of its own, without its `target`, to paste tests into.
fn copy_of(name: &str) -> PathBuf {
    let to = std::env::temp_dir().join(format!("mutastic-suggested-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&to).ok();
    let from = PathBuf::from(fixture(name));
    let mut stack = vec![PathBuf::new()];
    while let Some(relative) = stack.pop() {
        std::fs::create_dir_all(to.join(&relative)).unwrap();
        for entry in std::fs::read_dir(from.join(&relative)).unwrap() {
            let entry = entry.unwrap();
            let path = relative.join(entry.file_name());
            if entry.file_name() == "target" {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                stack.push(path);
            } else {
                std::fs::copy(from.join(&path), to.join(&path)).unwrap();
            }
        }
    }
    to
}

fn mutastic_in(dir: &Path, args: &[&str]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_mutastic"))
        .args(args)
        .current_dir(dir)
        .env_remove("MUTASTIC_FORMAT")
        .env_remove("MUTASTIC_AGENT")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned()
            + &String::from_utf8_lossy(&output.stderr),
    )
}

/// `code` pasted into the module `module` names in `text`: after the brace that opens it, found by
/// each of its names in turn (the first may be the file's own module, with no `mod` line); or, when
/// there's no such module, in a new test module at the end, as the card says.
fn pasted(text: &str, module: &str, code: &str) -> String {
    let names: Vec<&str> = module.split("::").collect();
    let mut at = 0;
    let mut opened = false;
    for name in &names {
        let open = format!("mod {name} {{");
        opened = match text[at..].find(&open) {
            Some(found) => {
                at += found + open.len();
                true
            }
            None => false,
        };
    }
    if opened {
        format!("{}\n{code}{}", &text[..at], &text[at..])
    } else {
        format!("{text}\n#[cfg(test)]\nmod tests {{\n{code}}}\n")
    }
}

/// Runs `dir` with `operators`, pastes every test its cards suggest into its module, and checks
/// they compile, pass as the code is (or `verify` couldn't run), and catch the mutants they were
/// found for. Gives the events that had one.
fn suggested_tests_hold(dir: &Path, operators: &str) -> Vec<serde_json::Value> {
    let (_, jsonl) = mutastic_in(
        dir,
        &[
            "run",
            "--format=jsonl",
            "--report=none",
            &format!("--operators={operators}"),
        ],
    );
    let suggested: Vec<serde_json::Value> = jsonl
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|event| event.get("suggestedTest").is_some())
        .collect();
    let mut by_place: BTreeMap<(String, String), BTreeMap<String, String>> = BTreeMap::new();
    for event in &suggested {
        let test = &event["suggestedTest"];
        let indented: String = test["code"]
            .as_str()
            .unwrap()
            .lines()
            .map(|line| format!("    {line}\n"))
            .collect();
        by_place
            .entry((
                event["location"]["file"].as_str().unwrap().to_string(),
                test["testClass"].as_str().unwrap().to_string(),
            ))
            .or_default()
            .insert(test["name"].as_str().unwrap().to_string(), indented);
    }
    for ((file, module), tests) in &by_place {
        let path = dir.join(file);
        let text = std::fs::read_to_string(&path).unwrap();
        let code: String = tests.values().cloned().collect();
        std::fs::write(&path, pasted(&text, module, &code)).unwrap();
    }
    if suggested.is_empty() {
        return suggested;
    }
    let ids: Vec<&str> = suggested
        .iter()
        .map(|event| event["id"].as_str().unwrap())
        .collect();
    let (code, out) = mutastic_in(dir, &["verify", &ids.join(",")]);
    assert_eq!(
        code, 0,
        "every suggested test compiles, passes and catches: {out}"
    );
    for id in ids {
        assert!(
            out.contains(&format!("mutastic: {id} is caught by ")),
            "{out}"
        );
    }
    suggested
}

/// A suggested test says something true. Pasted into its module, every test the fixtures' cards
/// suggest compiles, passes as the code is, and catches the mutants it was found for; and a
/// function that answers one call two ways gets none.
#[test]
fn every_suggested_test_compiles_and_catches_its_mutant() {
    for (name, operators, least) in [
        ("pricing", "defaults", 1),
        ("suggestions", "defaults", 3),
        ("operators", "extended", 10),
    ] {
        let dir = copy_of(name);
        let suggested = suggested_tests_hold(&dir, operators);
        assert!(suggested.len() >= least, "{name}: {}", suggested.len());
        if name == "suggestions" {
            names_the_values_the_source_names(&suggested);
        }
        assert!(
            suggested
                .iter()
                .all(|e| e["location"]["method"] != "jitter"),
            "a clock's answer can't be pinned"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}

/// A suggested test says a value the way the source does: a `const` the comparison reads is
/// passed by name, its value in the comment, and a field the receiver is made with is one local,
/// passed to both; each test is named for it.
fn names_the_values_the_source_names(suggested: &[serde_json::Value]) {
    let code = |function: &str| -> Vec<String> {
        suggested
            .iter()
            .filter(|e| e["location"]["method"] == function)
            .map(|e| e["suggestedTest"]["code"].as_str().unwrap().to_string())
            .collect()
    };
    assert!(
        code("discount").contains(
            &"#[test]\nfn discount_at_free_from() {\n    assert_eq!(discount(FREE_FROM), 10); // FREE_FROM = 100 · today 10 · with this mutant 0\n}".to_string()
        ),
        "{:?}",
        code("discount")
    );
    assert!(
        code("is_free").iter().any(|test| test
            .starts_with("#[test]\nfn is_free_at_free_from() {\n    let free_from = ")
            && test.contains("Delivery { free_from }.is_free(free_from)")),
        "{:?}",
        code("is_free")
    );
}

/// The library `cargo new --lib` makes: its test calls `add(2, 2)`, which can't tell `+` from `*`,
/// and the card gives a call that can, with the values running it gave.
#[test]
fn a_new_librarys_add_gets_a_test_that_tells_plus_from_times() {
    let dir = std::env::temp_dir().join(format!("mutastic-new-lib-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"mylib\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/lib.rs"),
        "pub fn add(left: u64, right: u64) -> u64 {\n    left + right\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn it_works() {\n        let result = add(2, 2);\n        assert_eq!(result, 4);\n    }\n}\n",
    )
    .unwrap();
    let suggested = suggested_tests_hold(&dir, "stronger");
    let times = suggested
        .iter()
        .find(|e| e["mutation"]["replacement"] == "*")
        .expect("+ becoming * is missed, and gets a test");
    assert_eq!(
        times["suggestedTest"]["code"],
        "#[test]\nfn add_with_left_2_and_right_0() {\n    assert_eq!(add(2, 0), 2); // today 2 · with this mutant 0\n}"
    );
    std::fs::remove_dir_all(&dir).ok();
}
