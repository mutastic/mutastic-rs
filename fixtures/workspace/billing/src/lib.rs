//! A workspace member, for the docs: mutastic runs on one package at a time.

pub fn due(total: u32, paid: u32) -> u32 {
    total.saturating_sub(paid)
}
