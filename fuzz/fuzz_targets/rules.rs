//! A rules file: any text reads as call operators or as an error, never a panic.
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = mutastic::callrules::parse(text, "fuzzed.yaml");
    }
});
