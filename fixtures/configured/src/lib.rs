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
}
