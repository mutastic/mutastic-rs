//! std's methods, swapped for their counterparts, and the strings in the code.

pub fn is_comment(line: &str) -> bool {
    line.starts_with('#')
}

/// Never more than half off.
pub fn capped_discount(percent: u32) -> u32 {
    percent.min(50)
}

pub fn extension(file: &str) -> Option<&str> {
    file.find('.').map(|dot| &file[dot + 1..])
}

pub fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort_by(|a, b| a.cmp(b));
    names
}

pub fn header(columns: &[&str]) -> String {
    columns.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_lines_are_not_comments() {
        assert!(!is_comment("hello"));
    }

    #[test]
    fn half_off_at_most() {
        assert_eq!(capped_discount(50), 50);
    }

    #[test]
    fn a_text_file() {
        assert_eq!(extension("notes.txt"), Some("txt"));
    }

    #[test]
    fn sorting_keeps_every_name() {
        let names = vec!["cy".to_string(), "al".to_string(), "bo".to_string()];
        assert_eq!(sorted(names).len(), 3);
    }

    #[test]
    fn one_column() {
        assert_eq!(header(&["name"]), "name");
    }
}
