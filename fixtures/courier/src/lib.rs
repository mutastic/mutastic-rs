//! A courier's round, for the docs' page on output: a run long enough to show what mutastic says
//! while it works. The one test takes a second and a half, as a slow integration test does, and a
//! mutant that makes the courier wait at a door takes six seconds more, well past what its test
//! took, though inside its time limit. With one worker each mutant takes its turn, so the run
//! shows the same steps every time.

pub mod handover;

/// The postage on a parcel of `grams`: 3 up to a kilo, 5 above, and 2 more for a fragile one.
pub fn postage(grams: u32, fragile: bool) -> u32 {
    let base = if grams > 1000 { 5 } else { 3 };
    if fragile {
        base + 2
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_round() {
        // The van takes a while to get round.
        std::thread::sleep(std::time::Duration::from_millis(1500));
        assert_eq!(postage(500, false), 3);
        assert_eq!(postage(2000, true), 7);
        assert_eq!(handover::hand_over(false), "left in the porch");
    }
}
