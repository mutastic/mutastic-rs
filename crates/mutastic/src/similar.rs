//! The one name close to a word that isn't one: offered as a statement, "there's one called …",
//! never acted on. A guess acted on runs what nobody asked for; a guess offered costs a glance.

/// The candidate closest to `word`, if one is close enough: the same name in other case first,
/// then the fewest edits, where a swap of two letters side by side is one edit, so `tset` reaches
/// `test`. At most a third of the word's length (at least one) may change, and of two as close, the
/// one nearer its length wins, then the first: `tset` finds `test` before `set`.
pub fn closest<'a>(word: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let lower = word.to_lowercase();
    let length = word.chars().count();
    let most = length.max(3) / 3;
    candidates
        .into_iter()
        .filter(|candidate| *candidate != word)
        .map(|candidate| {
            let edits = edits(&lower, &candidate.to_lowercase());
            let apart = length.abs_diff(candidate.chars().count());
            (edits, apart, candidate)
        })
        .filter(|(edits, _, _)| *edits <= most)
        .min_by_key(|(edits, apart, _)| (*edits, *apart))
        .map(|(_, _, candidate)| candidate)
}

/// The edits from `from` to `to`: a letter added, taken away or changed, or two side by side
/// swapped, each one edit.
fn edits(from: &str, to: &str) -> usize {
    let from: Vec<char> = from.chars().collect();
    let to: Vec<char> = to.chars().collect();
    // Rows of the table: the one before the last, the last, and the one being filled.
    let mut before: Vec<usize> = vec![0; to.len() + 1];
    let mut last: Vec<usize> = (0..=to.len()).collect();
    for i in 1..=from.len() {
        let mut row = vec![i; to.len() + 1];
        for j in 1..=to.len() {
            let changed = usize::from(from[i - 1] != to[j - 1]);
            let mut best = (last[j] + 1).min(row[j - 1] + 1).min(last[j - 1] + changed);
            if i > 1 && j > 1 && from[i - 1] == to[j - 2] && from[i - 2] == to[j - 1] {
                best = best.min(before[j - 2] + 1);
            }
            row[j] = best;
        }
        before = std::mem::replace(&mut last, row);
    }
    last[to.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_swap_is_one_edit_and_the_nearest_length_wins_a_tie() {
        assert_eq!(edits("tset", "test"), 1);
        assert_eq!(edits("tset", "set"), 1);
        assert_eq!(edits("kitten", "sitting"), 3);
        assert_eq!(closest("tset", ["set", "test"]), Some("test"));
        assert_eq!(closest("lsit", ["edit", "list"]), Some("list"));
    }

    #[test]
    fn only_a_close_name_is_offered() {
        let members = ["mutastic", "cargo-mutastic", "mutastic-rs"];
        for (typed, offered) in [
            ("mutastic-sr", Some("mutastic-rs")),
            ("Mutastic-RS", Some("mutastic-rs")),
            ("mutastc", Some("mutastic")),
            ("crates", None),
            ("rs", None),
            ("mutastic", None),
        ] {
            assert_eq!(closest(typed, members), offered, "{typed}");
        }
    }
}
