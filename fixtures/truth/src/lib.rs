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

#[rustfmt::skip]
pub fn split_line(x: i32) -> bool {
    x * 2
        >= 4
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
        "{} {} {} {} {} {:?} {} {} {} {} {} {} {:?} {:?} {:?} {}",
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
        split_line(x),
    )
}

pub fn removes_calls(x: i32) -> Vec<i32> {
    let mut v = Vec::new();
    v.push(x);
    record(&mut v, x * 2);
    let mut m = std::collections::BTreeMap::new();
    m.insert(x, 1);
    let _ = m.insert(x + 1, 2);
    v.push(m.len() as i32);
    v
}

fn record(v: &mut Vec<i32>, value: i32) {
    if value > 4 {
        v.push(value);
    }
}

pub fn assigns(x: i32) -> i32 {
    let mut total = 1;
    total = total + x;
    let last;
    last = total * 2;
    total + last
}

pub fn strings(x: i32) -> String {
    let s = format!("ab{x}ba");
    let mut out = String::new();
    if s.starts_with("ab1") {
        out.push('s');
    }
    out.push_str(&s.to_uppercase());
    out.push_str(s.trim_start_matches('a'));
    if let Some(rest) = s.strip_prefix("ab") {
        out.push_str(rest);
    }
    out
}

pub fn maths(x: i32) -> i32 {
    let a = x.min(3);
    let b = std::cmp::max(x, -2);
    let c = (x as f64 / 2.0).floor() as i32;
    a * 100 + b * 10 + c
}

pub fn matches_any(x: i32) -> bool {
    [1, 2, x].iter().any(|v| *v > 2)
}

pub fn stages(x: i32) -> usize {
    (0..10)
        .filter(|v| v % 3 == 0)
        .skip(1)
        .take(x.clamp(0, 5) as usize)
        .count()
}

pub fn stage_in_for(x: i32) -> i32 {
    let mut total = 0;
    for v in [3, 1, 2].iter().rev().take(2) {
        total = total * 10 + v;
    }
    total + x
}

pub fn range_ends(x: i32) -> i32 {
    let mut total = 0;
    for i in 0..x.clamp(0, 4) {
        total += i;
    }
    let digits = [1, 2, 3, 4, 5];
    let part = &digits[..x.clamp(0, 4) as usize];
    if (1..=3).contains(&x) {
        total += 100;
    }
    total * 1000 + part.iter().sum::<i32>()
}

pub fn guards(x: i32) -> u8 {
    match x {
        v if is_even(v) => 1,
        _ => 0,
    }
}

fn is_even(v: i32) -> bool {
    v % 2 == 0
}

pub fn some_or_not(x: i32) -> i32 {
    let Some(y) = positive(x) else {
        return -1;
    };
    if let Some(z) = positive(y - 2) {
        z
    } else {
        y * 10
    }
}

pub fn wider_returns(x: i32) -> Result<u8, String> {
    if x > 7 {
        return Err("big".into());
    }
    Ok(x.unsigned_abs() as u8 + 1)
}

pub fn more(x: i32) -> String {
    format!(
        "{:?} {} {} {} {} {} {} {} {} {} {:?}",
        removes_calls(x),
        assigns(x),
        strings(x),
        maths(x),
        matches_any(x),
        stages(x),
        stage_in_for(x),
        range_ends(x),
        guards(x),
        some_or_not(x),
        wider_returns(x),
    )
}

pub fn comparisons(x: i32) -> u8 {
    let mut out = 0;
    if x == 3 {
        out += 1;
    }
    if x != -2 {
        out += 2;
    }
    if x < 4 {
        out += 4;
    }
    let v = vec![1, 2, 3];
    if v.len() > 0 {
        out += 8;
    }
    out
}

pub fn negations(x: i32) -> i32 {
    let y = -x;
    y * 3 - -4
}

#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub a: i32,
    pub b: i32,
}

pub fn fields(x: i32) -> i32 {
    let base = Point { a: 1, b: 2 };
    let p = Point {
        a: x,
        b: x * 2,
        ..base
    };
    p.a * 10 + p.b
}

pub fn optional(x: i32, limit: Option<i32>) -> i32 {
    let found: Option<i32> = Some(x * 2);
    let small = if found.is_some() { 1 } else { 0 };
    let cap = limit.unwrap_or(5);
    let d: i32 = positive(x).unwrap_or_default();
    let e = positive(x - 3).map_or(-1, |v| v * 2);
    let f = positive(x + 1).unwrap_or_else(|| 7);
    small + cap * 10 + d * 100 + e * 1000 + f
}

pub fn overflowing(x: i32) -> i32 {
    let a = (x as u8).checked_sub(3).unwrap_or(0) as i32;
    let b = (x as u8).saturating_sub(4) as i32;
    let c = x.wrapping_mul(3);
    a + b * 1000 + c * 100000
}

pub fn bits(x: i32) -> i32 {
    let mut flags = x & 6;
    flags |= 1;
    flags ^= x >> 1;
    (flags << 2) | (x ^ 3)
}

pub fn constants(x: i32) -> String {
    let label = if x > 2 { "big" } else { "" };
    let n = x + 100;
    let mut total = 0;
    total += n * 2;
    format!("{label}:{total}")
}

pub fn conditions(x: i32) -> u8 {
    let mut out = 0;
    if is_even(x) {
        out += 1;
    }
    if matches!(x, 1 | 3) {
        out += 2;
    }
    out
}

pub fn stronger(x: i32) -> String {
    format!(
        "{} {} {} {} {} {} {} {}",
        comparisons(x),
        negations(x),
        fields(x),
        conditions(x),
        bits(x),
        constants(x),
        overflowing(x),
        optional(x, positive(x)),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn probe() {
        for x in -6..10 {
            println!(
                "{x}: {} | {} | {}",
                super::all(x),
                super::more(x),
                super::stronger(x)
            );
        }
    }
}
