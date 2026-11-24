//! A package with a crate's name from crates.io, tested against that crate: Cargo has two
//! packages named `itoa` to build, and mutastic must say which is the one under test.

/// How many decimal digits `n` has.
pub fn digits(n: u32) -> usize {
    if n < 10 {
        1
    } else {
        1 + digits(n / 10)
    }
}

#[cfg(test)]
mod tests {
    use super::digits;

    #[test]
    fn as_many_digits_as_itoa_writes() {
        for n in [0, 9, 10, 99, 100, 12_345, u32::MAX] {
            assert_eq!(
                digits(n),
                itoa_registry::Buffer::new().format(n).len(),
                "{n}"
            );
        }
    }
}
