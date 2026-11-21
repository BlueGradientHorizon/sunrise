//! Owner enablement, attribution and panic containment for retained resources.
use crate::world::phase_refusal::{PhaseRefusal, contain_refusal};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::{AtomicBool, Ordering},
};

thread_local! {
    static CONTAINED: Cell<u32> = const { Cell::new(0) };
    static CAUGHT: RefCell<Option<String>> = const { RefCell::new(None) };
}

// The boundary may be monomorphized outside mod_vocab. Keep the thread-local reads and
// writes in non-inline functions so the callback and the game hook share the same slot.
struct PanicScope(Option<String>);

impl PanicScope {
    #[inline(never)]
    fn enter() -> Self {
        CONTAINED.with(|c| c.set(c.get() + 1));
        Self(CAUGHT.with(|c| c.replace(None)))
    }

    #[inline(never)]
    fn take_message(&self) -> Option<String> {
        CAUGHT.with(|c| c.borrow_mut().take())
    }
}

impl Drop for PanicScope {
    #[inline(never)]
    fn drop(&mut self) {
        CONTAINED.with(|c| c.set(c.get() - 1));
        drop(CAUGHT.with(|c| c.replace(self.0.take())));
    }
}

/// Catch a real panic and retain its diagnostic, even if dropping its payload also panics.
/// The game panic hook records its attributed message while this boundary is active.
pub fn contain_unwind<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    let scope = PanicScope::enter();
    catch_unwind(AssertUnwindSafe(f)).map_err(|payload| {
        let message = scope
            .take_message()
            .unwrap_or_else(|| panic_payload_text(&*payload));
        // An opaque payload may run mod code when dropped. Never replace the original
        // diagnostic with that destructor's panic, or try to drop a second hostile payload.
        if let Err(secondary) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
            std::mem::forget(secondary);
        }
        message
    })
}

/// Called by the game panic hook to log and record a contained panic instead of a crash.
#[inline(never)]
pub fn record_contained_panic(diagnostic: &dyn Fn() -> String) -> bool {
    if CONTAINED.with(Cell::get) == 0 {
        return false;
    }
    let message = diagnostic();
    CAUGHT.with(|c| *c.borrow_mut() = Some(message));
    true
}

/// The printable message Rust gives string payloads; other payloads are opaque.
pub fn panic_payload_text(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "Box<dyn Any>".into())
}

/// Owner identity and poison state for one retained mod callback.
// no wire: the callback's enablement and poison state belong to this process.
// exhaustive: engine-only callback bookkeeping; private fields prevent downstream construction.
pub struct CallbackOwner {
    id: &'static str,
    index: usize,
    poisoned: AtomicBool,
}

impl CallbackOwner {
    /// Bind a dispatch-attributed registration to its installed mod slot once, at freeze time.
    pub fn new(id: &'static str) -> Option<Self> {
        Some(Self::for_index(id, crate::__private::mod_index(id)?))
    }

    pub(crate) fn for_index(id: &'static str, index: usize) -> Self {
        Self {
            id,
            index,
            poisoned: AtomicBool::new(false),
        }
    }

    /// Run a delayed task using its installed owner's shared state. A refusal is logged and
    /// aborts just the transaction; only a panic returns a diagnostic for the server to relay.
    pub fn call_scheduled_task<H: crate::ServerHost>(
        id: &'static str,
        host: &mut H,
        invoke: impl FnOnce(&mut H),
    ) -> Result<(), String> {
        let state = crate::runtime::installed()
            .and_then(|installed| {
                installed
                    .by_id
                    .get(id)
                    .map(|&i| &installed.scheduled_tasks[i])
            })
            .map(|owner| (owner.index, &owner.poisoned));
        // A real panic unwinds the transaction before `run` catches it. A refusal stays
        // inside the transaction as an Err, including one the callback caught internally.
        match run(id, state, "scheduled task", "schedule_task", || {
            crate::run_transaction(host, |host, _| contain_refusal(id, || invoke(host)))
        }) {
            Err(CallbackFailure::Panicked(diagnostic)) => Err(diagnostic),
            _ => Ok(()),
        }
    }

    /// Invoke retained code with its owner's enablement, attribution and panic policy.
    pub fn call<T>(
        &self,
        kind: &'static str,
        operation: &'static str,
        invoke: impl FnOnce() -> T,
    ) -> Result<T, String> {
        call(self.id, self.index, &self.poisoned, kind, operation, invoke)
            .map_err(|why| format!("mod {}: {why}", self.id))
    }
}

pub(crate) fn enabled(index: usize, poisoned: &AtomicBool) -> bool {
    crate::mod_index_on(index) && !poisoned.load(Ordering::Acquire)
}

pub(crate) fn call<T>(
    owner: &'static str,
    index: usize,
    poisoned: &AtomicBool,
    kind: &'static str,
    operation: &'static str,
    invoke: impl FnOnce() -> T,
) -> Result<T, String> {
    run(owner, Some((index, poisoned)), kind, operation, || {
        contain_refusal(owner, invoke)
    })
    .map_err(|failure| match failure {
        CallbackFailure::Disabled => format!("{kind} owner is disabled"),
        CallbackFailure::Refused(refusal) => format!("{kind} callback refused: {refusal}"),
        CallbackFailure::Panicked(diagnostic) => {
            format!("{kind} callback panicked; it is disabled: {diagnostic}")
        }
    })
}

enum CallbackFailure {
    Disabled,
    Refused(PhaseRefusal),
    Panicked(String),
}

fn run<T>(
    owner: &'static str,
    state: Option<(usize, &AtomicBool)>,
    kind: &'static str,
    operation: &'static str,
    invoke: impl FnOnce() -> Result<T, PhaseRefusal>,
) -> Result<T, CallbackFailure> {
    let _guard = crate::mark_mod(owner, operation);
    match contain_unwind(|| {
        if state.is_some_and(|(index, poisoned)| enabled(index, poisoned)) {
            invoke().map(Some)
        } else {
            // Captures also belong to the owner if the callable is discarded without running.
            drop(invoke);
            Ok(None)
        }
    }) {
        Ok(Ok(Some(value))) => Ok(value),
        Ok(Ok(None)) => Err(CallbackFailure::Disabled),
        Ok(Err(refusal)) => {
            log::warn!("{kind} callback refused: {refusal}");
            Err(CallbackFailure::Refused(refusal))
        }
        Err(diagnostic) => {
            if let Some((_, poisoned)) = state {
                poisoned.store(true, Ordering::Release);
            }
            Err(CallbackFailure::Panicked(diagnostic))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panicking_payload_destructor_preserves_the_original_callback_diagnostic() {
        use std::sync::Arc;

        struct PanicOnDrop(Arc<AtomicBool>);
        impl Drop for PanicOnDrop {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Relaxed);
                panic!("secondary panic while dropping the callback payload");
            }
        }

        let dropped = Arc::new(AtomicBool::new(false));
        let poisoned = AtomicBool::new(false);
        let result: Result<(), _> = call("owner", 0, &poisoned, "test", "test", || {
            std::panic::panic_any(PanicOnDrop(dropped.clone()));
        });
        assert!(dropped.load(Ordering::Relaxed));
        assert_eq!(
            result.unwrap_err(),
            "test callback panicked; it is disabled: Box<dyn Any>"
        );
        assert!(poisoned.load(Ordering::Acquire));
    }
}
