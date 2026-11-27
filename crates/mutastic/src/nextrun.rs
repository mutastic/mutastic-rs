//! What the last `mutastic next` left uncaught, as `mutastic-core`'s
//! [`nextrun`](mutastic_core::nextrun) keeps it for every edition: here, the hash of a Rust
//! package's code under test.

pub use mutastic_core::nextrun::*;

use sha2::{Digest, Sha256};

use crate::sites::Crate;

/// A hash of the code under test: each file a run mutates, by its path, with its test code taken
/// out ([`crate::sites::without_test_code`]). A test written in `tests/` or in a `mod tests` leaves
/// it as it was, so the next `next` runs only what was left uncaught; a change to the code it
/// tests changes it. The run record's hash, of every file, tests included, would change with each
/// test written.
pub fn code_under_test(found: &Crate) -> String {
    let mut hasher = Sha256::new();
    let mut files: Vec<(String, &str)> = found
        .files
        .iter()
        .map(|file| (crate::workspace::slashed(&file.path), file.text.as_str()))
        .collect();
    files.sort();
    for (path, text) in files {
        let code = crate::sites::without_test_code(text);
        hasher.update(path.as_bytes());
        hasher.update([0u8]);
        hasher.update((code.len() as u64).to_le_bytes());
        hasher.update(code.as_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A test written into a file's `mod tests` leaves the code under test as it was; a change to
    /// the code it tests doesn't.
    #[test]
    fn a_new_test_leaves_the_code_under_test_as_it_was() {
        let found = |text: &str| Crate {
            files: vec![crate::sites::File {
                path: PathBuf::from("src/lib.rs"),
                text: text.to_string(),
                unparsed: Vec::new(),
            }],
            ..Crate::default()
        };
        let code = "pub fn big(n: u32) -> bool {\n    n >= 100\n}\n";
        let tested = format!(
            "{code}\n#[cfg(test)]\nmod tests {{\n    #[test]\n    fn t() {{ assert!(super::big(100)); }}\n}}\n"
        );
        let more = tested.replace("}\n}\n", "}\n    #[test]\n    fn u() {}\n}\n");
        assert_ne!(tested, more);
        let hash = code_under_test(&found(&tested));
        assert_eq!(hash, code_under_test(&found(&more)));
        assert_ne!(
            hash,
            code_under_test(&found(&tested.replace(">= 100", "> 100")))
        );
    }
}
