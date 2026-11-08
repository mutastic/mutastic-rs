//! Code in and around `unsafe`, for the safety rules: what's left alone, what's marked, and what a
//! crash means in each.

pub mod plain;
pub mod raw;

/// Test code by its `cfg`, however it's combined.
#[cfg(all(test, unix))]
mod unix_tests {
    pub fn helper(x: u32) -> u32 {
        x + 1
    }

    #[test]
    fn helps() {
        assert_eq!(helper(1), 2);
    }
}

#[cfg(any(test, feature = "fuzzing"))]
pub fn test_helper(x: u32) -> u32 {
    x * 2
}
