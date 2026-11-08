//! The suggestions sample, ported from the Java edition's `samples/suggestions`: each test leaves
//! a gap a suggested test can close.

pub mod rates {
    /// Late fees: none for the first 7 days, then 5 plus 2 per day after.
    pub fn late_fee(days_late: i64) -> i64 {
        if days_late <= 7 {
            return 0;
        }
        5 + (days_late - 7) * 2
    }

    /// Whether a balance is in credit: zero isn't.
    pub fn in_credit(balance: i32) -> bool {
        balance > 0
    }

    /// A small wobble on a price, for load testing: no test can pin its answer.
    pub fn jitter(price: i32) -> i32 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.subsec_nanos())
            .unwrap_or(0);
        price + (nanos % 5) as i32
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn late_fees_accrue_after_a_week() {
            assert_eq!(late_fee(2), 0);
            assert_eq!(late_fee(20), 31); // nothing checks day 7 or 8
        }

        #[test]
        fn a_positive_balance_is_in_credit() {
            assert!(in_credit(50)); // nothing checks zero
        }

        #[test]
        fn jitter_is_a_price() {
            assert!(jitter(100) > 0); // too weak to tell `+` from `-`
        }
    }
}

pub mod window {
    /// An opening window in days: from `start` (`None` when it's always been open) up to `end`.
    pub struct Window {
        pub start: Option<i32>,
        pub end: i64,
    }

    impl Window {
        pub fn open_on(&self, day: i32) -> bool {
            (self.start.is_none() || self.start.is_some_and(|start| start <= day))
                && i64::from(day) < self.end
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn open_in_the_middle() {
            let window = Window {
                start: Some(1),
                end: 10,
            };
            assert!(window.open_on(5)); // nothing checks the first or last day
        }
    }
}
