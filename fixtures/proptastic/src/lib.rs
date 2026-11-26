//! Functions checked only by proptastic's properties, which the tests reach through `testkit`, a
//! crate that wraps proptastic. A property draws its inputs from the seed mutastic gives it, so a
//! mutant only some inputs catch is caught in every run or in none.

/// The list in reverse order.
pub fn reversed(xs: &[i32]) -> Vec<i32> {
    let mut out = Vec::with_capacity(xs.len());
    for &x in xs.iter().rev() {
        out.push(x);
    }
    out
}

/// The list in ascending order, by insertion.
pub fn sorted(xs: &[i32]) -> Vec<i32> {
    let mut out: Vec<i32> = Vec::with_capacity(xs.len());
    for &x in xs {
        let mut at = out.len();
        while at > 0 && out[at - 1] > x {
            at -= 1;
        }
        out.insert(at, x);
    }
    out
}

/// Whether someone of `age` is an adult: 18 or older.
pub fn is_adult(age: i32) -> bool {
    age >= 18
}

#[cfg(test)]
mod props {
    use super::*;
    use testkit::{for_all, integers, lists};

    #[test]
    fn reverses_twice() {
        for_all(lists(integers::<i32>())).check(|xs| assert_eq!(reversed(&reversed(&xs)), xs));
    }

    #[test]
    fn sorts_as_std_does() {
        for_all(lists(integers::<i32>())).check(|xs| {
            let mut expected = xs.clone();
            expected.sort();
            assert_eq!(sorted(&xs), expected);
        });
    }

    #[test]
    fn adults_are_eighteen_or_over() {
        for_all(integers::<i32>().at_least(0).at_most(120))
            .check(|age| assert_eq!(is_adult(age), age >= 18));
    }
}
