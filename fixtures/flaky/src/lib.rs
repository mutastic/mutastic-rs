//! A raffle: numbered tickets, and a draw from the clock.

use std::time::{SystemTime, UNIX_EPOCH};

/// The number on the `n`th ticket sold.
pub fn ticket(n: u32) -> u32 {
    n * 10 + 1
}

/// The winning place among `count` tickets, from 1, by the clock.
pub fn draw(count: u32) -> u32 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.subsec_nanos());
    nanos % count + 1
}
