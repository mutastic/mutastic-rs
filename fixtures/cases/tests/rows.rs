//! test-case's rows, from outside the crate: each row is a test of its own.

use cases::{add, multiply};
use test_case::test_case;

#[test_case(100, 0, 100 ; "a hundred and nothing")]
fn adds(a: i64, b: i64, sum: i64) {
    assert_eq!(add(a, b), sum);
}

#[test_case(1, 1, 1 ; "one by one")]
#[test_case(2, 1, 2 ; "two by one")]
fn multiplies(a: i64, b: i64, product: i64) {
    assert_eq!(multiply(a, b), product);
}
