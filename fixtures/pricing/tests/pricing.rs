mod common;

use pricing::Pricing;

// Only far from the edge: `>=` turning into `>` goes unnoticed. A helper from another test module,
// as test data often is: mutastic must find tests in `tests/` as well as `mod tests`.
#[test]
fn big_orders_get_ten_percent_off() {
    assert_eq!(Pricing::new().discount(common::large()), 25);
}
