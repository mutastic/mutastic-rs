//! What only the operating system can say about the terminal, and the one key and signal a run
//! listens for: the terminal's width, so a live block fits it; `t`, Ctrl-T (SIGINFO, on macOS and
//! the BSDs) or SIGUSR1, for what's running now.
//!
//! This is the one module allowed unsafe code. It calls the C library directly rather than take a
//! dependency for a handful of calls, and each `unsafe` block says why it's sound. Everything here
//! degrades to nothing: no width, no key, no signal, never an error.

#![allow(
    unsafe_code,
    reason = "the terminal's size, its input mode and signals are only the C library's to give"
)]

use std::sync::atomic::{AtomicBool, Ordering};

/// Set when someone asks what's running: `t`, Ctrl-T or SIGUSR1. The progress ticker takes it.
static ASKED: AtomicBool = AtomicBool::new(false);

/// Whether someone asked what's running since the last time this was called.
pub fn asked() -> bool {
    ASKED.swap(false, Ordering::Relaxed)
}

/// stderr's width in columns, when it's a terminal that says.
pub fn width() -> Option<usize> {
    sys::width().filter(|&columns| columns > 0)
}

/// Listens for Ctrl-T (SIGINFO) and SIGUSR1, which ask what's running.
pub fn listen_for_signals() {
    sys::listen_for_signals();
}

/// The terminal's input, for the `t` key: read a key at a time and not echoed, while this lives.
/// Ctrl-C and Ctrl-Z still work, and leave the terminal as it was.
pub struct Keys {
    _held: Option<sys::Held>,
    stop: std::sync::Arc<AtomicBool>,
    reader: Option<std::thread::JoinHandle<()>>,
}

impl Keys {
    /// Listens for `t` when stdin is a terminal; otherwise nothing is listened for.
    pub fn listen() -> Keys {
        use std::io::IsTerminal as _;
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        if !std::io::stdin().is_terminal() {
            return Keys {
                _held: None,
                stop,
                reader: None,
            };
        }
        let held = sys::Held::take();
        let stopping = std::sync::Arc::clone(&stop);
        let reader = held.is_some().then(|| {
            std::thread::spawn(move || {
                while !stopping.load(Ordering::Relaxed) {
                    if sys::key() == Some(b't') {
                        ASKED.store(true, Ordering::Relaxed);
                    }
                }
            })
        });
        Keys {
            _held: held,
            stop,
            reader,
        }
    }
}

impl Keys {
    /// Whether `t` is being listened for.
    pub fn listening(&self) -> bool {
        self.reader.is_some()
    }
}

impl Drop for Keys {
    /// Stops reading before the terminal goes back to lines, so no key typed after the run is
    /// taken.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[cfg(unix)]
mod sys {
    use std::os::raw::{c_int, c_ulong, c_ushort};
    use std::sync::atomic::Ordering;
    use std::sync::OnceLock;

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

    extern "C" {
        fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
        fn signal(signum: c_int, handler: usize) -> usize;
        fn raise(signum: c_int) -> c_int;
        fn tcgetattr(fd: c_int, termios: *mut Termios) -> c_int;
        fn tcsetattr(fd: c_int, when: c_int, termios: *const Termios) -> c_int;
    }

    pub fn width() -> Option<usize> {
        let mut size = Winsize {
            rows: 0,
            columns: 0,
            x_pixels: 0,
            y_pixels: 0,
        };
        // SAFETY: TIOCGWINSZ writes one `struct winsize`, four unsigned shorts, which `Winsize`
        // lays out as C does, into memory this frame owns; on a descriptor that isn't a terminal
        // it fails and writes nothing.
        let status = unsafe { ioctl(2, TIOCGWINSZ, &mut size as *mut Winsize) };
        (status == 0).then_some(usize::from(size.columns))
    }

    extern "C" fn on_ask(_: c_int) {
        // Only an atomic store: all a signal handler may safely do here.
        super::ASKED.store(true, Ordering::Relaxed);
    }

    pub fn listen_for_signals() {
        for signum in [Some(SIGUSR1), SIGINFO].into_iter().flatten() {
            // SAFETY: `on_ask` is an `extern "C" fn(c_int)`, the handler's type, and does only an
            // atomic store, which is async-signal-safe. Replacing these signals' handlers is ours
            // to do: nothing else in mutastic handles them.
            unsafe { signal(signum, on_ask as extern "C" fn(c_int) as usize) };
        }
    }

    /// `struct termios`, as each C library lays it out.
    #[cfg(any(target_os = "linux", target_os = "android"))]
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
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const ICANON: u32 = 0o2;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const ECHO: u32 = 0o10;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const VTIME: usize = 5;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const VMIN: usize = 6;

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Termios {
        iflag: c_ulong,
        oflag: c_ulong,
        cflag: c_ulong,
        lflag: c_ulong,
        cc: [u8; 20],
        ispeed: c_ulong,
        ospeed: c_ulong,
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    const ICANON: c_ulong = 0x100;
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    const ECHO: c_ulong = 0x8;
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    const VMIN: usize = 16;
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    const VTIME: usize = 17;

    const TCSANOW: c_int = 0;

    /// The terminal's settings before the run took it, and the run's own, for every way back.
    static SAVED: OnceLock<(Termios, Termios)> = OnceLock::new();

    fn set(termios: &Termios) {
        // SAFETY: `termios` is a whole `struct termios` as this C library lays it out, read by
        // tcgetattr; tcsetattr only reads it. tcsetattr is async-signal-safe, so this is also
        // called from the signal handlers below.
        unsafe { tcsetattr(0, TCSANOW, termios) };
    }

    /// The terminal as it was, then the signal's own action, for Ctrl-C, a kill or Ctrl-Z; and the
    /// run's settings again when a stopped run continues.
    extern "C" fn on_leave(signum: c_int) {
        let Some((before, during)) = SAVED.get() else {
            return;
        };
        if signum == SIGCONT {
            set(during);
            return;
        }
        set(before);
        // SAFETY: signal and raise are async-signal-safe. The default action is the one this
        // process had: mutastic sets no other for these signals.
        unsafe {
            signal(signum, SIG_DFL);
            raise(signum);
        }
        if signum == SIGTSTP {
            // Continued: listen again for the next Ctrl-Z.
            // SAFETY: as when it was first set, in `Held::take`.
            unsafe { signal(SIGTSTP, on_leave as extern "C" fn(c_int) as usize) };
            set(during);
        }
    }

    /// The terminal reading a key at a time, without echo, until this is dropped.
    pub struct Held;

    impl Held {
        pub fn take() -> Option<Held> {
            let mut before = std::mem::MaybeUninit::<Termios>::uninit();
            // SAFETY: tcgetattr fills a whole `struct termios`, which `Termios` lays out as this C
            // library does, when it returns 0; it's read only then.
            let before = unsafe {
                if tcgetattr(0, before.as_mut_ptr()) != 0 {
                    return None;
                }
                before.assume_init()
            };
            let mut during = before;
            during.lflag &= !(ICANON | ECHO);
            // A read returns after a tenth of a second with nothing, so the reader can stop.
            during.cc[VMIN] = 0;
            during.cc[VTIME] = 1;
            if SAVED.set((before, during)).is_err() {
                return None;
            }
            for signum in [SIGINT, SIGTERM, SIGTSTP, SIGCONT] {
                // SAFETY: `on_leave` is an `extern "C" fn(c_int)` that does only async-signal-safe
                // calls; mutastic handles none of these signals itself.
                unsafe { signal(signum, on_leave as extern "C" fn(c_int) as usize) };
            }
            set(&during);
            Some(Held)
        }
    }

    impl Drop for Held {
        fn drop(&mut self) {
            if let Some((before, _)) = SAVED.get() {
                set(before);
            }
            for signum in [SIGINT, SIGTERM, SIGTSTP, SIGCONT] {
                // SAFETY: back to each signal's default action, as before the run took them.
                unsafe { signal(signum, SIG_DFL) };
            }
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
}

#[cfg(windows)]
mod sys {
    use std::os::raw::{c_int, c_short, c_ushort, c_void};

    #[repr(C)]
    struct Coord {
        x: c_short,
        y: c_short,
    }
    #[repr(C)]
    struct SmallRect {
        left: c_short,
        top: c_short,
        right: c_short,
        bottom: c_short,
    }
    #[repr(C)]
    struct ScreenBufferInfo {
        size: Coord,
        cursor: Coord,
        attributes: c_ushort,
        window: SmallRect,
        maximum: Coord,
    }
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetStdHandle(which: u32) -> *mut c_void;
        fn GetConsoleScreenBufferInfo(console: *mut c_void, info: *mut ScreenBufferInfo) -> c_int;
    }
    extern "C" {
        fn _kbhit() -> c_int;
        fn _getwch() -> u16;
    }

    pub fn width() -> Option<usize> {
        let mut info = std::mem::MaybeUninit::<ScreenBufferInfo>::uninit();
        // SAFETY: GetStdHandle returns a handle or null, which GetConsoleScreenBufferInfo
        // refuses; it fills a whole CONSOLE_SCREEN_BUFFER_INFO, laid out as `ScreenBufferInfo`,
        // when it returns nonzero, and it's read only then.
        let info = unsafe {
            if GetConsoleScreenBufferInfo(GetStdHandle(STD_ERROR_HANDLE), info.as_mut_ptr()) == 0 {
                return None;
            }
            info.assume_init()
        };
        usize::try_from(info.window.right - info.window.left + 1).ok()
    }

    /// Windows has no Ctrl-T; `t` still asks.
    pub fn listen_for_signals() {}

    /// The console needs no mode for `_kbhit`: nothing to hold.
    pub struct Held;

    impl Held {
        pub fn take() -> Option<Held> {
            Some(Held)
        }
    }

    pub fn key() -> Option<u8> {
        // SAFETY: the C runtime's console calls take no arguments; `_getwch` is called only when
        // `_kbhit` says a key is waiting, so it doesn't block.
        let key = unsafe { (_kbhit() != 0).then(|| _getwch()) };
        if key.is_none() {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        key.and_then(|key| u8::try_from(key).ok())
    }
}

#[cfg(not(any(unix, windows)))]
mod sys {
    pub fn width() -> Option<usize> {
        None
    }
    pub fn listen_for_signals() {}
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
