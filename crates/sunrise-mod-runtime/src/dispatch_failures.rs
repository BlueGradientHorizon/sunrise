//! Thread-owned synchronous guest-dispatch failure fence shared by engine and loader.
thread_local! {
    static DISPATCH_FAILURE_GENERATION: std::rc::Rc<std::cell::Cell<u64>> =
        std::rc::Rc::new(std::cell::Cell::new(0));
}

/// Capture once when constructing the engine's thread-owned state. A changed
/// value means a guest call or response decode failed on this thread; preceding
/// host effects must not be interpreted as an accepted guest response.
#[doc(hidden)]
pub fn dispatch_failure_counter() -> std::rc::Rc<std::cell::Cell<u64>> {
    DISPATCH_FAILURE_GENERATION.with(std::rc::Rc::clone)
}

/// Mark the failed synchronous dispatch on the same thread as its host effects.
#[doc(hidden)]
pub fn record_dispatch_failure() {
    DISPATCH_FAILURE_GENERATION.with(|generation| {
        generation.set(generation.get().wrapping_add(1));
    });
}

thread_local! {
    static ACTION_FIRE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Runs a decided action's listeners with the same unwind policy for native and wasm.
#[doc(hidden)]
pub fn with_action_fire<R>(fire: impl FnOnce() -> R) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTION_FIRE.set(self.0);
        }
    }
    let _restore = Restore(ACTION_FIRE.replace(true));
    fire()
}

/// A guest listener with no answer must unwind to the decision coordinator in this scope.
#[doc(hidden)]
pub fn action_fire_active() -> bool {
    ACTION_FIRE.get()
}
