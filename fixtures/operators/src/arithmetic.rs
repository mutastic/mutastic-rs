//! Arithmetic: operators, numbers and bits.

pub fn line_total(price: u32, quantity: u32) -> u32 {
    price * quantity
}

pub fn with_fee(price: u32, fee: u32) -> u32 {
    price + fee
}

pub fn per_person(bill: u32, people: u32) -> u32 {
    bill / people
}

pub fn after_refund(paid: u32, refund: u32) -> u32 {
    paid - refund
}

pub fn reversed(balance: i32) -> i32 {
    -balance
}

/// 2 to send anything, and 3 for each kilo.
pub fn postage(kilos: u32) -> u32 {
    kilos * 3 + 2
}

pub fn has_flag(flags: u8, flag: u8) -> bool {
    flags & flag != 0
}

/// `None` when the points would overflow.
pub fn add_points(points: u32, more: u32) -> Option<u32> {
    points.checked_add(more)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_of_each() {
        assert_eq!(line_total(30, 1), 30);
    }

    #[test]
    fn a_fee_is_added() {
        assert_eq!(with_fee(2, 2), 4);
    }

    #[test]
    fn a_bill_is_shared() {
        assert_eq!(per_person(4, 2), 2);
    }

    #[test]
    fn nothing_refunded() {
        assert_eq!(after_refund(10, 0), 10);
    }

    #[test]
    fn an_empty_balance_reversed() {
        assert_eq!(reversed(0), 0);
    }

    #[test]
    fn a_letter_costs_the_minimum() {
        assert_eq!(postage(0), 2);
    }

    #[test]
    fn a_flag_that_is_set() {
        assert!(has_flag(3, 1));
    }

    #[test]
    fn points_add_up() {
        assert_eq!(add_points(1, 2), Some(3));
    }
}
