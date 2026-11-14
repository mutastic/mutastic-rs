//! The console on Windows: its width from `GetConsoleScreenBufferInfo`, Ctrl-C from
//! `SetConsoleCtrlHandler`, and keys from the C runtime's `_kbhit` and `_getwch`. The calls are in
//! `ffi`, behind `Calls`; what's made of them is logic over `Calls`.

use std::os::raw::{c_short, c_ushort};
use std::sync::atomic::{AtomicBool, Ordering};

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

/// The calls, one each. Miri can't call them, so everything else is written over this, and the
/// tests give it a fake.
trait Calls: Sync {
    /// The part of stderr's console buffer the window shows, when stderr is a console.
    fn window(&self) -> Option<SmallRect>;
    /// `on_leave` added to the console's Ctrl-C handlers, or taken off.
    fn handle_ctrl_c(&self, on: bool);
    /// Whether a key is waiting, from `_kbhit`.
    fn key_waiting(&self) -> bool;
    /// The key waiting, from `_getwch`.
    fn waiting_key(&self) -> u16;
}

/// The calls themselves.
static C: ffi::Win32 = ffi::Win32;

pub fn width() -> Option<usize> {
    C.window().and_then(columns)
}

/// The window's width, from its edges, which are both in it.
fn columns(window: SmallRect) -> Option<usize> {
    usize::try_from(window.right - window.left + 1).ok()
}

/// Windows has no Ctrl-T; `t` still asks.
pub fn listen_for_signals() {}

pub fn guard(on: bool) {
    C.handle_ctrl_c(on);
}

/// The console needs no mode for `_kbhit`: nothing to hold.
pub struct Held;

impl Held {
    pub fn take() -> Option<Held> {
        Some(Held)
    }
}

pub fn key() -> Option<u8> {
    key_from(&C)
}

/// A key, or `None` after a tenth of a second with none, as on a Unix terminal.
fn key_from(c: &impl Calls) -> Option<u8> {
    let key = c.key_waiting().then(|| c.waiting_key());
    if key.is_none() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    key.and_then(|key| u8::try_from(key).ok())
}

/// Ctrl-C or the console closing: the tab without progress, then Windows's own action. It runs
/// on a thread of its own, not in a signal, so it can write as any thread does.
fn leave(tab: &AtomicBool, stderr: &mut impl std::io::Write) {
    if tab.load(Ordering::Relaxed) {
        let _ = stderr.write_all(super::TAB_CLEARED.as_bytes());
    }
}

/// The calls themselves, each in a block of its own that says why it's sound.
#[allow(
    unsafe_code,
    reason = "the console's size and its Ctrl-C handler are only Windows's to give"
)]
mod ffi {
    use super::{Calls, ScreenBufferInfo, SmallRect, STD_ERROR_HANDLE};
    use std::os::raw::{c_int, c_void};

    #[link(name = "kernel32")]
    extern "system" {
        fn GetStdHandle(which: u32) -> *mut c_void;
        fn GetConsoleScreenBufferInfo(console: *mut c_void, info: *mut ScreenBufferInfo) -> c_int;
        fn SetConsoleCtrlHandler(
            handler: Option<extern "system" fn(u32) -> c_int>,
            add: c_int,
        ) -> c_int;
    }
    extern "C" {
        fn _kbhit() -> c_int;
        fn _getwch() -> u16;
    }

    /// Returns FALSE, so the next handler, the default, still ends the process.
    extern "system" fn on_leave(_: u32) -> c_int {
        super::leave(&crate::TAB, &mut std::io::stderr());
        0
    }

    pub struct Win32;

    impl Calls for Win32 {
        fn window(&self) -> Option<SmallRect> {
            // SAFETY: GetStdHandle takes a standard device's number and returns its handle, or
            // null or INVALID_HANDLE_VALUE, which the next call refuses.
            let stderr = unsafe { GetStdHandle(STD_ERROR_HANDLE) };
            let mut info = std::mem::MaybeUninit::<ScreenBufferInfo>::uninit();
            // SAFETY: GetConsoleScreenBufferInfo fills a whole CONSOLE_SCREEN_BUFFER_INFO,
            // which `ScreenBufferInfo` lays out as Windows does (the tests check it), in memory
            // this frame owns, when it returns nonzero; otherwise it fails.
            let status = unsafe { GetConsoleScreenBufferInfo(stderr, info.as_mut_ptr()) };
            if status == 0 {
                return None;
            }
            // SAFETY: GetConsoleScreenBufferInfo returned nonzero, so it filled all of `info`.
            Some(unsafe { info.assume_init() }.window)
        }

        fn handle_ctrl_c(&self, on: bool) {
            // SAFETY: `on_leave` has the handler's type, and returns FALSE, so the next
            // handler, the default, still ends the process.
            unsafe { SetConsoleCtrlHandler(Some(on_leave), c_int::from(on)) };
        }

        fn key_waiting(&self) -> bool {
            // SAFETY: the C runtime's `_kbhit` takes no arguments and only looks.
            unsafe { _kbhit() != 0 }
        }

        fn waiting_key(&self) -> u16 {
            // SAFETY: `_getwch` takes no arguments; it's called only when `_kbhit` says a key is
            // waiting, so it doesn't block.
            unsafe { _getwch() }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of, size_of};
    use windows_sys::Win32::System::Console as console;

    /// A console with a key waiting, or none.
    struct Fake(Option<u16>);

    impl Calls for Fake {
        fn window(&self) -> Option<SmallRect> {
            None
        }
        fn handle_ctrl_c(&self, _: bool) {}
        fn key_waiting(&self) -> bool {
            self.0.is_some()
        }
        fn waiting_key(&self) -> u16 {
            self.0.expect("a key was read when none was waiting")
        }
    }

    #[test]
    fn a_key_waiting_is_read() {
        assert_eq!(key_from(&Fake(Some(u16::from(b't')))), Some(b't'));
    }

    #[test]
    fn no_key_waiting_is_none_and_isnt_read() {
        assert_eq!(key_from(&Fake(None)), None);
    }

    #[test]
    fn a_key_past_a_byte_is_none() {
        assert_eq!(key_from(&Fake(Some(0x0442))), None);
    }

    #[test]
    fn the_width_counts_both_edges() {
        let window = |left, right| SmallRect {
            left,
            top: 0,
            right,
            bottom: 29,
        };
        assert_eq!(columns(window(0, 119)), Some(120));
        assert_eq!(columns(window(10, 10)), Some(1));
        assert_eq!(columns(window(5, 3)), None);
    }

    #[test]
    fn the_tab_is_cleared_only_when_it_shows_progress() {
        let mut written = Vec::new();
        leave(&AtomicBool::new(false), &mut written);
        assert!(written.is_empty());
        leave(&AtomicBool::new(true), &mut written);
        assert_eq!(written, crate::TAB_CLEARED.as_bytes());
    }

    // What Miri can't check: that each struct and constant declared for Windows is Windows's.

    #[test]
    fn the_structs_are_laid_out_as_windows_does() {
        assert_eq!(size_of::<Coord>(), size_of::<console::COORD>());
        assert_eq!(align_of::<Coord>(), align_of::<console::COORD>());
        assert_eq!(offset_of!(Coord, x), offset_of!(console::COORD, X));
        assert_eq!(offset_of!(Coord, y), offset_of!(console::COORD, Y));

        assert_eq!(size_of::<SmallRect>(), size_of::<console::SMALL_RECT>());
        assert_eq!(align_of::<SmallRect>(), align_of::<console::SMALL_RECT>());
        assert_eq!(
            offset_of!(SmallRect, left),
            offset_of!(console::SMALL_RECT, Left)
        );
        assert_eq!(
            offset_of!(SmallRect, top),
            offset_of!(console::SMALL_RECT, Top)
        );
        assert_eq!(
            offset_of!(SmallRect, right),
            offset_of!(console::SMALL_RECT, Right)
        );
        assert_eq!(
            offset_of!(SmallRect, bottom),
            offset_of!(console::SMALL_RECT, Bottom)
        );

        type Info = console::CONSOLE_SCREEN_BUFFER_INFO;
        assert_eq!(size_of::<ScreenBufferInfo>(), size_of::<Info>());
        assert_eq!(align_of::<ScreenBufferInfo>(), align_of::<Info>());
        assert_eq!(offset_of!(ScreenBufferInfo, size), offset_of!(Info, dwSize));
        assert_eq!(
            offset_of!(ScreenBufferInfo, cursor),
            offset_of!(Info, dwCursorPosition)
        );
        assert_eq!(
            offset_of!(ScreenBufferInfo, attributes),
            offset_of!(Info, wAttributes)
        );
        assert_eq!(
            offset_of!(ScreenBufferInfo, window),
            offset_of!(Info, srWindow)
        );
        assert_eq!(
            offset_of!(ScreenBufferInfo, maximum),
            offset_of!(Info, dwMaximumWindowSize)
        );
    }

    #[test]
    fn each_constant_is_windowss() {
        assert_eq!(STD_ERROR_HANDLE, console::STD_ERROR_HANDLE);
    }

    // The real calls, which Miri can't make.

    #[test]
    #[cfg_attr(miri, ignore = "calls Windows")]
    fn windows_gives_a_width_only_for_a_console() {
        use std::io::IsTerminal as _;
        // Not the other way: an MSYS terminal is one to std, and not a console.
        if C.window().is_some() {
            assert!(std::io::stderr().is_terminal());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore = "calls Windows")]
    fn the_ctrl_c_handler_is_added_and_taken_off() {
        crate::Guard::take();
    }
}
