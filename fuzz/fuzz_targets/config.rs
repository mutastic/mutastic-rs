//! `mutastic.yaml`: any text reads as a configuration or as an error, never a panic.
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = mutastic::config::parse(text);
    }
});
