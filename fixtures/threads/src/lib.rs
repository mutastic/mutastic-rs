//! Threads set up with `std::thread::Builder`: names and a stack size, which only change how a
//! thread looks or what it costs, beside a spawn whose work decides what the code does.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

/// Squares `n` on a named thread with a stack of its own, and waits for it.
pub fn square(n: u64) -> u64 {
    thread::Builder::new()
        .name("squarer".to_string())
        .stack_size(64 * 1024)
        .spawn(move || n * n)
        .expect("spawn")
        .join()
        .expect("join")
}

/// Starts a named thread that sets `flag`, and doesn't wait for it.
pub fn fire_and_forget(flag: Arc<AtomicBool>) {
    let _ = thread::Builder::new()
        .name(String::from("background"))
        .spawn(move || flag.store(true, Ordering::SeqCst));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squares() {
        assert_eq!(square(7), 49);
    }

    #[test]
    fn forgets() {
        fire_and_forget(Arc::new(AtomicBool::new(false)));
    }
}
