//! Code for one platform or another, and behind features: only the code this build has is
//! mutated.

#[cfg(windows)]
mod windows;

/// Whether a path starts at the root, on Unix.
#[cfg(unix)]
pub fn is_absolute(path: &str) -> bool {
    path.starts_with('/') && path.len() > 1
}

/// Whether a path starts at a drive, on Windows.
#[cfg(windows)]
pub fn is_absolute(path: &str) -> bool {
    path.len() > 2 && path.as_bytes()[1] == b':'
}

/// The total, with the standard library: a default feature, so it's built.
#[cfg(feature = "std")]
pub fn total(prices: &[u32]) -> u32 {
    prices.iter().sum::<u32>() + 1
}

/// The total, four at a time: a feature that isn't on by default.
#[cfg(feature = "simd")]
pub fn total_wide(prices: &[u32]) -> u32 {
    prices
        .chunks(4)
        .map(|four| four.iter().sum::<u32>())
        .sum::<u32>()
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn absolute_paths_start_at_the_root() {
        assert!(is_absolute("/usr"));
        assert!(!is_absolute("usr"));
        assert!(!is_absolute("/"));
    }

    #[test]
    fn totals_add_one() {
        assert_eq!(total(&[2, 3]), 6);
    }
}
