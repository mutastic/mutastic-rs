//! A card says what its mutant does. For every mutant of `fixtures/truth`, which holds each shape
//! a mutated expression can take, this writes the card's claim (the `+` line's change) into a copy
//! of the source, builds all those copies in one crate, and checks each behaves as the mutant does
//! in mutastic's own build, over a range of inputs.

use std::collections::BTreeSet;
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
    let found = engine::find(&project, sites::STRONGER);
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
    let dir = project.mutastic_dir().join("claims");
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
