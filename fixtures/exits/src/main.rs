//! A tool whose tests call what `main` calls, in the tests' own process, where a mutant can make it
//! exit.

use std::process;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    println!("{}", answer(&args));
}

/// What the tool prints for `args`: the discount on a total. With no total it says how to use it
/// and exits 2; with `--help` it says so and exits 0.
fn answer(args: &[String]) -> String {
    if args.is_empty() {
        eprintln!("usage: exits <total>");
        process::exit(2);
    }
    if args[0] == "--help" {
        println!("prints the discount on a total");
        process::exit(0);
    }
    discount(args[0].parse().unwrap()).to_string()
}

/// 10% off orders of 100 or more.
fn discount(total: u32) -> u32 {
    if total >= 100 {
        total / 10
    } else {
        0
    }
}

/// How many digits `total` has.
fn digits(mut total: u32) -> u32 {
    let mut count = 1;
    while total >= 10 {
        total /= 10;
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_the_discount() {
        assert_eq!(answer(&["120".to_string()]), "12");
    }

    #[test]
    fn counts_digits() {
        assert_eq!(digits(120), 3);
    }
}
