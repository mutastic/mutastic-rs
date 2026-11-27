//! The command line written back for a person to copy, as a shell takes it: a mistyped word
//! corrected, the rest as it was typed.

/// `mutastic` and the words after it, with `args[at]` replaced by `with`, and the `taken` words
/// after it left out, as a shell takes them.
pub fn line(args: &[String], at: usize, with: &str, taken: usize) -> String {
    let words: Vec<String> = args
        .iter()
        .enumerate()
        .filter(|(index, _)| !(at + 1..=at + taken).contains(index))
        .map(|(index, word)| if index == at { with } else { word })
        .map(shell)
        .collect();
    format!("  mutastic {}", words.join(" "))
}

/// A word as a shell takes it: in double quotes when it has a space or a quote.
pub fn shell(word: &str) -> String {
    match word.split_once('=') {
        Some((flag, value)) if value.contains([' ', '\'', '"']) => {
            format!("{flag}=\"{}\"", value.replace('"', "\\\""))
        }
        _ if word.contains([' ', '\'', '"']) => format!("\"{}\"", word.replace('"', "\\\"")),
        _ => word.to_string(),
    }
}
