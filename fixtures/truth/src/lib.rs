//! Every shape a mutated expression can take, for the truth test: each card's claim is written
//! into this source and must behave as the mutant does. Keep each function about one shape.

pub fn in_if(x: i32) -> i32 {
    if x >= 3 {
        1
    } else {
        2
    }
}

pub fn in_else_if(x: i32) -> i32 {
    if x < 0 {
        -1
    } else if x == 0 {
        0
    } else {
        1
    }
}

pub fn in_while(x: i32) -> i32 {
    let mut n = 0;
    while n + x < 3 {
        n += 1;
    }
    n
}

pub fn in_match_guard(x: i32) -> u8 {
    match x {
        v if v > 2 => 1,
        v if v <= -2 => 2,
        _ => 0,
    }
}

pub fn in_closure(x: i32) -> bool {
    let small = |v: i32| v <= 2;
    small(x)
}

pub fn with_try(x: i32) -> Option<i32> {
    let y = positive(x)?;
    Some(y * 2 - 1)
}

fn positive(x: i32) -> Option<i32> {
    if x > 0 {
        Some(x)
    } else {
        None
    }
}

pub fn connector(x: i32) -> bool {
    x > 1 && x != 4 || x == -3
}

pub fn not_condition(x: i32) -> i32 {
    if !(x % 2 == 0) {
        x / 2
    } else {
        x % 3
    }
}

pub fn compound(x: i32) -> i32 {
    let mut total = 10;
    total += x;
    total *= 2;
    total -= x % 3;
    total
}

pub fn precedence(x: i32) -> i32 {
    2 * x + 3 - x / 2
}

pub fn returns_early(x: i32) -> u32 {
    if x > 5 {
        return 7;
    }
    0
}

pub fn returns_bool(x: i32) -> bool {
    x.is_positive()
}

pub fn returns_option(x: i32) -> Option<u8> {
    match x {
        1 => Some(1),
        _ => None,
    }
}

pub fn returns_string(x: i32) -> String {
    format!("{x}")
}

pub fn returns_vec(x: i32) -> Vec<i32> {
    vec![x; 2]
}

pub fn all(x: i32) -> String {
    format!(
        "{} {} {} {} {} {:?} {} {} {} {} {} {} {:?} {:?} {:?}",
        in_if(x),
        in_else_if(x),
        in_while(x.clamp(-5, 5)),
        in_match_guard(x),
        in_closure(x),
        with_try(x),
        connector(x),
        not_condition(x),
        compound(x),
        precedence(x),
        returns_early(x),
        returns_bool(x),
        returns_option(x),
        returns_string(x),
        returns_vec(x),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn probe() {
        for x in -6..10 {
            println!("{x}: {}", super::all(x));
        }
    }
}
