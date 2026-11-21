//! Owner enablement, attribution and panic containment for retained resources.
use crate::world::phase_refusal::{self, PhaseRefusal};
use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::{any::Any, cell::Cell};

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

thread_local! {
    static RETAINED_CALLBACK: Cell<bool> = const { Cell::new(false) };
}

/// Whether the current panic will be reported and contained by the retained callback owner.
pub fn retained_callback_active() -> bool {
    RETAINED_CALLBACK.try_with(Cell::get).unwrap_or(false)
}

// A delegated engine panic has already reached the fatal panic hook. Pass it through
// nested wrappers without turning it into a mod failure or reporting it a second time.
struct EnginePanic(Box<dyn Any + Send>);

struct RetainedCallbackScope(bool);
impl Drop for RetainedCallbackScope {
    fn drop(&mut self) {
        RETAINED_CALLBACK.set(self.0);
    }
}

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
#[derive(Clone, Debug)]
pub struct CallbackOwner {
    id: &'static str,
    index: Option<usize>,
    poisoned: Arc<AtomicBool>,
}

impl CallbackOwner {
    /// Bind a dispatch-attributed registration to its installed mod slot once, at freeze time.
    pub fn new(id: &'static str) -> Option<Self> {
        Some(Self {
            id,
            index: Some(crate::__private::mod_index(id)?),
            poisoned: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Bind a callback owner by mod id, resolving its installed slot when present.
    pub fn for_mod(id: &'static str) -> Self {
        Self {
            id,
            index: crate::__private::mod_index(id),
            poisoned: Arc::new(AtomicBool::new(false)),
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
        call_opt(
            self.id,
            self.index,
            &self.poisoned,
            kind,
            operation,
            false,
            invoke,
        )
        .map_err(|why| format!("mod {}: {why}", self.id))
    }

    /// A retained callback invoked by a generated registration listener shares that
    /// listener's phase scope so a phase refusal can roll back its staged registrations.
    pub(crate) fn call_in_listener<T>(
        &self,
        kind: &'static str,
        operation: &'static str,
        invoke: impl FnOnce() -> T,
    ) -> Result<T, String> {
        call_opt(
            self.id,
            self.index,
            &self.poisoned,
            kind,
            operation,
            true,
            invoke,
        )
        .map_err(|why| format!("mod {}: {why}", self.id))
    }

    /// Execute the engine body delegated by a retained wrapper, outside all mod scopes.
    pub fn call_engine<T>(invoke: impl FnOnce() -> T) -> T {
        if !retained_callback_active() {
            return invoke();
        }
        let _contained = RetainedCallbackScope(RETAINED_CALLBACK.replace(false));
        let _guard = crate::dispatch_scope::ModGuard::unattributed();
        let outer = phase_refusal::suspend_scope();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(invoke));
        let _ = phase_refusal::leave_scope(outer);
        result.unwrap_or_else(|payload| std::panic::resume_unwind(Box::new(EnginePanic(payload))))
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
    call_opt(owner, Some(index), poisoned, kind, operation, false, invoke)
}

fn call_opt<T>(
    owner: &'static str,
    index: Option<usize>,
    poisoned: &AtomicBool,
    kind: &'static str,
    operation: &'static str,
    in_listener: bool,
    invoke: impl FnOnce() -> T,
) -> Result<T, String> {
    if !enabled_opt(index, poisoned) {
        return Err(format!("{kind} owner is disabled"));
    }
    let _guard = crate::mark_mod(owner, operation);
    let contained = RetainedCallbackScope(RETAINED_CALLBACK.replace(true));
    // A refusal by the phase gate refuses this call and nothing more; a real panic disables
    // this callback for the session, logs loudly and notifies the client error toast queue.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if in_listener {
            Ok(invoke())
        } else {
            phase_refusal::contain_refusal(owner, invoke)
        }
    })) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(refusal)) => {
            refusal.report_each();
            Err(format!("{kind} callback refused: {refusal}"))
        }
        Err(payload) => {
            let payload = match payload.downcast::<EnginePanic>() {
                Ok(engine) if !contained.0 => std::panic::resume_unwind(engine.0),
                Ok(engine) => std::panic::resume_unwind(engine),
                Err(payload) => payload,
            };
            if in_listener && payload.is::<PhaseRefusal>() {
                // The enclosing listener's scope already latched this refusal.
                std::panic::resume_unwind(payload);
            }
            let first = !poisoned.swap(true, Ordering::AcqRel);
            let msg = panic_message(payload.as_ref());
            if let Err(payload) =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(payload)))
            {
                std::mem::forget(payload);
            }
            if !first {
                return Err(format!("{kind} callback panicked; it is disabled"));
            }
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
    fn identical_phase_refusals_are_each_reported_without_disabling_the_callback() {
        const OWNER: &str = "test_repeated_retained_refusal";
        const OPERATION: &str = "test_retained_phase_gate";
        struct RefusalLog(AtomicUsize);
        impl log::Log for RefusalLog {
            fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
                metadata.level() == log::Level::Warn
            }
            fn log(&self, record: &log::Record<'_>) {
                if self.enabled(record.metadata()) {
                    let message = record.args().to_string();
                    if message.contains(OWNER) && message.contains(OPERATION) {
                        self.0.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            fn flush(&self) {}
        }
        static LOG: RefusalLog = RefusalLog(AtomicUsize::new(0));
        log::set_logger(&LOG).unwrap();
        log::set_max_level(log::LevelFilter::Warn);

        let owner = CallbackOwner::for_mod(OWNER);
        for reports in [1, 2] {
            assert!(
                owner
                    .call::<()>("test", "retained_test", || {
                        phase_refusal::refuse(OPERATION, ModPhase::Setup);
                    })
                    .is_err()
            );
            assert_eq!(LOG.0.load(Ordering::Relaxed), reports);
            assert!(owner.is_enabled());
        }
        assert_eq!(owner.call("test", "retained_test", || 17), Ok(17));
    }
}
