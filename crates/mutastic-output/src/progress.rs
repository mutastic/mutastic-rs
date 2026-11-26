//! What a live progress line is drawn with: its bar, the width a terminal gives it and the line
//! cut to fit, so none wraps; and how long a log may go without a line before a heartbeat.

use std::time::Duration;

use crate::glyphs::{FILLED, RULE};
use crate::paint::{paint, DIM, GREEN};

/// How long a log may go without a line, unless it's told otherwise.
pub const HEARTBEAT: Duration = Duration::from_secs(30);

/// `value` as a number of seconds above 0, as a heartbeat's setting takes it: `30`, `0.5`.
pub fn seconds(value: &str) -> Option<Duration> {
    value
        .parse::<f64>()
        .ok()
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map(Duration::from_secs_f64)
}

/// The width a terminal gives `text`: its characters, without colour or links.
pub fn width(text: &str) -> usize {
    let mut width = 0;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            width += 1;
            continue;
        }
        match chars.next() {
            Some('[') => {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == '\x07' || (c == '\x1b' && chars.next().is_some()) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    width
}

/// `text` cut to `columns`, ending in `…` when cut, its colour closed.
pub fn fit(text: &str, columns: usize) -> String {
    if width(text) <= columns {
        return text.to_string();
    }
    let mut out = String::new();
    let mut shown = 0;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            out.push(c);
            for c in chars.by_ref() {
                out.push(c);
                if ('@'..='~').contains(&c) && c != '[' {
                    break;
                }
            }
            continue;
        }
        if shown + 1 >= columns {
            break;
        }
        out.push(c);
        shown += 1;
    }
    out.push('…');
    if text.contains('\x1b') {
        out.push_str("\x1b[0m");
    }
    out
}

/// A bar `columns` wide, filled to `fraction`: green `━` on dim `─`.
pub fn bar(fraction: f64, columns: usize, color: bool) -> String {
    let full = ((fraction.clamp(0.0, 1.0) * columns as f64).round() as usize).min(columns);
    format!(
        "{}{}",
        paint(color, GREEN, &FILLED.repeat(full)),
        paint(color, DIM, &RULE.repeat(columns - full))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bar_fills_to_its_fraction() {
        assert_eq!(bar(0.5, 4, false), "━━──");
        assert_eq!(bar(2.0, 2, false), "━━");
        assert_eq!(bar(0.0, 2, true), "\x1b[32m\x1b[0m\x1b[2m──\x1b[0m");
    }

    #[test]
    fn a_line_too_wide_is_cut_and_its_colour_closed() {
        assert_eq!(width("\x1b[32mabc\x1b[0m"), 3);
        assert_eq!(fit("abcdef", 4), "abc…");
        assert_eq!(fit("\x1b[32mabcdef", 4), "\x1b[32mabc…\x1b[0m");
    }

    #[test]
    fn seconds_are_above_zero() {
        assert_eq!(seconds("0.5"), Some(Duration::from_millis(500)));
        assert_eq!(seconds("0"), None);
        assert_eq!(seconds("soon"), None);
    }
}
