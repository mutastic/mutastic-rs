//! A loop counter, for the docs' troubleshooting: some of its mutants make the loop never end, and
//! each of those runs until its time is up.

pub fn digits(mut n: u32) -> u32 {
    let mut count = 1;
    while n >= 10 {
        n /= 10;
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_digits() {
        assert_eq!(digits(42), 2);
        assert_eq!(digits(7), 1);
    }
}
