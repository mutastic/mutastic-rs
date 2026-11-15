use std::time::Duration;

/// Hands a parcel over: one that needs a signature waits for someone to come to the door.
pub fn hand_over(signature: bool) -> &'static str {
    if signature {
        std::thread::sleep(Duration::from_secs(6));
        "signed for"
    } else {
        "left in the porch"
    }
}
