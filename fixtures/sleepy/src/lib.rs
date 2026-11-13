//! A crate whose one test takes three seconds: longer than a heartbeat, so a run on it shows
//! whether mutastic stays quiet while a slow test runs.

/// Twice `x`.
pub fn double(x: u32) -> u32 {
    x * 2
}

#[cfg(test)]
mod tests {
    #[test]
    fn doubles_slowly() {
        std::thread::sleep(std::time::Duration::from_secs(3));
        assert_eq!(super::double(3), 6);
    }
}
