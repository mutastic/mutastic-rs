//! Calls left out, and writes that don't happen.

use std::collections::HashMap;

#[derive(Default)]
pub struct Cart {
    items: Vec<String>,
    history: Vec<String>,
}

impl Cart {
    pub fn add(&mut self, item: &str) {
        self.items.push(item.to_string());
        self.history.push(format!("added {item}"));
    }

    pub fn count(&self) -> usize {
        self.items.len()
    }
}

/// Whether this is the first time `name` is seen.
pub fn first_visit(seen: &mut HashMap<String, u32>, name: &str) -> bool {
    let first = !seen.contains_key(name);
    seen.insert(name.to_string(), 1);
    first
}

pub fn with_tip(bill: u32, tip: u32) -> u32 {
    let mut total = bill;
    total = total + tip;
    total
}

pub struct Order {
    pub paid: bool,
    pub total: u32,
}

impl Order {
    /// Marks the order paid, and gives what was due.
    pub fn pay(&mut self) -> u32 {
        self.paid = true;
        self.total
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub volume: u8,
    pub muted: bool,
}

pub fn muted(settings: &Settings) -> Settings {
    Settings {
        muted: true,
        ..settings.clone()
    }
}

/// Runs `work` on a thread of its own, and waits for it to finish.
pub fn run_and_wait(work: impl FnOnce() + Send + 'static) {
    let handle = std::thread::spawn(work);
    handle.join().unwrap();
}

/// Deletes the file at `path`, which must be there.
pub fn discard(path: &std::path::Path) {
    let removed = std::fs::remove_file(path);
    removed.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn adding_an_item_counts_it() {
        let mut cart = Cart::default();
        cart.add("tea");
        assert_eq!(cart.count(), 1);
    }

    #[test]
    fn a_first_visit() {
        let mut seen = HashMap::new();
        assert!(first_visit(&mut seen, "ann"));
    }

    #[test]
    fn no_tip() {
        assert_eq!(with_tip(10, 0), 10);
    }

    #[test]
    fn paying_gives_the_total() {
        let mut order = Order {
            paid: false,
            total: 30,
        };
        assert_eq!(order.pay(), 30);
    }

    #[test]
    fn the_work_is_done_when_it_returns() {
        let done = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&done);
        run_and_wait(move || {
            thread::sleep(Duration::from_millis(200));
            flag.store(true, Ordering::SeqCst);
        });
        assert!(done.load(Ordering::SeqCst));
    }

    #[test]
    fn a_discarded_file_is_gone() {
        // A name of its own: copies of this module can run side by side in one process.
        let name = format!("{}-discarded.txt", module_path!().replace("::", "-"));
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, "old").unwrap();
        discard(&path);
        assert!(!path.exists());
    }

    #[test]
    fn muting_keeps_the_volume() {
        let loud = Settings {
            volume: 9,
            muted: false,
        };
        assert_eq!(muted(&loud).volume, 9);
    }
}
