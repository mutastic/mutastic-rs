//! The colour roles, as the terminal's SGR codes, the Java edition's code for code; labels drawn
//! as blocks of colour; and terminal links. Every colour has a glyph or a word beside it
//! ([`crate::glyphs`]), so output reads the same without it.

/// What passed or is done.
pub const GREEN: &str = "32";
/// What failed: only its glyph, label and caret.
pub const RED: &str = "1;31";
/// What wasn't reached, or needs a look.
pub const AMBER: &str = "33";
/// What's slow: amber, in bold.
pub const SLOW: &str = "1;33";
/// A heading, or what to do next.
pub const BLUE: &str = "1;34";
/// A name to copy, an id say.
pub const CYAN: &str = "36";
pub const BOLD: &str = "1";
/// What's said by the way.
pub const DIM: &str = "2";

/// Label backgrounds.
pub const RED_LABEL: &str = "41";
pub const GREEN_LABEL: &str = "42";
pub const CYAN_LABEL: &str = "46";

/// Nerd Font half-circles, for rounded label ends.
const ROUND_LEFT: &str = "\u{e0b6}";
const ROUND_RIGHT: &str = "\u{e0b4}";

/// `text` in `code`, then the colour reset, when `color` is on; else `text` as it is.
pub fn paint(color: bool, code: &str, text: &str) -> String {
    if color {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

/// How a label such as `MISSED` is drawn in colour, as the Java edition draws it: a block with
/// square ends, the default; with rounded ends, which need a Nerd Font, or the ends show as boxes;
/// or no block at all, only the label's words in its colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Labels {
    #[default]
    Square,
    Rounded,
    Off,
}

impl Labels {
    /// The choices a setting for them takes.
    pub const CHOICES: &[&str] = &["square", "rounded", "off"];

    pub fn parse(name: &str) -> Option<Labels> {
        match name {
            "square" => Some(Labels::Square),
            "rounded" => Some(Labels::Rounded),
            "off" => Some(Labels::Off),
            _ => None,
        }
    }
}

/// A label, `MISSED` on red: black bold text on a block of `background`, `[MISSED]` without colour.
/// With rounded ends the ends are drawn in the label's colour; with none, the words are.
pub fn label(color: bool, labels: Labels, text: &str, background: &str) -> String {
    if !color {
        return format!("[{text}]");
    }
    // 41, a red background, is 31 for red text.
    let edge = background
        .parse::<u8>()
        .map_or_else(|_| background.to_string(), |code| (code - 10).to_string());
    match labels {
        Labels::Square => format!("\x1b[{background};30;1m {text} \x1b[0m"),
        Labels::Rounded => format!(
            "{}\x1b[{background};30;1m{text}\x1b[0m{}",
            paint(color, &edge, ROUND_LEFT),
            paint(color, &edge, ROUND_RIGHT)
        ),
        Labels::Off => paint(color, &format!("{BOLD};{edge}"), text),
    }
}

/// `text` linking to `url` (OSC 8), when `links` is on: terminals that know links open it, and
/// others show the text.
pub fn link(links: bool, url: &str, text: &str) -> String {
    if links {
        format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_role_is_its_code_then_a_reset() {
        assert_eq!(paint(true, RED, "✗"), "\x1b[1;31m✗\x1b[0m");
        assert_eq!(paint(false, GREEN, "✓"), "✓");
    }

    #[test]
    fn a_label_is_a_block_or_brackets() {
        assert_eq!(
            label(false, Labels::Square, "MISSED", RED_LABEL),
            "[MISSED]"
        );
        assert_eq!(
            label(true, Labels::Square, "MISSED", RED_LABEL),
            "\x1b[41;30;1m MISSED \x1b[0m"
        );
        assert_eq!(
            label(true, Labels::Off, "MISSED", RED_LABEL),
            "\x1b[1;31mMISSED\x1b[0m"
        );
        assert!(label(true, Labels::Rounded, "MISSED", RED_LABEL).starts_with("\x1b[31m\u{e0b6}"));
    }

    #[test]
    fn a_link_is_its_text_without_links() {
        assert_eq!(link(false, "file:///a", "a"), "a");
        assert_eq!(
            link(true, "file:///a", "a"),
            "\x1b]8;;file:///a\x1b\\a\x1b]8;;\x1b\\"
        );
        assert_eq!(Labels::parse("rounded"), Some(Labels::Rounded));
        assert_eq!(Labels::parse("round"), None);
    }
}
