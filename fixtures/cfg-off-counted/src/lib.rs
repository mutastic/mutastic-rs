//! Code under `cfg`s nothing sets, beside code that's built: only the built code is mutated.

/// The total, built everywhere.
pub fn total(prices: &[u32]) -> u32 {
    prices.iter().sum::<u32>() + 1
}

/// A flag nothing sets: not built.
#[cfg(my_flag)]
pub fn flagged(count: u32) -> u32 {
    count * 2
}

/// A name and value nothing sets: not built.
#[cfg(note = "x")]
pub fn noted(count: u32) -> u32 {
    count * 3
}

/// A feature that isn't on: not built.
#[cfg(feature = "nope")]
pub fn featured(count: u32) -> u32 {
    count * 4
}

/// Another platform's: not built, unless this is it.
#[cfg(target_os = "windows")]
pub fn windowed(count: u32) -> u32 {
    count * 5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totals_add_one() {
        assert_eq!(total(&[2, 3]), 6);
    }
}
