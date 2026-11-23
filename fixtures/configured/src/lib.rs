//! A crate with a `mutastic.yaml`: a rule that skips generated code, a rules file's operator, and
//! the operators it chooses.

/// A customer's name, as they typed it.
pub fn clean(name: &str) -> String {
    name.trim().to_lowercase()
}

/// A discount, from 100 up.
pub fn discount(total: u32) -> u32 {
    if total >= 100 {
        total / 10
    } else {
        0
    }
}

/// The port to listen on: 8080 unless one is set.
pub fn port(set: Option<u16>) -> u16 {
    set.unwrap_or(8080)
}

/// How many times to try again: 3 unless a number is set.
pub fn retries(set: Option<u8>) -> u8 {
    set.unwrap_or(3)
}

/// The first page of a list.
pub fn first_page(items: &[u32], size: usize) -> Vec<u32> {
    items.iter().copied().take(size).collect()
}

/// The highest scores, of scores already in order.
pub fn top(scores: &[u32], count: usize) -> Vec<u32> {
    scores.iter().copied().take(count).collect()
}

/// Two amounts together, or nothing when that's more than a `u32` holds.
pub fn sum(amount: u32, other: u32) -> Option<u32> {
    amount.checked_add(other)
}

/// A volume turned up, as far as it goes.
pub fn louder(volume: u8, by: u8) -> u8 {
    volume.saturating_add(by)
}

pub mod generated {
    /// Written by a code generator: the rule skips it.
    pub fn checksum(bytes: &[u8]) -> u32 {
        bytes.iter().map(|b| *b as u32).sum::<u32>() + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_lower_case() {
        assert_eq!(clean("Ada"), "ada");
    }

    #[test]
    fn big_totals_get_a_tenth_off() {
        assert_eq!(discount(250), 25);
        assert_eq!(discount(50), 0);
        assert_eq!(discount(100), 10);
    }

    #[test]
    fn a_port_is_8080_unless_set() {
        assert_eq!(port(None), 8080);
    }

    #[test]
    fn retries_are_the_number_set() {
        assert_eq!(retries(Some(5)), 5);
        assert_eq!(retries(None), 3);
    }

    #[test]
    fn a_short_list_is_one_page() {
        assert_eq!(first_page(&[1, 2], 5), [1, 2]);
    }

    #[test]
    fn the_top_two_scores() {
        assert_eq!(top(&[9, 7, 4], 2), [9, 7]);
    }

    #[test]
    fn amounts_add_up() {
        assert_eq!(sum(1, 2), Some(3));
    }

    #[test]
    fn volume_stops_at_the_top() {
        assert_eq!(louder(250, 10), 255);
        assert_eq!(louder(1, 2), 3);
    }
}
