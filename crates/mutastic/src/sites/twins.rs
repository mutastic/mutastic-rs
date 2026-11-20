//! Mutants at one place that always do the same: twins. Their ids were kept before mutastic knew
//! they're alike, and an id once kept names its mutant for good, so each is still made; but only
//! one of them runs, and each other is skipped as `twin: same as <id>`, naming it, so the place
//! counts once in the score.
//!
//! Two ways are known. The same code: two operators that replace the same code with the same
//! code, as mutastic/return-value and mutastic/remove-conditional both make a returned `n > 0`
//! `true`. And a comparison of a value the source proves unsigned with 0, which only ever sees 0
//! and above: two changes that give the same for both are twins, `n > 0` made `n >= 0` and made
//! `true`.
//!
//! The one that runs is the first in the defaults, then stronger, then extended, in the order the
//! mutants are found; a mutant whose twins aren't in the run's operators runs as any mutant does.

use super::{Mutant, DEFAULTS, STRONGER};
use std::collections::BTreeMap;

/// How a twin's skip starts: `twin: same as mb2f8ae57`.
pub const SAME_AS: &str = "twin: same as ";

/// The id of the mutant `mutant` is the same as, which runs for it, when it's a twin.
pub fn partner_of(mutant: &Mutant) -> Option<&str> {
    mutant.skipped.as_deref()?.strip_prefix(SAME_AS)
}

/// The key two mutants share when they make the same code: the same bytes of the same file
/// replaced by the same text, in the same form.
pub(super) fn code_key(mutant: &Mutant) -> String {
    format!(
        "code@{}:{}..{}:{:?}:{}",
        mutant.file.display(),
        mutant.site.start,
        mutant.site.end,
        mutant.form,
        mutant.replacement
    )
}

/// Which set an operator is in, the defaults first.
fn tier(operator: &str) -> usize {
    if DEFAULTS.contains(&operator) {
        0
    } else if STRONGER.contains(&operator) {
        1
    } else {
        2
    }
}

/// Skips each mutant that is the same as one found before it of a set, as the same as the one of
/// the set that runs. Only a mutant that would run is one: a skip of another kind says more.
pub(super) fn mark(mutants: &mut [Mutant]) {
    // Each mutant's set, as the index of the first mutant in it: two that share a key are in one.
    let mut set: Vec<usize> = (0..mutants.len()).collect();
    fn first(set: &mut [usize], at: usize) -> usize {
        let mut at = at;
        while set[at] != at {
            set[at] = set[set[at]];
            at = set[at];
        }
        at
    }
    let mut by_key: BTreeMap<&str, usize> = BTreeMap::new();
    for (index, mutant) in mutants.iter().enumerate() {
        if mutant.skipped.is_some() {
            continue;
        }
        for key in &mutant.twin_keys {
            match by_key.get(key.as_str()) {
                Some(&other) => {
                    let (one, two) = (first(&mut set, other), first(&mut set, index));
                    set[one.max(two)] = one.min(two);
                }
                None => {
                    by_key.insert(key, index);
                }
            }
        }
    }
    let mut members: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (index, mutant) in mutants.iter().enumerate() {
        if mutant.skipped.is_none() {
            members
                .entry(first(&mut set, index))
                .or_default()
                .push(index);
        }
    }
    for twins in members.values().filter(|twins| twins.len() > 1) {
        let runs = *twins
            .iter()
            .min_by_key(|&&index| (tier(mutants[index].operator), index))
            .expect("a set has members");
        let id = mutants[runs].id.clone();
        for &index in twins.iter().filter(|&&index| index != runs) {
            mutants[index].skipped = Some(format!("{SAME_AS}{id}"));
        }
    }
}
