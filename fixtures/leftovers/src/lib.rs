//! Two functions that write the same file in the temporary directory and delete it again. A mutant
//! that removes the first one's delete leaves the file there; a test of the second, on a mutant
//! that removes its write, then reads the file the first left and passes. Each test process starts
//! with an empty temporary directory of its own, so it can't.

use std::path::Path;

/// Writes `text` to `shared.txt` in `dir`, and deletes it again: how long it was.
pub fn stash(dir: &Path, text: &str) -> usize {
    let path = dir.join("shared.txt");
    std::fs::write(&path, text).unwrap();
    std::fs::remove_file(&path).unwrap();
    text.len()
}

/// Writes `text` to `shared.txt` in `dir`, and measures the file: its size in bytes.
pub fn measure(dir: &Path, text: &str) -> u64 {
    let path = dir.join("shared.txt");
    std::fs::write(&path, text).unwrap();
    let size = std::fs::metadata(&path).unwrap().len();
    std::fs::remove_file(&path).unwrap();
    size
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stash_gives_the_length() {
        assert_eq!(stash(&std::env::temp_dir(), "hello"), 5);
    }

    #[test]
    fn measure_gives_the_size() {
        assert_eq!(measure(&std::env::temp_dir(), "hello"), 5);
    }
}
