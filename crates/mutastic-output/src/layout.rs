//! Help's layout: text wrapped between words, a name with its meaning hanging beside it, and an
//! example broken as a shell still takes it.

/// `name`, then `text` from `column` on, wrapped there; a name too wide for the column has a line
/// of its own. No line is wider than `width`, unless a word is.
pub fn hanging(name: &str, text: &str, column: usize, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut first = name.to_string();
    if first.chars().count() + 1 > column {
        out.push(first);
        first = String::new();
    }
    for line in wrapped(text, width - column) {
        out.push(format!("{first:<column$}{line}"));
        first = String::new();
    }
    out
}

/// An example as it's shown, set in by two spaces. One too long for a line is broken between its
/// words, outside quotes, each line but the last ending in ` \` so the whole still pastes into a
/// shell as one command.
pub fn example_lines(example: &str, width: usize) -> Vec<String> {
    const INDENT: &str = "  ";
    const MORE: &str = "      ";
    const BREAK: &str = " \\";
    if INDENT.len() + example.chars().count() <= width {
        return vec![format!("{INDENT}{example}")];
    }
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in shell_words(example) {
        let lead = if lines.is_empty() { INDENT } else { MORE };
        let longer = lead.len() + line.chars().count() + 1 + word.chars().count() + BREAK.len();
        if !line.is_empty() && longer > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    lines.push(line);
    let last = lines.len() - 1;
    lines
        .into_iter()
        .enumerate()
        .map(|(at, line)| {
            let lead = if at == 0 { INDENT } else { MORE };
            let more = if at < last { BREAK } else { "" };
            format!("{lead}{line}{more}")
        })
        .collect()
}

/// `line`'s words, split at spaces outside double quotes.
fn shell_words(line: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut quoted = false;
    let mut start = 0;
    for (at, character) in line.char_indices() {
        match character {
            '"' => quoted = !quoted,
            ' ' if !quoted => {
                words.push(&line[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    words.push(&line[start..]);
    words.retain(|word| !word.is_empty());
    words
}

/// `text` in lines of at most `width` characters, broken between words.
pub fn wrapped(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    lines.push(line);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_breaks_between_words() {
        assert_eq!(wrapped("one two three", 7), ["one two", "three"]);
        assert_eq!(wrapped("", 7), [""]);
    }

    #[test]
    fn a_meaning_hangs_beside_its_name() {
        assert_eq!(
            hanging("  -q", "one two three", 6, 13),
            ["  -q  one two", "      three"]
        );
        assert_eq!(hanging("  --long", "one", 6, 13), ["  --long", "      one"]);
    }

    #[test]
    fn a_long_example_breaks_outside_its_quotes() {
        let lines = example_lines(
            "mutastic run --since=main --only=\"a b c\" --format=agent",
            30,
        );
        assert_eq!(
            lines,
            [
                "  mutastic run --since=main \\",
                "      --only=\"a b c\" \\",
                "      --format=agent"
            ]
        );
    }
}
