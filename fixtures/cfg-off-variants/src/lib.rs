//! Code under `cfg`s that only the build can judge: the build script sets `from_build`, and
//! nothing sets the others. Only the code the build compiled is mutated.

/// Twice the count, as the build script's `cfg` has it: built.
#[cfg(from_build)]
pub fn doubled(count: u32) -> u32 {
    count * 2
}

/// Twice the count, without the build script's `cfg`: not built.
#[cfg(not(from_build))]
pub fn doubled(count: u32) -> u32 {
    count + count
}

/// What docs.rs builds: not built.
#[cfg(docsrs)]
pub fn documented(count: u32) -> u32 {
    count * 3
}

/// What a loom test build has: not built.
#[cfg(loom)]
pub fn modelled(count: u32) -> u32 {
    count * 4
}

/// What Miri runs: not built.
#[cfg(miri)]
pub fn interpreted(count: u32) -> u32 {
    count * 5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles() {
        assert_eq!(doubled(3), 6);
    }
}
