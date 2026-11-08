//! The pricing sample, ported from the Java edition's `samples/pricing`, so the two editions'
//! cards can be read side by side.

pub struct Pricing {
    free_shipping_from: u32,
}

impl Default for Pricing {
    fn default() -> Self {
        Pricing::new()
    }
}

impl Pricing {
    pub fn new() -> Self {
        Pricing::with_free_shipping_from(50)
    }

    /// A test seam: tests set the free-shipping threshold.
    pub fn with_free_shipping_from(free_shipping_from: u32) -> Self {
        Pricing { free_shipping_from }
    }

    /// 10% off orders of 100 or more.
    pub fn discount(&self, total: u32) -> u32 {
        if total >= 100 {
            return total / 10;
        }
        0
    }

    /// Free from the threshold up.
    pub fn shipping(&self, total: u32) -> u32 {
        if total < self.free_shipping_from {
            return 5;
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_orders_pay_full_price() {
        let small: u32 = include_str!("../tests/orders.txt").trim().parse().unwrap();
        assert_eq!(Pricing::new().discount(small), 0);
    }

    // Through the test seam, and right at the edge.
    #[test]
    fn shipping_is_free_from_the_threshold() {
        let pricing = Pricing::with_free_shipping_from(40);
        assert_eq!(pricing.shipping(40), 0);
        assert_eq!(pricing.shipping(39), 5);
    }
}
