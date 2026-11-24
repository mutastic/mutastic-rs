//! Amounts as mutastic writes them, in a summary, a progress line or a note: a count with its
//! noun, `3 tests`, and how long something took, `2.1s`.

/// `1 test`, `3 tests`: a count with its noun, plural unless it is one.
pub fn count(number: usize, noun: &str) -> String {
    format!("{number} {noun}{}", if number == 1 { "" } else { "s" })
}

/// `340ms`, `2.1s`, `1m39s`, as the Java edition writes them.
pub fn duration(took: std::time::Duration) -> String {
    let millis = took.as_millis();
    if millis < 1000 {
        format!("{millis}ms")
    } else if millis < 60_000 {
        format!("{:.1}s", millis as f64 / 1000.0)
    } else {
        format!("{}m{:02}s", millis / 60_000, (millis / 1000) % 60)
    }
}
