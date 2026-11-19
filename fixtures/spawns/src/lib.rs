//! Threads started and not waited for here: a test that waits for one and checks what it did
//! catches its `spawn` taken away, and a test that doesn't wait can't.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;

/// Starts a thread that sets the flag, then says so.
pub fn spawned(flag: Arc<AtomicBool>, done: Sender<()>) {
    thread::spawn(move || {
        flag.store(true, Ordering::SeqCst);
        let _ = done.send(());
    });
}

/// The same, on a named thread from a builder.
pub fn built(flag: Arc<AtomicBool>, done: Sender<()>) {
    let _ = thread::Builder::new()
        .name("worker".to_string())
        .spawn(move || {
            flag.store(true, Ordering::SeqCst);
            let _ = done.send(());
        });
}

/// Starts a thread no test waits for.
pub fn unwaited(flag: Arc<AtomicBool>) {
    std::thread::spawn(move || flag.store(true, Ordering::SeqCst));
}

/// A scoped thread, which the scope waits for: its card is any statement's.
pub fn scoped(flag: &AtomicBool) {
    thread::scope(|scope| {
        scope.spawn(|| flag.store(true, Ordering::SeqCst));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// What the card says to do: wait on a channel the thread sends to, then look.
    fn waited(start: fn(Arc<AtomicBool>, Sender<()>)) -> bool {
        let flag = Arc::new(AtomicBool::new(false));
        let (done, finished) = mpsc::channel();
        start(Arc::clone(&flag), done);
        finished.recv_timeout(Duration::from_secs(5)).is_ok() && flag.load(Ordering::SeqCst)
    }

    #[test]
    fn spawned_sets_the_flag() {
        assert!(waited(spawned));
    }

    #[test]
    fn built_sets_the_flag() {
        assert!(waited(built));
    }

    #[test]
    fn unwaited_returns() {
        unwaited(Arc::new(AtomicBool::new(false)));
    }

    #[test]
    fn scoped_returns() {
        scoped(&AtomicBool::new(false));
    }
}
