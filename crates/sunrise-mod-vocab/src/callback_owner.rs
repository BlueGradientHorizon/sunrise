//! Owner enablement, attribution and panic containment for retained resources.
use std::any::Any;
use std::collections::VecDeque;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

/// A retained mod callback panicked and was disabled for the rest of this session.
// no wire: in-process notification drained by the client's error toast queue.
// exhaustive: engine-only notification record produced by `callback_owner::call`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallbackPanicNotice {
    /// The `[mod] id` whose callback panicked.
    pub mod_id: &'static str,
    /// Callback category (for example `"menu screen"` or `"debug entry"`).
    pub kind: &'static str,
    /// Specific callback operation (for example `"menu_screen_draw"`).
    pub operation: &'static str,
    /// Panic message payload, if it was a string.
    pub message: String,
}

static PANIC_NOTICES: Mutex<VecDeque<CallbackPanicNotice>> = Mutex::new(VecDeque::new());

/// Drain pending callback panic notices queued since the last call.
pub fn take_callback_panic_notices() -> Vec<CallbackPanicNotice> {
    let mut queue = PANIC_NOTICES
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    queue.drain(..).collect()
}

fn record_callback_panic(
    owner: &'static str,
    kind: &'static str,
    operation: &'static str,
    message: String,
) {
    PANIC_NOTICES
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .push_back(CallbackPanicNotice {
            mod_id: owner,
            kind,
            operation,
            message,
        });
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(msg) = payload.downcast_ref::<&str>() {
        (*msg).to_owned()
    } else if let Some(msg) = payload.downcast_ref::<String>() {
        msg.clone()
    } else {
        "panic".to_owned()
    }
}

/// Owner identity and poison state for one retained mod callback.
// no wire: the callback's enablement and poison state belong to this process.
// exhaustive: engine-only callback bookkeeping; private fields prevent downstream construction.
pub struct CallbackOwner {
    id: &'static str,
    index: Option<usize>,
    poisoned: AtomicBool,
}

impl CallbackOwner {
    /// Bind a dispatch-attributed registration to its installed mod slot once, at freeze time.
    pub fn new(id: &'static str) -> Option<Self> {
        Some(Self {
            id,
            index: Some(crate::__private::mod_index(id)?),
            poisoned: AtomicBool::new(false),
        })
    }

    /// Bind a callback owner by mod id, resolving its installed slot when present.
    pub fn for_mod(id: &'static str) -> Self {
        Self {
            id,
            index: crate::__private::mod_index(id),
            poisoned: AtomicBool::new(false),
        }
    }

    /// The owning mod's identifier.
    pub fn id(&self) -> &'static str {
        self.id
    }

    /// Whether this callback is still enabled for this session.
    pub fn is_enabled(&self) -> bool {
        enabled_opt(self.index, &self.poisoned)
    }

    /// Invoke retained code with its owner's enablement, attribution and panic policy.
    pub fn call<T>(
        &self,
        kind: &'static str,
        operation: &'static str,
        invoke: impl FnOnce() -> T,
    ) -> Result<T, String> {
        call_opt(self.id, self.index, &self.poisoned, kind, operation, invoke)
            .map_err(|why| format!("mod {}: {why}", self.id))
    }
}

pub(crate) fn enabled(index: usize, poisoned: &AtomicBool) -> bool {
    enabled_opt(Some(index), poisoned)
}

fn enabled_opt(index: Option<usize>, poisoned: &AtomicBool) -> bool {
    index.is_none_or(crate::mod_index_on) && !poisoned.load(Ordering::Acquire)
}

pub(crate) fn call<T>(
    owner: &'static str,
    index: usize,
    poisoned: &AtomicBool,
    kind: &'static str,
    operation: &'static str,
    invoke: impl FnOnce() -> T,
) -> Result<T, String> {
    call_opt(owner, Some(index), poisoned, kind, operation, invoke)
}

fn call_opt<T>(
    owner: &'static str,
    index: Option<usize>,
    poisoned: &AtomicBool,
    kind: &'static str,
    operation: &'static str,
    invoke: impl FnOnce() -> T,
) -> Result<T, String> {
    if !enabled_opt(index, poisoned) {
        return Err(format!("{kind} owner is disabled"));
    }
    let _guard = crate::mark_mod(owner, operation);
    // A refusal by the phase gate refuses this call and nothing more; a real panic disables
    // this callback for the session, logs loudly and notifies the client error toast queue.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::world::phase_refusal::contain_refusal(owner, invoke)
    })) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(refusal)) => {
            refusal.report();
            Err(format!("{kind} callback refused: {refusal}"))
        }
        Err(payload) => {
            poisoned.store(true, Ordering::Release);
            let msg = panic_message(payload.as_ref());
            log::error!(
                "mod {owner}: {kind} callback ({operation}) panicked and is disabled for this session: {msg}"
            );
            record_callback_panic(owner, kind, operation, msg.clone());
            Err(format!("{kind} callback panicked ({msg}); it is disabled"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::phase::ModPhase;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn panic_disables_only_that_callback_and_enqueues_notice_while_refusal_never_latches() {
        let _ = take_callback_panic_notices();

        let bad = CallbackOwner::for_mod("panicky_mod");
        let sibling = CallbackOwner::for_mod("panicky_mod");
        let runs = AtomicUsize::new(0);

        let err = bad
            .call("debug entry", "debug_entry_callback", || {
                runs.fetch_add(1, Ordering::Relaxed);
                panic!("boom in callback");
            })
            .unwrap_err();
        assert!(err.contains("panicky_mod"), "{err}");
        assert!(err.contains("boom in callback"), "{err}");
        assert!(!bad.is_enabled());
        assert_eq!(runs.load(Ordering::Relaxed), 1);

        // Disabled for the session: later calls do not run the closure again.
        assert!(
            bad.call("debug entry", "debug_entry_callback", || {
                runs.fetch_add(1, Ordering::Relaxed);
            })
            .is_err()
        );
        assert_eq!(runs.load(Ordering::Relaxed), 1);

        // Another callback of the same mod is unaffected.
        assert!(sibling.is_enabled());
        assert_eq!(sibling.call("debug entry", "other_callback", || 42), Ok(42));

        let notices = take_callback_panic_notices();
        let notice = notices
            .iter()
            .find(|n| n.mod_id == "panicky_mod")
            .expect("panic notice queued");
        assert_eq!(notice.kind, "debug entry");
        assert_eq!(notice.operation, "debug_entry_callback");
        assert_eq!(notice.message, "boom in callback");

        // A phase refusal reports on every invocation, does not poison the callback,
        // and does not enqueue a panic notice.
        let refusable = CallbackOwner::for_mod("refused_mod");
        let attempts = AtomicUsize::new(0);
        for _ in 0..2 {
            let err = refusable
                .call("menu screen", "menu_screen_draw", || {
                    attempts.fetch_add(1, Ordering::Relaxed);
                    crate::world::phase_refusal::refuse("level", ModPhase::Setup);
                })
                .unwrap_err();
            assert!(err.contains("refused"), "{err}");
            assert!(refusable.is_enabled());
        }
        assert_eq!(attempts.load(Ordering::Relaxed), 2);
        assert!(
            take_callback_panic_notices()
                .iter()
                .all(|n| n.mod_id != "refused_mod")
        );
        // Once the callback succeeds, it runs normally without having latched.
        assert_eq!(
            refusable.call("menu screen", "menu_screen_draw", || 7),
            Ok(7)
        );
    }
}
