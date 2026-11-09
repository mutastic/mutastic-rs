//! A pattern that binds by reference to a place: `if let Some(ref mut start) = self.start` changes
//! the field itself, as url's `restore_after_path` does. Written into a `match`, the field would be
//! a value, a copy of it, and the change would be lost with no mutant live.

pub struct Span {
    start: Option<u32>,
    end: Option<u32>,
}

impl Span {
    pub fn new(start: u32, end: u32) -> Self {
        Span {
            start: Some(start),
            end: Some(end),
        }
    }

    pub fn shift(&mut self, by: u32) {
        if let Some(ref mut start) = self.start {
            *start += by;
        }
        let Some(ref mut end) = self.end else {
            return;
        };
        *end += by;
    }

    pub fn bounds(&self) -> (Option<u32>, Option<u32>) {
        (self.start, self.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shifting_moves_both_ends() {
        let mut span = Span::new(2, 5);
        span.shift(3);
        assert_eq!(span.bounds(), (Some(5), Some(8)));
    }
}
