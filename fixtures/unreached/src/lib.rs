//! Code no test reaches, for the assertion mutastic offers to start each function's first test
//! with, and the code it offers none for.

/// What a parcel costs to send: 4 up to a kilo, then 2 more for each kilo started.
pub fn postage(grams: u32) -> u32 {
    if grams <= 1000 {
        return 4;
    }
    4 + grams.div_ceil(1000) * 2 - 2
}

/// Whether a parcel is too heavy to send.
pub fn too_heavy(grams: u32) -> bool {
    grams > 30_000
}

/// The crate's own: only `label` gives it its input, so a test of it alone might test what can't
/// happen. mutastic offers no assertion for it.
fn zone(postcode: u32) -> u32 {
    postcode / 10_000 + 1
}

/// Takes text, which mutastic can't make up: no assertion.
pub fn label(postcode: &str) -> String {
    format!("zone {}", zone(postcode.parse().unwrap_or(0)))
}

pub mod loyalty {
    /// Points earned and spent.
    pub struct Card {
        points: u32,
    }

    impl Card {
        pub fn new(points: u32) -> Self {
            Card { points }
        }

        /// What the card's points take off an order: one each, never more than the order.
        pub fn discount(&self, order: u32) -> u32 {
            if self.points > order {
                return order;
            }
            self.points
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn points_buy_a_discount() {
            assert_eq!(Card::new(3).discount(50), 3);
        }
    }
}
