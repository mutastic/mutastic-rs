//! Mutant ids, made as the Java edition makes them: the first hex digits of a SHA-256 over what
//! the mutant is, with no line number in it, so an id stays the same while code around it moves.

use sha2::{Digest, Sha256};

/// The id (`m` and 8 hex digits) and the fingerprint (16 hex digits) of the parts given.
pub fn of(parts: &[&str]) -> (String, String) {
    let mut hasher = Sha256::new();
    hasher.update(b"rust");
    for part in parts {
        hasher.update([0u8]);
        hasher.update(part.as_bytes());
    }
    let hex: String = hasher
        .finalize()
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    (format!("m{}", &hex[..8]), hex)
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_id_is_m_and_eight_hex_digits_of_its_fingerprint() {
        let (id, fingerprint) = super::of(&["a", "b"]);
        assert_eq!(id.len(), 9);
        assert_eq!(fingerprint.len(), 16);
        assert_eq!(&id[1..], &fingerprint[..8]);
        assert_ne!(super::of(&["ab", ""]).0, super::of(&["a", "b"]).0);
    }
}
