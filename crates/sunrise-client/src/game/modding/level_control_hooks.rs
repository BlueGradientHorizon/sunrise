//! While dispatching a tick-rate notification, refuse another tick-rate write
//! so it cannot recursively fire the hook.
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum LevelControl {
    TickRate,
}

thread_local! {
    static HEARING: Cell<u8> = const { Cell::new(0) };
}

impl LevelControl {
    const fn bit(self) -> u8 {
        1 << self as u8
    }

    /// Whether this control's hook is being heard on this thread.
    pub(in crate::game) fn is_heard(self) -> bool {
        HEARING.with(|h| h.get() & self.bit() != 0)
    }

    /// Runs `f`, a dispatch of this control's hook, with the control marked as heard until it
    /// returns or unwinds.
    pub(in crate::game) fn hear<R>(self, f: impl FnOnce() -> R) -> R {
        struct Heard(u8);
        impl Drop for Heard {
            fn drop(&mut self) {
                HEARING.with(|h| h.set(self.0));
            }
        }
        let _restore = Heard(HEARING.with(|h| h.replace(h.get() | self.bit())));
        f()
    }
}
