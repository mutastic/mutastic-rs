//! The coverage page's example, ported from the Java edition's `samples/checkout`: tests that
//! reach every line and both sides of the `if`, whether or not they check anything.

/// 10% off orders of 100 or more.
pub fn discount(total: u32) -> u32 {
    if total >= 100 {
        return total / 10;
    }
    0
}

/// What `quantity` items at `price` each cost, less any discount, which goes on the receipt.
pub fn pay(quantity: u32, price: u32, receipt: &mut Vec<String>) -> u32 {
    let total = quantity * price;
    let off = discount(total);
    receipt.push(format!("discount {off}"));
    total - off
}
