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

pub fn arithmetic(x: i32) -> i32 {
    let a = x + 3;
    let b = 20 - x;
    let mut c = 7;
    c += x;
    c -= 1;
    a * 100 + b * 10 + c
}

#[derive(Default)]
pub struct Builder {
    v: i32,
}

impl Builder {
    pub fn scale(self, by: i32) -> Self {
        Builder {
            v: (self.v + 1) * by,
        }
    }

    pub fn offset(mut self, by: i32) -> Self {
        self.v += by;
        self
    }

    pub fn value(&self) -> i32 {
        self.v
    }
}

fn shout(text: String) -> String {
    text.to_uppercase()
}

pub fn omissions(x: i32) -> String {
    let owned = format!("  {x} ");
    let s: &str = &owned;
    let t = s.trim();
    let n = (x - 3).abs();
    let b = Builder::default().scale(x).offset(1);
    let loud = shout(format!("v{x}"));
    format!("{t}|{n}|{}|{loud}", b.value())
}

fn between(x: i32, lo: i32, hi: i32) -> bool {
    lo <= x && x < hi
}

pub fn swaps(x: i32) -> bool {
    between(x, 0, 5)
}

pub fn searches(x: i32) -> String {
    let s = format!("a{x}b{x}c");
    let first = s.find(&x.to_string()).map_or(-1, |i| i as i32);
    let split = s.split_once('b').map(|(a, _)| a.len()).unwrap_or(0);
    format!("{first} {split}")
}

pub fn ordering(x: i32) -> Vec<i32> {
    let mut v = vec![3, x, 1];
    v.sort_by(|a, b| a.cmp(b));
    let mut w = vec![x, 2, 5];
    w.sort_by(|a, b| a.cmp(&b).reverse());
    v.extend(w);
    v
}

pub fn arms(x: i32) -> u8 {
    match x {
        0 => 10,
        1 | 2 => 20,
        n if n > 5 => 30,
        _ => 40,
    }
}

pub fn tries(x: i32) -> Option<i32> {
    let y = positive(x)?;
    let z = positive(y - 1)?;
    Some(z * 2)
}

pub fn rollback(x: i32) -> String {
    format!(
        "{} {} {} {} {} {:?} {} {:?}",
        arithmetic(x),
        omissions(x),
        swaps(x),
        searches(x),
        arms(x),
        tries(x),
        searches(x + 1),
        ordering(x),
    )
}

#[derive(Debug, PartialEq)]
pub enum Problem {
    Negative,
    TooBig,
}

pub fn checked(x: i32) -> Result<i32, Problem> {
    if x < 0 {
        return Err(Problem::Negative);
    }
    if x > 8 {
        return Err(Problem::TooBig);
    }
    Ok(x * 3)
}

#[derive(Debug, Default, PartialEq)]
pub struct Stats {
    pub count: i32,
}

pub fn stats(x: i32) -> Stats {
    Stats { count: x * 2 }
}

pub fn loops(x: i32) -> i32 {
    let mut total = 0;
    for i in 0..10 {
        if i == x {
            continue;
        }
        if i > 6 {
            break;
        }
        total += i;
    }
    total
}

pub fn defaults(x: i32, label: String) -> String {
    format!("{label}{x}")
}

pub fn pipelines(x: i32) -> i32 {
    let found = Some(x).filter(|v| *v > 2).unwrap_or(-1);
    let sum: i32 = (0..5).map(|v| v * x).sum();
    let seen: i32 = (0..3).inspect(|v| assert!(*v < 3)).sum();
    found + sum * 100 + seen * 10000
}

fn total_of(values: &[i32]) -> i32 {
    values.iter().sum()
}

fn count_of(values: Vec<i32>) -> usize {
    values.len()
}

pub fn varargs(x: i32) -> i32 {
    total_of(&[x, 2, 3]) + count_of(vec![x, 4, 5]) as i32 * 100
}

/// A stand-in for `regex::Regex`, by its path: what matters is the text it's given.
pub struct Regex(String);

impl Regex {
    pub fn new(pattern: &str) -> Result<Regex, String> {
        Ok(Regex(pattern.to_string()))
    }
}

/// A stand-in for `rust_decimal::Decimal`, by its method's name.
#[derive(Clone, Copy)]
pub struct Decimal(i64);

impl Decimal {
    pub fn round_dp(self, places: u32) -> Decimal {
        Decimal(self.0 / 10i64.pow(places) * 10i64.pow(places))
    }
}

pub fn patterns(x: i32) -> String {
    let re = Regex::new(r"^\d+$").unwrap();
    let price = Decimal(x as i64 * 137).round_dp(1);
    format!("{} {}", re.0.len() + re.0.matches('d').count() * 10, price.0)
}

pub fn extended(x: i32) -> String {
    format!(
        "{:?} {:?} {} {} {} {} {}",
        checked(x),
        stats(x),
        loops(x),
        defaults(x, "n".into()),
        pipelines(x),
        varargs(x),
        patterns(x),
    )
}

use std::num::NonZeroU32;

/// A zero check on a `NonZeroU32`, which can't hold 0: the guard is never true, so a mutant that
/// differs only when it is is equivalent.
pub fn per_part(total: u32, parts: NonZeroU32) -> u32 {
    if parts.get() == 0 {
        return 0;
    }
    total / parts.get()
}

/// The zero on the left, through a reference.
pub fn not_zero_flipped(n: &std::num::NonZeroU8) -> bool {
    0 != n.get()
}

/// A `let` with its type written, the generic form.
pub fn above_zero(x: i32) -> bool {
    let n: std::num::NonZero<u16> =
        std::num::NonZero::new(x.unsigned_abs() as u16).unwrap_or(std::num::NonZero::<u16>::MAX);
    n.get() > 0
}

pub struct Plan {
    parts: std::num::NonZeroU32,
}

impl Plan {
    /// A field.
    pub fn share(&self, total: u32) -> u32 {
        if self.parts.get() <= 0 {
            return total;
        }
        total / self.parts.get()
    }
}

/// Not a `NonZero*`: 0 is an input here.
pub fn per_part_u32(total: u32, parts: u32) -> u32 {
    if parts == 0 {
        return 0;
    }
    total / parts
}

/// A `NonZero*` compared with 1, which it can hold.
pub fn is_one(parts: std::num::NonZeroU32) -> bool {
    parts.get() == 1
}

/// A signed `NonZero*` below 0, which it can be.
pub fn below_zero(n: std::num::NonZeroI32) -> bool {
    n.get() < 0
}

pub fn nonzero(x: i32) -> String {
    let unsigned = std::num::NonZeroU32::new(x.unsigned_abs()).unwrap_or(std::num::NonZeroU32::MAX);
    let signed = std::num::NonZeroI32::new(x).unwrap_or(std::num::NonZeroI32::MIN);
    let small =
        std::num::NonZeroU8::new(x.unsigned_abs() as u8).unwrap_or(std::num::NonZeroU8::MAX);
    let plan = Plan { parts: unsigned };
    format!(
        "{} {} {} {} {} {} {}",
        per_part(100, unsigned),
        not_zero_flipped(&small),
        above_zero(x),
        plan.share(100),
        per_part_u32(100, x.unsigned_abs()),
        is_one(unsigned),
        below_zero(signed),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn probe() {
        for x in -6..10 {
            println!(
                "{x}: {} | {} | {} | {} | {} | {}",
                super::all(x),
                super::more(x),
                super::stronger(x),
                super::rollback(x),
                super::extended(x),
                super::nonzero(x)
            );
        }
    }
}
