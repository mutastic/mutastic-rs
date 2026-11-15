//! The HTML report's page is vendored from mutastic, where `report-ui/` builds it, at the commit
//! `src/report/page/report-page.json` pins (`scripts/pin-report-page.sh` moves it). The report module's own test
//! holds the vendored copy to the pin's sha256; this holds the pin to the checksum mutastic
//! committed with the page at that commit, fetched once and kept under the target directory.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

#[test]
fn the_pin_is_the_page_mutastic_built_at_its_commit() {
    let pin: Value =
        serde_json::from_str(include_str!("../src/report/page/report-page.json")).unwrap();
    let commit = pin["commit"].as_str().unwrap();
    let kept = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("report-page-{commit}.sha256"));
    let sums = match std::fs::read_to_string(&kept) {
        Ok(text) => text,
        Err(_) => {
            let url = format!(
                "{}/{commit}/{}.sha256",
                pin["raw"].as_str().unwrap(),
                pin["path"].as_str().unwrap()
            );
            let output = Command::new("curl")
                .args(["--fail", "--silent", "--show-error", "--location", &url])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "couldn't fetch the page's checksum from {url}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let text = String::from_utf8(output.stdout).unwrap();
            std::fs::write(&kept, &text).unwrap();
            text
        }
    };
    assert_eq!(
        sums.split_whitespace().next(),
        pin["sha256"].as_str(),
        "the pin.s sha256 isn't mutastic's for report.html at {commit}"
    );
}
