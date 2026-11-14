//! Sums checked by tables of cases: rstest's `#[case]` and `#[values]`, a loop over rows and a
//! property test here, test-case's rows in `tests/rows.rs`. Most rows are weak, as rows written
//! for convenience are: adding 0, multiplying by 1. One case sits on the discount's edge.

pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

pub fn subtract(a: i64, b: i64) -> i64 {
    a - b
}

pub fn multiply(a: i64, b: i64) -> i64 {
    a * b
}

/// 10% off orders of 100 or more.
pub fn discount(total: i64) -> i64 {
    if total >= 100 {
        return total / 10;
    }
    0
}

#[cfg(test)]
mod rstest_tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(100, 0, 100)]
    fn adds(#[case] a: i64, #[case] b: i64, #[case] sum: i64) {
        assert_eq!(add(a, b), sum);
    }

    #[rstest]
    #[case(30, 0, 30)]
    fn subtracts(#[case] a: i64, #[case] b: i64, #[case] difference: i64) {
        assert_eq!(subtract(a, b), difference);
    }

    #[rstest]
    #[case(1, 1, 1)]
    #[case(2, 1, 2)]
    fn multiplies(#[case] a: i64, #[case] b: i64, #[case] product: i64) {
        assert_eq!(multiply(a, b), product);
    }

    // Named cases, one on the edge: the one strong row.
    #[rstest]
    #[case::just_under(99, 0)]
    #[case::at_the_edge(100, 10)]
    fn discounts(#[case] total: i64, #[case] off: i64) {
        assert_eq!(discount(total), off);
    }

    // A matrix: a test for each pair. No 0, so dividing can't crash and pass for a check.
    #[rstest]
    fn multiplying_by_one_keeps_it(#[values(2, 3)] a: i64, #[values(1)] b: i64) {
        assert_eq!(multiply(a, b), a);
    }
}

#[cfg(test)]
mod table_tests {
    use super::*;

    // One test however many rows: the message names the row that failed.
    #[test]
    fn subtracting_nothing() {
        for (a, b, difference) in [(30, 0, 30), (7, 0, 7)] {
            assert_eq!(subtract(a, b), difference, "subtract({a}, {b})");
        }
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn adding_zero_keeps_it(a in -1000i64..1000) {
            prop_assert_eq!(add(a, 0), a);
        }
    }
}
