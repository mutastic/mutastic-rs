//! Code with arid parts, and tests that check what it does, with one that checks a `Debug`'s text
//! too: an audit of the arid rules (`MUTASTIC_ARID=audit`) runs what they skip, and says where a
//! test checks it.

use std::fmt;
use std::time::Duration;

/// An order and its lines.
pub struct Order {
    pub id: u32,
    pub lines: Vec<u32>,
}

impl fmt::Debug for Order {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Order {}", self.id)
    }
}

/// Each line doubled, in a list sized for them.
pub fn doubled(order: &Order) -> Vec<u32> {
    let mut out = Vec::with_capacity(order.lines.len() * 2);
    for line in &order.lines {
        out.push(line * 2);
    }
    out
}

/// The first line, or an error saying there's none.
pub fn first(lines: &[u32]) -> Result<u32, String> {
    match lines.first() {
        Some(line) => Ok(*line),
        None => Err("an order with no lines".into()),
    }
}

/// Gives a busy service a moment before the next try.
pub fn settle(attempt: u64) {
    std::thread::sleep(Duration::from_millis(attempt * 2));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order() -> Order {
        Order {
            id: 7,
            lines: vec![1, 2],
        }
    }

    #[test]
    fn doubles_each_line() {
        assert_eq!(doubled(&order()), vec![2, 4]);
    }

    #[test]
    fn the_first_line_or_an_error() {
        assert_eq!(first(&[5, 6]), Ok(5));
        assert!(first(&[]).is_err());
    }

    #[test]
    fn settles() {
        settle(1);
    }

    #[test]
    fn debug_names_the_order() {
        assert_eq!(format!("{:?}", order()), "Order 7");
    }
}
