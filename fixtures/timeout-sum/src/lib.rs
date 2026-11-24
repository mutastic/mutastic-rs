//! A write loop that many slow tests reach: a mutant that makes `chunk` give 0 makes the loop never
//! end, in the fast test as in the slow ones. Its time limit is its tests' own time, each counted
//! once and side by side, so it's timed out about as soon as one test's limit, not after all
//! twenty-one added up.

/// How many chunks it takes to write `n` bytes, at most 64 at a time.
pub fn write_all(n: usize) -> usize {
    let mut written = 0;
    let mut chunks = 0;
    while written < n {
        written += chunk(n - written);
        chunks += 1;
    }
    chunks
}

/// What one write takes of the `left` bytes.
fn chunk(left: usize) -> usize {
    left.min(64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_in_chunks_of_64() {
        assert_eq!(write_all(200), 4);
    }

    /// Twenty tests that each take a second before they write.
    macro_rules! slow {
        ($($name:ident),*) => {
            $(
                #[test]
                fn $name() {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    assert_eq!(write_all(64), 1);
                }
            )*
        };
    }

    slow!(
        slow_01, slow_02, slow_03, slow_04, slow_05, slow_06, slow_07, slow_08, slow_09, slow_10,
        slow_11, slow_12, slow_13, slow_14, slow_15, slow_16, slow_17, slow_18, slow_19, slow_20
    );
}
