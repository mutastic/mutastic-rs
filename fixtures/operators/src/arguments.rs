//! What a call is given: arguments, parameters, and the calls in between.

pub fn username(input: &str) -> String {
    input.trim().to_lowercase()
}

pub fn between(value: i32, low: i32, high: i32) -> bool {
    low <= value && value <= high
}

pub fn is_percent(value: i32) -> bool {
    between(value, 0, 100)
}

pub fn largest(values: &[u32]) -> u32 {
    values.iter().copied().max().unwrap_or(0)
}

pub fn best_of_three(a: u32, b: u32, c: u32) -> u32 {
    largest(&[a, b, c])
}

pub fn price_with(base: u32, coupon: Option<u32>) -> u32 {
    base - coupon.unwrap_or(0)
}

pub fn greet(name: String, excited: bool) -> String {
    if excited {
        format!("Hi {name}!")
    } else {
        format!("Hi {name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames_are_lowercase() {
        assert_eq!(username("Ann"), "ann");
    }

    #[test]
    fn too_much_is_not_a_percent() {
        assert!(!is_percent(150));
    }

    #[test]
    fn the_best_comes_first() {
        assert_eq!(best_of_three(9, 2, 1), 9);
    }

    #[test]
    fn full_price_without_a_coupon() {
        assert_eq!(price_with(10, None), 10);
    }

    #[test]
    fn a_calm_greeting() {
        assert_eq!(greet("Ann".into(), false), "Hi Ann");
    }
}
