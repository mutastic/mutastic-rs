//! What only the operating system can say about the terminal, and the one key and signal a run
//! listens for: the terminal's width, so a live block fits it; `t`, Ctrl-T (SIGINFO, on macOS and
//! the BSDs) or SIGUSR1, for what's running now; and every way out leaving the terminal as it was,
//! its input in lines and its tab without progress.
//!
//! This is mutastic's one crate with unsafe code, and so where to audit it: it calls the C library
//! directly rather than take a dependency for a handful of calls. The calls are in each platform's
//! `ffi` module, one to an `unsafe` block, each saying why it's sound. Everything else is safe
//! logic over a trait of those calls, which the tests run against a fake, under Miri too; and the
//! tests hold each struct and constant declared for C to the `libc` crate's. Everything here
//! degrades to nothing: no width, no key, no signal, never an error.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The C library's calls, on the platforms whose layouts the tests check against `libc`'s. Linux
/// on MIPS, PowerPC and SPARC numbers its ioctls and lays out `termios` otherwise, and the other
/// BSDs, illumos and the rest weren't checked: they get nothing, as below.
#[cfg(any(
    all(
        any(target_os = "linux", target_os = "android"),
        any(
            target_arch = "x86_64",
            target_arch = "x86",
            target_arch = "aarch64",
            target_arch = "arm",
            target_arch = "riscv64",
            target_arch = "s390x",
            target_arch = "loongarch64"
        )
    ),
    target_vendor = "apple",
    target_os = "freebsd"
))]
#[path = "unix.rs"]
mod sys;

#[cfg(windows)]
#[path = "windows.rs"]
mod sys;

/// Everywhere else: no width, no key, no signal.
#[cfg(not(any(
    all(
        any(target_os = "linux", target_os = "android"),
        any(
            target_arch = "x86_64",
            target_arch = "x86",
            target_arch = "aarch64",
            target_arch = "arm",
            target_arch = "riscv64",
            target_arch = "s390x",
            target_arch = "loongarch64"
        )
    ),
    target_vendor = "apple",
    target_os = "freebsd",
    windows
)))]
mod sys {
    pub fn width() -> Option<usize> {
        None
    }
    pub fn listen_for_signals() {}
    pub fn guard(_: bool) {}
    pub struct Held;
    impl Held {
        pub fn take() -> Option<Held> {
            None
        }
    }
    pub fn key() -> Option<u8> {
        None
    }
}

/// Set when someone asks what's running: `t`, Ctrl-T or SIGUSR1. The progress ticker takes it.
static ASKED: AtomicBool = AtomicBool::new(false);

/// Whether someone asked what's running since the last time this was called.
pub fn asked() -> bool {
    ASKED.swap(false, Ordering::Relaxed)
}

/// Set while the tab shows progress: every way out clears it, Ctrl-C and kills included.
static TAB: AtomicBool = AtomicBool::new(false);

/// What clears the tab's progress (OSC 9;4, state 0).
pub const TAB_CLEARED: &str = "\x1b]9;4;0\x1b\\";

/// Whether the tab shows progress now, for the ways out that must clear it.
pub fn tab_shown(shown: bool) {
    TAB.store(shown, Ordering::Relaxed);
}

/// Whether the terminal draws progress in its tab, from what it says of itself: Windows Terminal,
/// ConEmu, Ghostty and WezTerm do. Others ignore the sequence, but some take `ESC ] 9` for a
/// notification, so it's only sent where it's known. `MUTASTIC_TERM_PROGRESS` (or Cargo's
/// `CARGO_TERM_PROGRESS_TERM_INTEGRATION`), `true` or `false`, decides instead.
pub fn tab_progress(var: impl Fn(&str) -> Option<String>) -> bool {
    if let Some(asked) =
        var("MUTASTIC_TERM_PROGRESS").or_else(|| var("CARGO_TERM_PROGRESS_TERM_INTEGRATION"))
    {
        return matches!(asked.as_str(), "true" | "1" | "always");
    }
    var("WT_SESSION").is_some()
        || var("ConEmuANSI").as_deref() == Some("ON")
        || var("TERM").as_deref() == Some("xterm-ghostty")
        || matches!(var("TERM_PROGRAM").as_deref(), Some("ghostty" | "WezTerm"))
}

/// While this lives, Ctrl-C, a kill or Ctrl-Z leave the terminal as it was before the signal's
/// own action: its input back to lines if `Keys` took it, its tab without progress.
pub struct Guard;

impl Guard {
    pub fn take() -> Guard {
        sys::guard(true);
        Guard
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        sys::guard(false);
    }
}

/// stderr's width in columns, when it's a terminal that says.
pub fn width() -> Option<usize> {
    columns(sys::width())
}

/// A width the terminal gives, unless it gives none: a terminal with no size says 0.
fn columns(said: Option<usize>) -> Option<usize> {
    said.filter(|&columns| columns > 0)
}

/// Listens for Ctrl-T (SIGINFO) and SIGUSR1, which ask what's running.
pub fn listen_for_signals() {
    sys::listen_for_signals();
}

/// The terminal's input, for the `t` key: read a key at a time and not echoed, while this lives.
/// Ctrl-C and Ctrl-Z still work, and leave the terminal as it was.
pub struct Keys {
    listener: Listener<sys::Held>,
}

impl Keys {
    /// Listens for `t` when stdin is a terminal; otherwise nothing is listened for.
    pub fn listen() -> Keys {
        use std::io::IsTerminal as _;
        Keys {
            listener: Listener::start(
                std::io::stdin().is_terminal(),
                sys::Held::take,
                sys::key,
                &ASKED,
            ),
        }
    }

    /// Whether `t` is being listened for.
    pub fn listening(&self) -> bool {
        self.listener.reader.is_some()
    }
}

/// What reads the keys: `held` keeps the terminal reading a key at a time, and a thread reads
/// them until it's dropped. Over what holds the terminal and what reads a key, so the tests can
/// give it fakes.
struct Listener<H> {
    _held: Option<H>,
    stop: Arc<AtomicBool>,
    reader: Option<std::thread::JoinHandle<()>>,
}

impl<H> Listener<H> {
    /// Takes the terminal, when `terminal` says it's one and `take` gets it, and reads keys from
    /// it with `key` until dropped: each `t` sets `asked`.
    fn start(
        terminal: bool,
        take: impl FnOnce() -> Option<H>,
        key: impl Fn() -> Option<u8> + Send + 'static,
        asked: &'static AtomicBool,
    ) -> Listener<H> {
        let stop = Arc::new(AtomicBool::new(false));
        if !terminal {
            return Listener {
                _held: None,
                stop,
                reader: None,
            };
        }
        let held = take();
        let stopping = Arc::clone(&stop);
        let reader = held.is_some().then(|| {
            std::thread::spawn(move || {
                while !stopping.load(Ordering::Relaxed) {
                    if key() == Some(b't') {
                        asked.store(true, Ordering::Relaxed);
                    }
                }
            })
        });
        Listener {
            _held: held,
            stop,
            reader,
        }
    }
}

impl<H> Drop for Listener<H> {
    /// Stops reading before the terminal goes back to lines, so no key typed after the run is
    /// taken: the fields, `_held` among them, are dropped after this.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The tests that read or set `ASKED` or `TAB`, one at a time.
    pub(crate) static FLAGS: Mutex<()> = Mutex::new(());

    fn with(vars: &[(&str, &str)]) -> bool {
        tab_progress(|name| {
            vars.iter()
                .find(|(var, _)| *var == name)
                .map(|(_, value)| value.to_string())
        })
    }

    #[test]
    fn the_tab_shows_progress_where_the_terminal_says_it_draws_it() {
        assert!(with(&[("WT_SESSION", "f00")]));
        assert!(with(&[("ConEmuANSI", "ON")]));
        assert!(with(&[("TERM", "xterm-ghostty")]));
        assert!(with(&[("TERM_PROGRAM", "ghostty")]));
        assert!(with(&[("TERM_PROGRAM", "WezTerm")]));
        assert!(!with(&[]));
        assert!(!with(&[("ConEmuANSI", "OFF")]));
        assert!(!with(&[("TERM", "xterm-256color")]));
        assert!(!with(&[("TERM_PROGRAM", "iTerm.app")]));
    }

    #[test]
    fn what_you_set_decides_the_tab_over_what_the_terminal_says() {
        for yes in ["true", "1", "always"] {
            assert!(with(&[("MUTASTIC_TERM_PROGRESS", yes)]), "{yes}");
            assert!(
                with(&[("CARGO_TERM_PROGRESS_TERM_INTEGRATION", yes)]),
                "{yes}"
            );
        }
        assert!(!with(&[
            ("MUTASTIC_TERM_PROGRESS", "false"),
            ("WT_SESSION", "f00")
        ]));
        assert!(!with(&[("MUTASTIC_TERM_PROGRESS", "yes")]));
        assert!(!with(&[
            ("CARGO_TERM_PROGRESS_TERM_INTEGRATION", "false"),
            ("TERM_PROGRAM", "WezTerm"),
        ]));
        // mutastic's own variable before Cargo's.
        assert!(!with(&[
            ("MUTASTIC_TERM_PROGRESS", "false"),
            ("CARGO_TERM_PROGRESS_TERM_INTEGRATION", "true"),
        ]));
        assert!(with(&[
            ("MUTASTIC_TERM_PROGRESS", "true"),
            ("CARGO_TERM_PROGRESS_TERM_INTEGRATION", "false"),
        ]));
    }

    #[test]
    fn a_question_is_answered_once() {
        let _flags = FLAGS.lock().unwrap();
        assert!(!asked());
        ASKED.store(true, Ordering::Relaxed);
        assert!(asked());
        assert!(!asked());
    }

    #[test]
    fn the_tab_is_marked_while_it_shows_progress() {
        let _flags = FLAGS.lock().unwrap();
        tab_shown(true);
        assert!(TAB.load(Ordering::Relaxed));
        tab_shown(false);
        assert!(!TAB.load(Ordering::Relaxed));
    }

    #[test]
    fn a_terminal_with_no_size_has_no_width() {
        assert_eq!(columns(Some(120)), Some(120));
        assert_eq!(columns(Some(1)), Some(1));
        assert_eq!(columns(Some(0)), None);
        assert_eq!(columns(None), None);
    }

    /// What holds the fake terminal: says in `log` when it lets go.
    struct Held(Arc<Mutex<Vec<&'static str>>>);

    impl Drop for Held {
        fn drop(&mut self) {
            self.0.lock().unwrap().push("let go");
        }
    }

    /// Keys typed one after another, then none, each read written to `log`.
    fn typed(
        keys: &'static [u8],
        log: &Arc<Mutex<Vec<&'static str>>>,
    ) -> impl Fn() -> Option<u8> + Send + 'static {
        let next = std::sync::atomic::AtomicUsize::new(0);
        let log = Arc::clone(log);
        move || {
            log.lock().unwrap().push("read");
            let key = keys.get(next.fetch_add(1, Ordering::Relaxed)).copied();
            if key.is_none() {
                // As the terminal does: a tenth of a second with nothing, here a millisecond.
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            key
        }
    }

    #[test]
    fn t_asks_whats_running_and_other_keys_dont() {
        static ASKED_HERE: AtomicBool = AtomicBool::new(false);
        let log = Arc::new(Mutex::new(Vec::new()));
        let held = Held(Arc::clone(&log));
        let listener = Listener::start(true, || Some(held), typed(b"xq", &log), &ASKED_HERE);
        assert!(listener.reader.is_some());
        while log.lock().unwrap().len() < 4 {
            std::thread::yield_now();
        }
        assert!(!ASKED_HERE.load(Ordering::Relaxed));
        drop(listener);

        let log = Arc::new(Mutex::new(Vec::new()));
        let held = Held(Arc::clone(&log));
        let listener = Listener::start(true, || Some(held), typed(b"xt", &log), &ASKED_HERE);
        while !ASKED_HERE.load(Ordering::Relaxed) {
            std::thread::yield_now();
        }
        drop(listener);
    }

    #[test]
    fn the_reader_stops_before_the_terminal_goes_back_to_lines() {
        static ASKED_HERE: AtomicBool = AtomicBool::new(false);
        let log = Arc::new(Mutex::new(Vec::new()));
        let held = Held(Arc::clone(&log));
        let listener = Listener::start(true, || Some(held), typed(b"", &log), &ASKED_HERE);
        while log.lock().unwrap().is_empty() {
            std::thread::yield_now();
        }
        drop(listener);
        let log = log.lock().unwrap();
        assert_eq!(log.last(), Some(&"let go"), "{log:?}");
        assert_eq!(log.iter().filter(|&&event| event == "let go").count(), 1);
    }

    #[test]
    fn off_a_terminal_nothing_is_taken_or_read() {
        static ASKED_HERE: AtomicBool = AtomicBool::new(false);
        let log = Arc::new(Mutex::new(Vec::new()));
        let listener: Listener<Held> = Listener::start(
            false,
            || panic!("the terminal was taken"),
            typed(b"t", &log),
            &ASKED_HERE,
        );
        assert!(listener.reader.is_none());
        drop(listener);
        assert!(log.lock().unwrap().is_empty());
    }

    #[test]
    fn a_terminal_that_cant_be_taken_isnt_read() {
        static ASKED_HERE: AtomicBool = AtomicBool::new(false);
        let log = Arc::new(Mutex::new(Vec::new()));
        let listener: Listener<Held> =
            Listener::start(true, || None, typed(b"t", &log), &ASKED_HERE);
        assert!(listener.reader.is_none());
        drop(listener);
        assert!(log.lock().unwrap().is_empty());
        assert!(!ASKED_HERE.load(Ordering::Relaxed));
    }
}
