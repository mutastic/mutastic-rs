//! A module with no unsafe code.

/// One of its mutants aborts the process: caught, but by the crash.
pub fn guarded(x: i32) -> i32 {
    if x < 0 {
        std::process::abort();
    }
    x
}

/// `-` underflows on the tests' inputs: caught by the language's own panic.
pub fn total(a: u32, b: u32) -> u32 {
    a + b
}
