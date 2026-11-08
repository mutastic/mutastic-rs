//! Three missed mutants, found in the opposite order to the one they're ranked in: arithmetic that
//! three tests run, a condition one test runs, and a condition three tests run. A condition comes
//! before arithmetic whatever reaches it, and of two conditions the one more tests run comes first.

pub fn doubled(n: u32) -> u32 {
    let d = n * 2;
    d
}

pub fn is_big(n: u32) -> bool {
    n >= 100
}

pub fn is_small(n: u32) -> bool {
    n < 10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_doubled_is_more_than_one_and_small() {
        assert!(doubled(4) > 1);
        assert!(is_small(4));
    }

    #[test]
    fn five_doubled_is_more_than_one_and_small() {
        assert!(doubled(5) > 1);
        assert!(is_small(5));
    }

    #[test]
    fn six_doubled_is_more_than_one_and_small() {
        assert!(doubled(6) > 1);
        assert!(is_small(6));
    }

    #[test]
    fn five_isnt_big() {
        assert!(!is_big(5));
    }
}
