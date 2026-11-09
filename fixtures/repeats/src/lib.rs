//! Four identical lines in one function: each has mutants of its own, with ids of their own.

/// Four steps up from `start`, one line each, with where each step got to put in `seen`.
pub fn four_steps(start: i32, seen: &mut Vec<i32>) -> i32 {
    let mut i = start;
    i = i + 1;
    seen.push(i);
    i = i + 1;
    seen.push(i);
    i = i + 1;
    seen.push(i);
    i = i + 1;
    seen.push(i);
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_goes_up() {
        let mut seen = Vec::new();
        assert!(four_steps(10, &mut seen) > 0); // too weak to tell one step from another
    }
}
