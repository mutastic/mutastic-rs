/// A Windows path's drive letter.
pub fn drive(path: &str) -> Option<char> {
    if path.len() > 1 && path.as_bytes()[1] == b':' {
        path.chars().next()
    } else {
        None
    }
}
