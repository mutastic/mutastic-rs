/// The total, and one more.
pub fn total(prices: &[u32]) -> u32 {
    prices.iter().sum::<u32>() + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totals_add_one() {
        assert_eq!(total(&[2, 3]), 6);
    }
}
