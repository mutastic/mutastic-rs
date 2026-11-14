//! A ledger's tests, one of them broken on purpose, as the Java edition's `false-catches` sample
//! has them: it passes the first time it runs, so it passes before any mutant, but fails against
//! every mutant it reaches, and on its own once the mutants have run. Taken at face value, its
//! failures would be catches.

use std::path::Path;

use ledger::{fee, waived, Ledger};

/// Broken on purpose: one file in the build's directory, shared and never cleaned up. mutastic's
/// copy starts each run without it, so the test passes the first time it runs in a run, before
/// any mutant; after that, the entry it left behind fails it.
fn shared() -> Ledger {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    std::fs::create_dir_all(&dir).unwrap();
    Ledger::new(dir.join("ledger.txt"))
}

#[test]
fn a_new_ledger_takes_an_amount_less_its_fee() {
    let ledger = shared();
    assert!(ledger.is_empty().unwrap());
    ledger.record(100 - fee(100, 10)).unwrap();
    assert_eq!(ledger.total().unwrap(), 90);
}

#[test]
fn a_fee_is_never_negative() {
    assert!(fee(100, 10) >= 0);
}

#[test]
fn waives_from_the_threshold() {
    assert!(waived(100, 100));
    assert!(!waived(99, 100));
}

/// A ledger of its own, in a file no other test or process uses, gone once it's checked.
#[test]
fn a_ledger_of_its_own_adds_up() {
    let file = std::env::temp_dir().join(format!(
        "mutastic-ledger-fixture-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&file);
    let ledger = Ledger::new(&file);
    ledger.record(5).unwrap();
    ledger.record(-7).unwrap();
    let total = ledger.total();
    let _ = std::fs::remove_file(&file);
    assert_eq!(total.unwrap(), -2);
}
