//! The one name close to a word that isn't one: offered as a statement, "there's one called …",
//! never acted on. A guess acted on runs what nobody asked for; a guess offered costs a glance.

/// The candidate closest to `word`, if one is close enough: the same name in other case first,
/// then the fewest edits, where a swap of two letters side by side is one edit, so `tset` reaches
/// `test`. At most a third of the word's length (at least one) may change, and of two as close, the
/// one nearer its length wins, then the first: `tset` finds `test` before `set`.
pub fn closest<'a>(word: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    closest_within(word, candidates, word.chars().count().max(3) / 3)
}

/// The candidate closest to `word` with at most `most` edits. A mutant's id is eight hex digits
/// with no meaning to a near miss: one digit wrong, or two swapped, is a typo, and three in nine
/// would be some other mutant's id by chance.
pub fn closest_within<'a>(
    word: &str,
    candidates: impl IntoIterator<Item = &'a str>,
    most: usize,
) -> Option<&'a str> {
    let lower = word.to_lowercase();
    let length = word.chars().count();
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

/// " There's one called `x`.", as the sentence after the one saying `word` isn't a name mutastic
/// has; nothing when no name is close.
pub fn offered<'a>(word: &str, candidates: impl IntoIterator<Item = &'a str>) -> String {
    closest(word, candidates).map_or(String::new(), |name| format!(" There's one called {name}."))
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

    /// The typos both editions are held to: the word typed, the names it could be, and the one
    /// offered, `-` for none.
    #[test]
    fn the_typos_table_offers_what_it_says() {
        let table = include_str!("../tests/typos.tsv");
        let rows = table
            .lines()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty());
        let mut checked = 0;
        for row in rows {
            let [typed, names, offered]: [&str; 3] = row
                .split('\t')
                .collect::<Vec<_>>()
                .try_into()
                .unwrap_or_else(|_| panic!("three columns: {row}"));
            let expected = (offered != "-").then_some(offered);
            assert_eq!(closest(typed, names.split(',')), expected, "{row}");
            checked += 1;
        }
        assert!(checked >= 20, "{checked} rows");
    }

    #[test]
    fn an_id_is_offered_only_one_edit_away() {
        let ids = ["mb2f8ae57", "m95f96847"];
        assert_eq!(closest_within("mb2f8ae58", ids, 1), Some("mb2f8ae57"));
        assert_eq!(closest_within("mb2f8ea57", ids, 1), Some("mb2f8ae57"));
        assert_eq!(closest_within("mb2f8ae00", ids, 1), None);
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
