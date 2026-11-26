//! A fixture, not proptastic: the surface of it mutastic depends on, so mutastic's public build
//! can check it without the library, which isn't published yet. Only the words these tests use:
//! `for_all`, `check`, `integers` (with `at_least` and `at_most`) and `lists`.
//!
//! - **Settings,** read from the environment as each property starts: `PROPTASTIC_SEED`, `0x` and
//!   hex digits only (a bare number is refused); `PROPTASTIC_TRIES`; `PROPTASTIC_SHRINK=false`
//!   reports the first counterexample; `PROPTASTIC_EVENTS=false` writes no event and
//!   `PROPTASTIC_REPLAY=false` saves no failure (both under `target/proptastic/`);
//!   `PROPTASTIC_CORPUS=off` leaves the corpus, `.proptastic/corpus/`, alone; and
//!   `PROPTASTIC_COLOR=always` colours the report's keys, `never` doesn't.
//! - **The random stream** is keyed on the test's name, never its module.
//! - **The failure** panics with the assertion's own message, a blank line, then the report, its
//!   keys right-aligned, stacked when a value spans lines or passes the width (`COLUMNS`, else 100).
//!
//! Unlike proptastic, it names its draws by their place (`a`), not as the closure does, and keeps
//! no corpus entries: it only writes each property's description there, as proptastic does.

use std::cell::Cell;
use std::fmt::Debug;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::sync::Once;

/// What produces values.
pub trait Domain {
    type Value: Clone + Debug;
    fn draw(&self, random: &mut Random) -> Self::Value;
    /// Values smaller than `value`, the smallest first.
    fn smaller(&self, value: &Self::Value) -> Vec<Self::Value>;
    /// `value` as Rust source, for the pin line.
    fn source(&self, value: &Self::Value) -> String;
}

/// A property's random stream: splitmix64.
pub struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number from 0 to `n`, both included.
    fn up_to(&mut self, n: u128) -> u128 {
        let wide = (u128::from(self.next()) << 64) | u128::from(self.next());
        if n == u128::MAX {
            wide
        } else {
            wide % (n + 1)
        }
    }
}

/// The integer types `integers` draws.
pub trait Integer: Copy + Debug + 'static {
    const LEAST: i128;
    const MOST: i128;
    fn from_wide(value: i128) -> Self;
    fn wide(self) -> i128;
}

macro_rules! integer {
    ($($t:ty),*) => {$(
        impl Integer for $t {
            const LEAST: i128 = <$t>::MIN as i128;
            const MOST: i128 = <$t>::MAX as i128;
            fn from_wide(value: i128) -> Self {
                value as $t
            }
            fn wide(self) -> i128 {
                self as i128
            }
        }
    )*};
}
integer!(i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

/// Integers between two inclusive bounds, shrunk toward the one nearest zero.
#[derive(Clone, Copy)]
pub struct Integers<T> {
    least: i128,
    most: i128,
    of: std::marker::PhantomData<T>,
}

/// Every value of `T`, unless bounded.
pub fn integers<T: Integer>() -> Integers<T> {
    Integers {
        least: T::LEAST,
        most: T::MOST,
        of: std::marker::PhantomData,
    }
}

impl<T: Integer> Integers<T> {
    /// `n` or more.
    pub fn at_least(self, n: T) -> Self {
        Self { least: n.wide(), ..self }
    }

    /// `n` or less.
    pub fn at_most(self, n: T) -> Self {
        Self { most: n.wide(), ..self }
    }
}

impl<T: Integer> Domain for Integers<T> {
    type Value = T;

    fn draw(&self, random: &mut Random) -> T {
        let span = (self.most - self.least) as u128;
        T::from_wide(self.least + random.up_to(span) as i128)
    }

    fn smaller(&self, value: &T) -> Vec<T> {
        let value = value.wide();
        let target = 0.clamp(self.least, self.most);
        let mut out = Vec::new();
        if value != target {
            out.push(target);
            let half = value - (value - target) / 2;
            if half != value && half != target {
                out.push(half);
            }
            let step = if value > target { value - 1 } else { value + 1 };
            if step != target && step != half {
                out.push(step);
            }
        }
        out.into_iter().map(T::from_wide).collect()
    }

    fn source(&self, value: &T) -> String {
        format!("{value:?}")
    }
}

/// Lists of up to 20 values, shrunk by taking values out, then by shrinking each.
pub struct Lists<D> {
    element: D,
}

/// Lists of `element`'s values.
pub fn lists<D: Domain>(element: D) -> Lists<D> {
    Lists { element }
}

impl<D: Domain> Domain for Lists<D> {
    type Value = Vec<D::Value>;

    fn draw(&self, random: &mut Random) -> Self::Value {
        let size = random.up_to(20) as usize;
        (0..size).map(|_| self.element.draw(random)).collect()
    }

    fn smaller(&self, value: &Self::Value) -> Vec<Self::Value> {
        let mut out = Vec::new();
        if !value.is_empty() {
            out.push(Vec::new());
        }
        for at in 0..value.len() {
            let mut without = value.clone();
            without.remove(at);
            out.push(without);
        }
        for (at, element) in value.iter().enumerate() {
            for smaller in self.element.smaller(element) {
                let mut changed = value.clone();
                changed[at] = smaller;
                out.push(changed);
            }
        }
        out
    }

    fn source(&self, value: &Self::Value) -> String {
        let each: Vec<String> = value.iter().map(|v| self.element.source(v)).collect();
        format!("vec![{}]", each.join(", "))
    }
}

/// A property over one domain, run by `check`.
pub struct Property<D> {
    domain: D,
}

/// A property over `domain`.
pub fn for_all<D: Domain>(domain: D) -> Property<D> {
    Property { domain }
}

thread_local! {
    /// Whether this thread is trying an input, when a panic is the property's to report.
    static TRYING: Cell<bool> = const { Cell::new(false) };
}

static QUIET: Once = Once::new();

/// A panic hook that says nothing of a try's panic, and hands every other to the one before.
fn quiet_tries() {
    QUIET.call_once(|| {
        let before = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if !TRYING.with(Cell::get) {
                before(info);
            }
        }));
    });
}

impl<D: Domain> Property<D> {
    /// Runs the property now: panics with the counterexample if `test` fails.
    pub fn check(self, test: impl Fn(D::Value)) {
        quiet_tries();
        let thread = std::thread::current();
        let name = thread.name().and_then(|n| n.rsplit("::").next()).unwrap_or("property");
        describe(name);
        let seed = seed();
        let mut random = Random(seed ^ key(name));
        let tries: u32 = setting("PROPTASTIC_TRIES").and_then(|t| t.parse().ok()).unwrap_or(100);
        for tried in 1..=tries {
            let value = self.domain.draw(&mut random);
            if let Some(message) = fails(&test, value.clone()) {
                let shrink = !off("PROPTASTIC_SHRINK");
                let (value, message, shrunk) = if shrink {
                    self.shrunk(&test, value, message)
                } else {
                    (value, message, false)
                };
                let input = format!("a = {value:?}");
                if !off("PROPTASTIC_REPLAY") {
                    write(&format!("target/proptastic/failures/{name}.txt"), &input);
                }
                event(name, "property-failed", seed);
                let tries = if tried == 1 { "try" } else { "tries" };
                let header = format!("{name} failed after {tried} {tries} (seed {seed:#x})");
                let facts = [
                    (if shrunk { "shrunk counterexample" } else { "counterexample" }, input),
                    ("pin it", format!(".example({})", self.domain.source(&value))),
                ];
                panic!("{message}\n\n{}", report(&header, &facts));
            }
        }
        event(name, "property-passed", seed);
    }

    /// The smallest failing value the candidates lead to, within a budget, its message, and
    /// whether it's smaller than the first.
    fn shrunk(
        &self,
        test: &impl Fn(D::Value),
        first: D::Value,
        message: String,
    ) -> (D::Value, String, bool) {
        let (mut value, mut message, mut shrunk) = (first, message, false);
        let mut runs = 0;
        'smaller: while runs < 1_000 {
            for candidate in self.domain.smaller(&value) {
                runs += 1;
                if let Some(said) = fails(test, candidate.clone()) {
                    (value, message, shrunk) = (candidate, said, true);
                    continue 'smaller;
                }
            }
            break;
        }
        (value, message, shrunk)
    }
}

/// What `test` panicked with on `value`, if it did.
fn fails<T>(test: &impl Fn(T), value: T) -> Option<String> {
    TRYING.with(|trying| trying.set(true));
    let result = panic::catch_unwind(AssertUnwindSafe(|| test(value)));
    TRYING.with(|trying| trying.set(false));
    let payload = result.err()?;
    Some(
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|said| said.to_string()))
            .unwrap_or_else(|| "the test panicked".to_string()),
    )
}

/// The report: the header, then each fact, keys right-aligned with the colon after, so the
/// counterexample's, the longest, is two spaces in; stacked when a value spans lines or a line
/// passes the width.
fn report(header: &str, facts: &[(&str, String)]) -> String {
    let key = facts.iter().map(|(key, _)| key.len()).max().unwrap_or(0);
    let width = setting("COLUMNS").and_then(|c| c.parse::<usize>().ok()).unwrap_or(100).max(40);
    let stacked = facts
        .iter()
        .any(|(_, value)| value.contains('\n') || 2 + key + 2 + value.len() > width);
    let colour = setting("PROPTASTIC_COLOR").as_deref() == Some("always");
    let mut out = header.to_string();
    for (name, value) in facts {
        let label = if colour {
            format!("\x1b[2m{name}:\x1b[0m")
        } else {
            format!("{name}:")
        };
        if stacked {
            out.push_str(&format!("\n  {label}"));
            for line in value.lines() {
                out.push_str(&format!("\n    {line}"));
            }
        } else {
            let pad = " ".repeat(key - name.len());
            out.push_str(&format!("\n  {pad}{label} {value}"));
        }
    }
    out
}

/// The seed: `PROPTASTIC_SEED` as `0x` and hex digits, else a fresh one.
fn seed() -> u64 {
    let Some(said) = setting("PROPTASTIC_SEED") else {
        return std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos() as u64);
    };
    said.strip_prefix("0x")
        .filter(|digits| !digits.is_empty() && digits.len() <= 16)
        .and_then(|digits| u64::from_str_radix(digits, 16).ok())
        .unwrap_or_else(|| {
            panic!(
                "proptastic: PROPTASTIC_SEED is {said}, which isn't a seed. Seeds are 0x and hex \
                 digits, as failures print them."
            )
        })
}

/// The random stream's key: the test's name alone, FNV-1a.
fn key(name: &str) -> u64 {
    name.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn setting(name: &str) -> Option<String> {
    std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Whether a switch is off: `false`, `0`, `off` or `no`.
fn off(name: &str) -> bool {
    matches!(setting(name).as_deref(), Some("false" | "0" | "off" | "no"))
}

/// The property's corpus folder and its description, as proptastic writes them on each run,
/// unless `PROPTASTIC_CORPUS=off`.
fn describe(name: &str) {
    let corpus = setting("PROPTASTIC_CORPUS").unwrap_or_else(|| ".proptastic/corpus".to_string());
    if corpus != "off" {
        write(&format!("{corpus}/{name}/property.txt"), &format!("key={name}"));
    }
}

/// One event per property, its last, unless `PROPTASTIC_EVENTS=false`.
fn event(name: &str, reason: &str, seed: u64) {
    if !off("PROPTASTIC_EVENTS") {
        let event = format!("{{\"reason\":\"{reason}\",\"seed\":\"{seed:#x}\"}}");
        write(&format!("target/proptastic/events/{name}.json"), &event);
    }
}

/// Writes `text` to `file`; a file that can't be written never fails the test.
fn write(file: &str, text: &str) {
    let file = Path::new(file);
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(file, format!("{text}\n"));
}
