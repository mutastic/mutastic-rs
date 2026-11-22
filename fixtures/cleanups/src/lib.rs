//! Releases a test can see: a lock let go early, a value's `Drop`, a file removed. Each test is the
//! one the card names, and catches the release taken away; the same code run by a test that
//! checks nothing keeps its mutant, to show the card.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// Counts, lets the lock go, then says whether another taker could have it.
pub fn bump(counter: &Mutex<u32>) -> bool {
    let mut guard = counter.lock().unwrap();
    *guard += 1;
    drop(guard);
    counter.try_lock().is_ok()
}

/// Counts each one dropped.
pub struct Counted(pub Arc<AtomicU32>);

impl Drop for Counted {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// Removes what it was given.
pub fn cleaned(path: &Path) {
    fs::remove_file(path).unwrap();
}

/// As `bump`, run by a test that checks nothing.
pub fn bump_alone(counter: &Mutex<u32>) -> bool {
    let mut guard = counter.lock().unwrap();
    *guard += 1;
    drop(guard);
    counter.try_lock().is_ok()
}

/// Counts each one dropped, run by a test that checks nothing.
pub struct Silent(pub Arc<AtomicU32>);

impl Drop for Silent {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// As `cleaned`, run by a test that checks nothing.
pub fn left(path: &Path) {
    fs::remove_file(path).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_is_free_again() {
        let counter = Mutex::new(0);
        assert!(bump(&counter));
        assert_eq!(*counter.lock().unwrap(), 1);
    }

    #[test]
    fn dropping_one_counts_it() {
        let drops = Arc::new(AtomicU32::new(0));
        drop(Counted(Arc::clone(&drops)));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn the_file_is_gone() {
        let dir = std::env::temp_dir().join(format!("cleanups-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("made");
        fs::write(&path, "x").unwrap();
        cleaned(&path);
        let gone = !path.exists();
        let _ = fs::remove_dir_all(&dir);
        assert!(gone);
    }

    #[test]
    fn the_others_run() {
        bump_alone(&Mutex::new(0));
        drop(Silent(Arc::new(AtomicU32::new(0))));
        let path = std::env::temp_dir().join(format!("cleanups-left-{}", std::process::id()));
        fs::write(&path, "x").unwrap();
        left(&path);
        let _ = fs::remove_file(&path);
    }
}
