//! Finding mutants: any text, as a crate's only file, gives its mutants, never a panic. Every
//! operator, unsafe code included, so every finder sees it.
#![no_main]

use std::path::PathBuf;

use mutastic::sites::{self, UnsafeMode};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("mutastic-fuzz-sites-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/lib.rs"), text).unwrap();
    let _ = sites::find_with(
        &dir,
        "fuzzed",
        &[PathBuf::from("src/lib.rs")],
        sites::ALL,
        UnsafeMode::All,
        &mutastic::cfg::Cfg::unknown(),
    );
});
