//! The write loop `timeout-sum` has, reached by one fast test: a mutant that makes `chunk` give 0
//! spins until the shortest time limit there is.

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
}
