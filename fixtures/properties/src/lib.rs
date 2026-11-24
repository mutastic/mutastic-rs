//! Functions checked only by property tests: proptest's in `props`, quickcheck's in `checks`. A
//! property draws its inputs at random, so a mutant only a few inputs catch, as `is_adult`'s
//! boundary is caught only at 18, is caught or missed by the draw.

/// The list in reverse order.
pub fn reversed(xs: &[i32]) -> Vec<i32> {
    let mut out = Vec::with_capacity(xs.len());
    for &x in xs.iter().rev() {
        out.push(x);
    }
    out
}

/// The list in ascending order, by insertion.
pub fn sorted(xs: &[i32]) -> Vec<i32> {
    let mut out: Vec<i32> = Vec::with_capacity(xs.len());
    for &x in xs {
        let mut at = out.len();
        while at > 0 && out[at - 1] > x {
            at -= 1;
        }
        out.insert(at, x);
    }
    out
}

/// Whether someone of `age` is an adult: 18 or older.
pub fn is_adult(age: u8) -> bool {
    age >= 18
}

/// Twice `x`, widened so it can't overflow.
pub fn double(x: u8) -> u16 {
    x as u16 * 2
}

#[cfg(test)]
mod props {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn reverses_twice(xs in proptest::collection::vec(any::<i32>(), 0..50)) {
            prop_assert_eq!(reversed(&reversed(&xs)), xs);
        }

        #[test]
        fn sorts_in_order(xs in proptest::collection::vec(-1000i32..1000, 0..200)) {
            let out = sorted(&xs);
            prop_assert_eq!(out.len(), xs.len());
            prop_assert!(out.windows(2).all(|pair| pair[0] <= pair[1]));
        }
    }

    proptest! {
        // Few cases over many ages: whether one is 18 is the draw's.
        #![proptest_config(ProptestConfig::with_cases(12))]

        #[test]
        fn adults_are_18_or_older(age in 0u8..60) {
            prop_assert_eq!(is_adult(age), age > 17);
        }
    }
}

#[cfg(test)]
mod checks {
    use super::*;
    use quickcheck_macros::quickcheck;

    #[quickcheck]
    fn doubles(x: u8) -> bool {
        double(x) == x as u16 + x as u16
    }
}
