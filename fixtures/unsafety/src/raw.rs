//! A module with unsafe code: every function here is near it.

/// Called from inside an `unsafe` block: mutated, and marked.
pub fn positive(x: i32) -> bool {
    x > 0
}

/// Holds an `unsafe` block: left alone.
pub fn halve(x: i32) -> i32 {
    unsafe { std::hint::assert_unchecked(positive(x)) };
    x / 2
}

/// Unsafe itself: left alone.
///
/// # Safety
/// `values` isn't empty.
pub unsafe fn first(values: &[u8]) -> u8 {
    *values.get_unchecked(0) + 1
}

/// Safe, beside unsafe code: mutated, and marked. One of its mutants aborts.
pub fn checked(x: i32) -> i32 {
    if x < 0 {
        std::process::abort();
    }
    x
}
