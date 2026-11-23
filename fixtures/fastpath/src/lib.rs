//! Reads a whole number written in decimal digits, a fast way or a safe way. The check that picks
//! between them is the only code its tests can't catch a change to: sending more input the safe way
//! gives the same answer.

/// The number `digits` spells, which has only the digits 0 to 9.
pub fn read(digits: &str) -> u64 {
    if digits.len() > 9 {
        return read_carefully(digits);
    }
    let mut value: u32 = 0;
    for digit in digits.bytes() {
        value = value * 10 + u32::from(digit - b'0');
    }
    u64::from(value)
}

/// At any length, in a `u64` that panics rather than overflows.
fn read_carefully(digits: &str) -> u64 {
    let mut value: u64 = 0;
    for digit in digits.bytes() {
        value = value
            .checked_mul(10)
            .and_then(|tens| tens.checked_add(u64::from(digit - b'0')))
            .expect("more digits than a u64 holds");
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_few_digits() {
        assert_eq!(read("7"), 7);
        assert_eq!(read("305"), 305);
    }

    #[test]
    fn reads_nine_digits() {
        assert_eq!(read("987654321"), 987_654_321);
    }

    #[test]
    fn reads_more_than_a_u32_holds() {
        assert_eq!(read("12345678901"), 12_345_678_901);
    }

    #[test]
    #[should_panic(expected = "more digits than a u64 holds")]
    fn refuses_more_than_a_u64_holds() {
        read("99999999999999999999");
    }
}
