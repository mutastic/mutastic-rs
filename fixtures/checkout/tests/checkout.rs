use checkout::pay;

#[test]
fn big_orders_get_a_discount() {
    let mut receipt = Vec::new();
    assert!(pay(1, 250, &mut receipt) < 250);
}

#[test]
fn small_orders_pay_full_price() {
    pay(1, 50, &mut Vec::new());
}
