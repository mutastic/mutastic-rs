//! Iterator chains.

/// Whether any invoice is more than 30 days late.
pub fn any_overdue(days_late: &[u32]) -> bool {
    days_late.iter().any(|days| *days > 30)
}

/// The total of the orders that are paid.
pub fn paid_total(orders: &[(u32, bool)]) -> u32 {
    orders
        .iter()
        .filter(|(_, paid)| *paid)
        .map(|(total, _)| total)
        .sum()
}

/// The total, with each value written to `log` as it's added.
pub fn logged_total(values: &[u32], log: &mut Vec<u32>) -> u32 {
    values.iter().inspect(|value| log.push(**value)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_late_invoice() {
        assert!(any_overdue(&[45]));
    }

    #[test]
    fn a_paid_order() {
        assert_eq!(paid_total(&[(10, true)]), 10);
    }

    #[test]
    fn totals_add_up() {
        let mut log = Vec::new();
        assert_eq!(logged_total(&[1, 2], &mut log), 3);
    }
}
