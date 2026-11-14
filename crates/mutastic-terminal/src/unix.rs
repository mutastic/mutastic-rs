//! The terminal on Linux, Android, macOS and FreeBSD: its width from `ioctl`, its input mode from
//! `tcgetattr` and `tcsetattr`, and its signals from `signal` and `raise`. The calls are in `ffi`,
//! behind `Calls`; what they're asked and when, the handlers included, is logic over `Calls`.

use std::os::raw::{c_int, c_ulong, c_ushort};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// `struct winsize`.
#[repr(C)]
struct Winsize {
    rows: c_ushort,
    columns: c_ushort,
    x_pixels: c_ushort,
    y_pixels: c_ushort,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
const TIOCGWINSZ: c_ulong = 0x5413;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const TIOCGWINSZ: c_ulong = 0x4008_7468;

#[cfg(any(target_os = "linux", target_os = "android"))]
const SIGUSR1: c_int = 10;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const SIGUSR1: c_int = 30;
/// Ctrl-T's signal: only macOS and the BSDs have it.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const SIGINFO: Option<c_int> = Some(29);
#[cfg(any(target_os = "linux", target_os = "android"))]
const SIGINFO: Option<c_int> = None;
const SIGINT: c_int = 2;
const SIGTERM: c_int = 15;
#[cfg(any(target_os = "linux", target_os = "android"))]
const SIGTSTP: c_int = 20;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const SIGTSTP: c_int = 18;
#[cfg(any(target_os = "linux", target_os = "android"))]
const SIGCONT: c_int = 18;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const SIGCONT: c_int = 19;
const SIG_DFL: usize = 0;

/// `struct termios`, as each C library lays it out: glibc's and musl's on Linux, with the speeds,
/// Bionic's on Android, the kernel's, without; and the BSDs', where macOS's flags are `unsigned
/// long` and FreeBSD's `unsigned int`.
#[cfg(target_os = "linux")]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Termios {
    iflag: u32,
    oflag: u32,
    cflag: u32,
    lflag: u32,
    line: u8,
    cc: [u8; 32],
    ispeed: u32,
    ospeed: u32,
}
#[cfg(target_os = "android")]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Termios {
    iflag: u32,
    oflag: u32,
    cflag: u32,
    lflag: u32,
    line: u8,
    cc: [u8; 19],
}
#[cfg(any(target_os = "linux", target_os = "android"))]
type Flags = u32;
#[cfg(any(target_os = "linux", target_os = "android"))]
const ICANON: Flags = 0o2;
#[cfg(any(target_os = "linux", target_os = "android"))]
const ECHO: Flags = 0o10;
#[cfg(any(target_os = "linux", target_os = "android"))]
const VTIME: usize = 5;
#[cfg(any(target_os = "linux", target_os = "android"))]
const VMIN: usize = 6;

#[cfg(not(any(target_os = "linux", target_os = "android")))]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Termios {
    iflag: Flags,
    oflag: Flags,
    cflag: Flags,
    lflag: Flags,
    cc: [u8; 20],
    ispeed: Flags,
    ospeed: Flags,
}
#[cfg(target_vendor = "apple")]
type Flags = c_ulong;
#[cfg(target_os = "freebsd")]
type Flags = std::os::raw::c_uint;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const ICANON: Flags = 0x100;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const ECHO: Flags = 0x8;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const VMIN: usize = 16;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const VTIME: usize = 17;

const TCSANOW: c_int = 0;

/// Where a signal goes: to `on_ask`, to `on_leave`, or to its default action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Handler {
    Ask,
    Leave,
    Default,
}

/// The C library's calls, one each. Miri can't call C, so everything else is written over this,
/// and the tests give it a fake.
pub(crate) trait Calls: Sync {
    /// stderr's width in columns, from `ioctl(TIOCGWINSZ)`, when it's a terminal.
    fn columns(&self) -> Option<usize>;
    /// The handler a signal goes to from now on, from `signal`.
    fn signal(&self, signum: c_int, handler: Handler);
    /// The signal sent to this process, from `raise`.
    fn raise(&self, signum: c_int);
    /// stdin's terminal settings, from `tcgetattr`, when it's a terminal.
    fn settings(&self) -> Option<Termios>;
    /// stdin's terminal settings set now, with `tcsetattr`.
    fn set(&self, settings: &Termios);
    /// `bytes` written to stderr, with `write`.
    fn write_stderr(&self, bytes: &[u8]);
}

/// The terminal's settings before the run took it and the run's own, and whether the run's are
/// the terminal's now: what every way back reads.
pub(crate) struct Modes {
    saved: OnceLock<(Termios, Termios)>,
    taken: AtomicBool,
}

impl Modes {
    const fn new() -> Modes {
        Modes {
            saved: OnceLock::new(),
            taken: AtomicBool::new(false),
        }
    }
}

/// The C library itself.
static C: ffi::Libc = ffi::Libc;
/// The terminal's settings, for the real calls and the signal handlers.
static MODES: Modes = Modes::new();

pub fn width() -> Option<usize> {
    C.columns()
}

pub fn listen_for_signals() {
    listen(&C);
}

pub fn guard(on: bool) {
    guard_with(&C, on);
}

/// The terminal reading a key at a time, without echo, until this is dropped.
pub struct Held;

impl Held {
    pub fn take() -> Option<Held> {
        take(&C, &MODES).then_some(Held)
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        give_back(&C, &MODES);
    }
}

/// A key from the terminal, or `None` after a tenth of a second with none: the terminal
/// returns a read with nothing then, as `Held` set it to.
pub fn key() -> Option<u8> {
    use std::io::Read as _;
    let mut byte = [0u8; 1];
    match std::io::stdin().read(&mut byte) {
        Ok(1) => Some(byte[0]),
        _ => None,
    }
}

/// Ctrl-T and SIGUSR1, the handler `ffi::on_ask`'s.
fn on_ask() {
    // Only an atomic store: all a signal handler may safely do here.
    super::ASKED.store(true, Ordering::Relaxed);
}

/// Ctrl-C, a kill, Ctrl-Z and a continue, the handler `ffi::on_leave`'s.
fn on_leave(signum: c_int) {
    on_signal(&C, &MODES, &super::TAB, signum);
}

/// SIGUSR1 and, where there's one, Ctrl-T's signal ask what's running.
fn listen(c: &impl Calls) {
    for signum in [Some(SIGUSR1), SIGINFO].into_iter().flatten() {
        c.signal(signum, Handler::Ask);
    }
}

/// Ctrl-C, a kill and Ctrl-Z go through `on_leave` while `on`, and to their default after.
fn guard_with(c: &impl Calls, on: bool) {
    let handler = if on { Handler::Leave } else { Handler::Default };
    for signum in [SIGINT, SIGTERM, SIGTSTP, SIGCONT] {
        c.signal(signum, handler);
    }
}

/// The settings a run reads keys with: a key at a time, not echoed, and a read returns after a
/// tenth of a second with nothing, so the reader can stop.
fn keys_mode(before: Termios) -> Termios {
    let mut during = before;
    during.lflag &= !(ICANON | ECHO);
    during.cc[VMIN] = 0;
    during.cc[VTIME] = 1;
    during
}

/// Takes the terminal for the keys: its settings kept for every way back, and the run's set.
/// Only once in a process.
fn take(c: &impl Calls, modes: &Modes) -> bool {
    let Some(before) = c.settings() else {
        return false;
    };
    let during = keys_mode(before);
    if modes.saved.set((before, during)).is_err() {
        return false;
    }
    modes.taken.store(true, Ordering::Relaxed);
    c.set(&during);
    true
}

/// The terminal's input back to lines, when the keys are done with it.
fn give_back(c: &impl Calls, modes: &Modes) {
    modes.taken.store(false, Ordering::Relaxed);
    if let Some((before, _)) = modes.saved.get() {
        c.set(before);
    }
}

/// The terminal as the run found it: its input back to lines, no progress in its tab. Only
/// async-signal-safe calls, as the signal handlers call it.
fn leave(c: &impl Calls, modes: &Modes, tab: &AtomicBool) {
    if let (true, Some((before, _))) = (modes.taken.load(Ordering::Relaxed), modes.saved.get()) {
        c.set(before);
    }
    if tab.load(Ordering::Relaxed) {
        c.write_stderr(super::TAB_CLEARED.as_bytes());
    }
}

/// The run's settings again, when a stopped run continues; its tab is drawn at the next tick.
fn resume(c: &impl Calls, modes: &Modes) {
    if let (true, Some((_, during))) = (modes.taken.load(Ordering::Relaxed), modes.saved.get()) {
        c.set(during);
    }
}

/// Ctrl-C, a kill or Ctrl-Z: the terminal as it was, then the signal's own action; and the run's
/// settings again when it continues.
fn on_signal(c: &impl Calls, modes: &Modes, tab: &AtomicBool, signum: c_int) {
    if signum == SIGCONT {
        resume(c, modes);
        return;
    }
    leave(c, modes, tab);
    // The default action is the one this process had: mutastic sets no other for these signals.
    c.signal(signum, Handler::Default);
    c.raise(signum);
    if signum == SIGTSTP {
        // Continued: listen again for the next Ctrl-Z.
        c.signal(SIGTSTP, Handler::Leave);
        resume(c, modes);
    }
}

/// The calls themselves, each in a block of its own that says why it's sound.
#[allow(
    unsafe_code,
    reason = "the terminal's size, its input mode and signals are only the C library's to give"
)]
mod ffi {
    use super::{Calls, Handler, Termios, Winsize, SIG_DFL, TCSANOW, TIOCGWINSZ};
    use std::os::raw::{c_int, c_ulong};

    extern "C" {
        fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
        fn signal(signum: c_int, handler: usize) -> usize;
        fn raise(signum: c_int) -> c_int;
        fn tcgetattr(fd: c_int, termios: *mut Termios) -> c_int;
        fn tcsetattr(fd: c_int, when: c_int, termios: *const Termios) -> c_int;
        fn write(fd: c_int, buf: *const std::ffi::c_void, count: usize) -> isize;
    }

    extern "C" fn on_ask(_: c_int) {
        super::on_ask();
    }

    extern "C" fn on_leave(signum: c_int) {
        super::on_leave(signum);
    }

    pub struct Libc;

    impl Calls for Libc {
        fn columns(&self) -> Option<usize> {
            let mut size = Winsize {
                rows: 0,
                columns: 0,
                x_pixels: 0,
                y_pixels: 0,
            };
            // SAFETY: TIOCGWINSZ writes one `struct winsize`, four unsigned shorts, which
            // `Winsize` lays out as C does (the tests check it), into memory this frame owns; on
            // a descriptor that isn't a terminal it fails and writes nothing.
            let status = unsafe { ioctl(2, TIOCGWINSZ, &mut size as *mut Winsize) };
            (status == 0).then_some(usize::from(size.columns))
        }

        fn signal(&self, signum: c_int, handler: Handler) {
            let handler = match handler {
                Handler::Ask => on_ask as extern "C" fn(c_int) as usize,
                Handler::Leave => on_leave as extern "C" fn(c_int) as usize,
                Handler::Default => SIG_DFL,
            };
            // SAFETY: `on_ask` and `on_leave` are `extern "C" fn(c_int)`, the handler's type, and
            // make only async-signal-safe calls: atomic loads and stores, a `OnceLock`'s `get`,
            // which is one, and the calls in this impl, which are each async-signal-safe.
            // SIG_DFL is each signal's default. signal is async-signal-safe, so the handlers
            // call it too. Replacing these signals' handlers is ours to do: nothing else in
            // mutastic handles them.
            unsafe { signal(signum, handler) };
        }

        fn raise(&self, signum: c_int) {
            // SAFETY: raise takes any signal number, failing on one that isn't, and is
            // async-signal-safe.
            unsafe { raise(signum) };
        }

        fn settings(&self) -> Option<Termios> {
            let mut settings = std::mem::MaybeUninit::<Termios>::uninit();
            // SAFETY: tcgetattr writes a whole `struct termios`, which `Termios` lays out as this
            // C library does (the tests check it), into memory this frame owns, when stdin is a
            // terminal; otherwise it fails.
            let status = unsafe { tcgetattr(0, settings.as_mut_ptr()) };
            if status != 0 {
                return None;
            }
            // SAFETY: tcgetattr returned 0, so it filled every byte of `settings`.
            Some(unsafe { settings.assume_init() })
        }

        fn set(&self, settings: &Termios) {
            // SAFETY: `settings` is a whole `struct termios` as this C library lays it out, read
            // by tcgetattr; tcsetattr only reads it. tcsetattr is async-signal-safe, so the
            // signal handlers call it too.
            unsafe { tcsetattr(0, TCSANOW, settings) };
        }

        fn write_stderr(&self, bytes: &[u8]) {
            // SAFETY: write reads `bytes.len()` bytes from `bytes`, which outlives the call, and
            // is async-signal-safe.
            unsafe { write(2, bytes.as_ptr().cast(), bytes.len()) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of, size_of};
    use std::sync::Mutex;

    /// Settings as a terminal reading lines has them, echoing.
    fn lines() -> Termios {
        #[cfg(target_os = "linux")]
        let mut settings = Termios {
            iflag: 0,
            oflag: 0,
            cflag: 0,
            lflag: 0,
            line: 0,
            cc: [0; 32],
            ispeed: 0,
            ospeed: 0,
        };
        #[cfg(target_os = "android")]
        let mut settings = Termios {
            iflag: 0,
            oflag: 0,
            cflag: 0,
            lflag: 0,
            line: 0,
            cc: [0; 19],
        };
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let mut settings = Termios {
            iflag: 0,
            oflag: 0,
            cflag: 0,
            lflag: 0,
            cc: [0; 20],
            ispeed: 0,
            ospeed: 0,
        };
        settings.iflag = 0o1;
        settings.lflag = ICANON | ECHO | 0x4000_0000;
        settings.cc[VMIN] = 1;
        settings.cc[VTIME] = 0;
        settings.cc[0] = 3;
        settings
    }

    /// What a call was asked, in order.
    #[derive(Debug, PartialEq, Eq)]
    enum Call {
        Signal(c_int, Handler),
        Raise(c_int),
        Settings,
        Set(Flags, u8, u8),
        Write(Vec<u8>),
    }

    /// The C library, played: answers as a terminal would, or as no terminal, and keeps what it
    /// was asked.
    struct Fake {
        terminal: bool,
        calls: Mutex<Vec<Call>>,
    }

    impl Fake {
        fn new(terminal: bool) -> Fake {
            Fake {
                terminal,
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<Call> {
            std::mem::take(&mut self.calls.lock().unwrap())
        }

        fn call(&self, call: Call) {
            self.calls.lock().unwrap().push(call);
        }
    }

    impl Calls for Fake {
        fn columns(&self) -> Option<usize> {
            self.terminal.then_some(80)
        }
        fn signal(&self, signum: c_int, handler: Handler) {
            self.call(Call::Signal(signum, handler));
        }
        fn raise(&self, signum: c_int) {
            self.call(Call::Raise(signum));
        }
        fn settings(&self) -> Option<Termios> {
            self.call(Call::Settings);
            self.terminal.then(lines)
        }
        fn set(&self, settings: &Termios) {
            self.call(Call::Set(
                settings.lflag,
                settings.cc[VMIN],
                settings.cc[VTIME],
            ));
        }
        fn write_stderr(&self, bytes: &[u8]) {
            self.call(Call::Write(bytes.to_vec()));
        }
    }

    const LINES: Call = Call::Set(ICANON | ECHO | 0x4000_0000, 1, 0);
    const KEYS: Call = Call::Set(0x4000_0000, 0, 1);

    #[test]
    fn keys_are_read_one_at_a_time_unechoed_and_the_rest_is_kept() {
        let during = keys_mode(lines());
        assert_eq!(during.lflag, 0x4000_0000);
        assert_eq!((during.cc[VMIN], during.cc[VTIME]), (0, 1));
        assert_eq!(during.iflag, 0o1);
        assert_eq!(during.cc[0], 3);
    }

    #[test]
    fn the_signals_that_ask_whats_running() {
        let c = Fake::new(true);
        listen(&c);
        let mut expected = vec![Call::Signal(SIGUSR1, Handler::Ask)];
        if let Some(info) = SIGINFO {
            expected.push(Call::Signal(info, Handler::Ask));
        }
        assert_eq!(c.calls(), expected);
    }

    #[test]
    fn the_guard_takes_each_way_out_and_gives_it_back() {
        let c = Fake::new(true);
        guard_with(&c, true);
        let signals = [SIGINT, SIGTERM, SIGTSTP, SIGCONT];
        let to = |handler| Vec::from(signals.map(|signum| Call::Signal(signum, handler)));
        assert_eq!(c.calls(), to(Handler::Leave));
        guard_with(&c, false);
        assert_eq!(c.calls(), to(Handler::Default));
    }

    #[test]
    fn taking_the_terminal_keeps_its_settings_and_giving_it_back_restores_them() {
        let c = Fake::new(true);
        let modes = Modes::new();
        assert!(take(&c, &modes));
        assert_eq!(c.calls(), [Call::Settings, KEYS]);
        assert!(modes.taken.load(Ordering::Relaxed));
        give_back(&c, &modes);
        assert_eq!(c.calls(), [LINES]);
        assert!(!modes.taken.load(Ordering::Relaxed));
        // Once in a process: the settings kept are the first ones.
        assert!(!take(&c, &modes));
        assert_eq!(c.calls(), [Call::Settings]);
    }

    #[test]
    fn no_terminal_is_taken_off_one() {
        let c = Fake::new(false);
        let modes = Modes::new();
        assert!(!take(&c, &modes));
        assert_eq!(c.calls(), [Call::Settings]);
        assert!(modes.saved.get().is_none());
        give_back(&c, &modes);
        assert_eq!(c.calls(), []);
    }

    #[test]
    fn ctrl_c_restores_the_terminal_and_clears_the_tab_then_acts() {
        let c = Fake::new(true);
        let modes = Modes::new();
        let tab = AtomicBool::new(true);
        take(&c, &modes);
        c.calls();
        for signum in [SIGINT, SIGTERM] {
            on_signal(&c, &modes, &tab, signum);
            assert_eq!(
                c.calls(),
                [
                    LINES,
                    Call::Write(crate::TAB_CLEARED.as_bytes().to_vec()),
                    Call::Signal(signum, Handler::Default),
                    Call::Raise(signum),
                ]
            );
        }
    }

    #[test]
    fn a_way_out_touches_only_what_the_run_changed() {
        let c = Fake::new(true);
        let modes = Modes::new();
        let tab = AtomicBool::new(false);
        on_signal(&c, &modes, &tab, SIGINT);
        assert_eq!(
            c.calls(),
            [Call::Signal(SIGINT, Handler::Default), Call::Raise(SIGINT)]
        );
        // Given back already: the terminal is in lines, and stays so.
        take(&c, &modes);
        give_back(&c, &modes);
        c.calls();
        on_signal(&c, &modes, &tab, SIGTERM);
        assert_eq!(
            c.calls(),
            [
                Call::Signal(SIGTERM, Handler::Default),
                Call::Raise(SIGTERM)
            ]
        );
    }

    #[test]
    fn ctrl_z_restores_the_terminal_stops_and_takes_it_again_on_continuing() {
        let c = Fake::new(true);
        let modes = Modes::new();
        let tab = AtomicBool::new(false);
        take(&c, &modes);
        c.calls();
        on_signal(&c, &modes, &tab, SIGTSTP);
        assert_eq!(
            c.calls(),
            [
                LINES,
                Call::Signal(SIGTSTP, Handler::Default),
                Call::Raise(SIGTSTP),
                Call::Signal(SIGTSTP, Handler::Leave),
                KEYS,
            ]
        );
        // Continued by `fg` or `bg`, rather than raised here: only the run's settings again.
        on_signal(&c, &modes, &tab, SIGCONT);
        assert_eq!(c.calls(), [KEYS]);
        give_back(&c, &modes);
        c.calls();
        on_signal(&c, &modes, &tab, SIGCONT);
        assert_eq!(c.calls(), []);
    }

    #[test]
    fn the_width_is_what_the_call_says() {
        assert_eq!(Fake::new(true).columns(), Some(80));
        assert_eq!(Fake::new(false).columns(), None);
    }

    // What Miri can't check: that each struct and constant declared for C is the C library's.
    // These hold them to the `libc` crate's, on each platform the tests run on.

    #[test]
    fn winsize_is_laid_out_as_c_does() {
        assert_eq!(size_of::<Winsize>(), size_of::<libc::winsize>());
        assert_eq!(align_of::<Winsize>(), align_of::<libc::winsize>());
        assert_eq!(offset_of!(Winsize, rows), offset_of!(libc::winsize, ws_row));
        assert_eq!(
            offset_of!(Winsize, columns),
            offset_of!(libc::winsize, ws_col)
        );
        assert_eq!(
            offset_of!(Winsize, x_pixels),
            offset_of!(libc::winsize, ws_xpixel)
        );
        assert_eq!(
            offset_of!(Winsize, y_pixels),
            offset_of!(libc::winsize, ws_ypixel)
        );
    }

    #[test]
    fn termios_is_laid_out_as_c_does() {
        assert_eq!(size_of::<Termios>(), size_of::<libc::termios>());
        assert_eq!(align_of::<Termios>(), align_of::<libc::termios>());
        assert_eq!(
            offset_of!(Termios, iflag),
            offset_of!(libc::termios, c_iflag)
        );
        assert_eq!(
            offset_of!(Termios, oflag),
            offset_of!(libc::termios, c_oflag)
        );
        assert_eq!(
            offset_of!(Termios, cflag),
            offset_of!(libc::termios, c_cflag)
        );
        assert_eq!(
            offset_of!(Termios, lflag),
            offset_of!(libc::termios, c_lflag)
        );
        assert_eq!(offset_of!(Termios, cc), offset_of!(libc::termios, c_cc));
        assert_eq!(size_of::<Flags>(), size_of::<libc::tcflag_t>());
        assert_eq!(lines().cc.len(), libc::NCCS);
        #[cfg(any(target_os = "linux", target_os = "android"))]
        assert_eq!(offset_of!(Termios, line), offset_of!(libc::termios, c_line));
        #[cfg(not(target_os = "android"))]
        {
            assert_eq!(
                offset_of!(Termios, ispeed),
                offset_of!(libc::termios, c_ispeed)
            );
            assert_eq!(
                offset_of!(Termios, ospeed),
                offset_of!(libc::termios, c_ospeed)
            );
        }
    }

    #[test]
    fn each_constant_is_the_c_librarys() {
        #[allow(
            clippy::unnecessary_cast,
            reason = "TIOCGWINSZ's type is c_ulong in some C libraries and c_int in others"
        )]
        let tiocgwinsz = libc::TIOCGWINSZ as c_ulong;
        assert_eq!(TIOCGWINSZ, tiocgwinsz);
        assert_eq!(SIGUSR1, libc::SIGUSR1);
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        assert_eq!(SIGINFO, Some(libc::SIGINFO));
        assert_eq!(SIGINT, libc::SIGINT);
        assert_eq!(SIGTERM, libc::SIGTERM);
        assert_eq!(SIGTSTP, libc::SIGTSTP);
        assert_eq!(SIGCONT, libc::SIGCONT);
        assert_eq!(SIG_DFL, libc::SIG_DFL);
        assert_eq!(TCSANOW, libc::TCSANOW);
        assert_eq!(ICANON, libc::ICANON);
        assert_eq!(ECHO, libc::ECHO);
        assert_eq!(VMIN, libc::VMIN);
        assert_eq!(VTIME, libc::VTIME);
    }

    // The real calls, which Miri can't make; AddressSanitizer runs them, on a terminal.

    #[test]
    #[cfg_attr(miri, ignore = "calls the C library")]
    fn the_c_library_gives_a_width_only_for_a_terminal() {
        use std::io::IsTerminal as _;
        assert_eq!(width().is_some(), std::io::stderr().is_terminal());
    }

    #[test]
    #[cfg_attr(miri, ignore = "calls the C library")]
    fn a_signal_asks_whats_running() {
        let _flags = crate::tests::FLAGS.lock().unwrap();
        listen_for_signals();
        C.raise(SIGUSR1);
        assert!(crate::asked());
        assert!(!crate::asked());
    }

    #[test]
    #[cfg_attr(miri, ignore = "calls the C library")]
    fn the_terminal_is_taken_and_given_back() {
        use std::io::IsTerminal as _;
        let Some(before) = C.settings() else {
            assert!(!std::io::stdin().is_terminal());
            assert!(Held::take().is_none());
            return;
        };
        let held = Held::take().unwrap();
        let during = C.settings().unwrap();
        assert_eq!(during.lflag & (ICANON | ECHO), 0);
        assert_eq!((during.cc[VMIN], during.cc[VTIME]), (0, 1));
        // A continue while the guard is on takes the run's settings again: they're the
        // terminal's already.
        let guard = crate::Guard::take();
        C.raise(SIGCONT);
        drop(guard);
        assert_eq!(C.settings().unwrap().lflag, during.lflag);
        drop(held);
        // What the run changed is as it was. The driver may set other flags of its own, as
        // macOS sets PENDIN on leaving a key at a time.
        let after = C.settings().unwrap();
        let changed = ICANON | ECHO;
        assert_eq!(after.lflag & changed, before.lflag & changed);
        assert_eq!(
            (after.cc[VMIN], after.cc[VTIME]),
            (before.cc[VMIN], before.cc[VTIME])
        );
    }

    #[test]
    #[cfg_attr(miri, ignore = "calls the C library")]
    fn writing_nothing_to_stderr_is_harmless() {
        C.write_stderr(b"");
    }
}
