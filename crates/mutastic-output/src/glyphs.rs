//! The glyphs output draws with, each beside a word or a colour, never instead of one, so output
//! reads the same without colour and when copied. Both editions draw the same ones.

/// Passed: caught, done, all clear.
pub const PASS: &str = "✓";
/// Failed: missed.
pub const FAIL: &str = "✗";
/// Not reached: not covered.
pub const OPEN: &str = "○";
/// Took too long.
pub const TIMER: &str = "⏱";
/// What to look at, or to do, next.
pub const POINTER: &str = "❯";
/// Up since the last time.
pub const UP: &str = "▲";
/// Down since the last time.
pub const DOWN: &str = "▼";
/// A rule across a section, repeated; a bar's empty part.
pub const RULE: &str = "─";
/// A bar's filled part, repeated.
pub const FILLED: &str = "━";
/// The gutter beside a block of code.
pub const GUTTER: &str = "│";
/// What opens a block of code, before its location.
pub const CORNER: &str = "┌─";
/// What hangs a line under the one above.
pub const UNDER: &str = "└";
/// A spinner's frames, one per redraw.
pub const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
