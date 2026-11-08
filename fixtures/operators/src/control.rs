//! `match` arms and loops.

pub fn delivery_days(zone: u8) -> u32 {
    match zone {
        1 => 1,
        2 => 3,
        _ => 7,
    }
}

pub fn first_negative(values: &[i32]) -> Option<usize> {
    let mut found = None;
    for (index, value) in values.iter().enumerate() {
        if *value < 0 {
            found = Some(index);
            break;
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_and_far() {
        assert_eq!(delivery_days(1), 1);
        assert_eq!(delivery_days(9), 7);
    }

    #[test]
    fn every_zone_within_a_week() {
        for zone in 1..=3 {
            assert!(delivery_days(zone) <= 7);
        }
    }

    #[test]
    fn finds_a_negative() {
        assert_eq!(first_negative(&[4, -1, 2]), Some(1));
    }
}
