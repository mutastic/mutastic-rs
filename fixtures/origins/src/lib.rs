//! Where a catch fails: `totals` checks three functions with a panic of its own, never an
//! assertion, so each catch's failure is in the test's code, and the run notes the test that
//! catches everything alike. `halves` checks with `assert_eq!`; `last` panics in the code.

pub fn double(n: u32) -> u32 {
    n * 2
}

pub fn triple(n: u32) -> u32 {
    n * 3
}

pub fn square(n: u32) -> u32 {
    n * n
}

pub fn half(n: u32) -> u32 {
    n / 2
}

/// The last value: an index past the end panics here, in the code.
pub fn last(values: &[u32]) -> u32 {
    values[values.len() - 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totals() {
        let found = [double(3), triple(3), square(3)];
        if found != [6, 9, 9] {
            let none: Option<u32> = None;
            none.unwrap();
        }
    }

    #[test]
    fn halves() {
        assert_eq!(half(8), 4);
    }

    #[test]
    fn reads() {
        assert_eq!(last(&[1, 2, 3]), 3);
    }
}
