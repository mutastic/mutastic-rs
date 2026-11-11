//! Conditions: comparisons, `if`, `&&` and `||`.

use std::num::NonZeroU32;

/// Free delivery from 50 up.
pub fn free_delivery(total: u32) -> bool {
    total >= 50
}

/// Time to restock once stock falls below the low mark.
pub fn needs_restock(stock: u32, low: u32) -> bool {
    stock < low
}

pub fn greeting(member: bool) -> &'static str {
    if member {
        "welcome back"
    } else {
        "welcome"
    }
}

pub fn can_ship(paid: bool, in_stock: bool) -> bool {
    paid && in_stock
}

/// Saturday is 6 and Sunday 7.
pub fn is_weekend(day: u8) -> bool {
    matches!(day, 6 | 7)
}

/// An equal share for each part. A `NonZeroU32` is never 0, so the guard never passes.
pub fn share(total: u32, parts: NonZeroU32) -> u32 {
    if parts.get() == 0 {
        unreachable!("a NonZeroU32 is never 0");
    }
    total / parts.get()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_orders_pay_for_delivery() {
        assert!(!free_delivery(20));
        assert!(free_delivery(80));
    }

    #[test]
    fn restock_when_low() {
        assert!(needs_restock(2, 5));
        assert!(!needs_restock(5, 5));
    }

    #[test]
    fn members_are_welcomed_back() {
        assert_eq!(greeting(true), "welcome back");
    }

    #[test]
    fn ships_when_paid_and_in_stock() {
        assert!(can_ship(true, true));
        assert!(!can_ship(false, false));
    }

    #[test]
    fn weekend_days_are_known() {
        let saturday = is_weekend(6);
        println!("saturday: {saturday}");
    }

    #[test]
    fn three_parts_share_ten() {
        assert_eq!(share(10, NonZeroU32::new(3).unwrap()), 3);
    }
}
