//! Owner enablement, attribution and panic containment for retained resources.
use crate::runtime::{DispatchOutcome, invoke_guarded, log_and_queue_panic};
use std::sync::atomic::{AtomicBool, Ordering};

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
        Some(Self {
            id,
            index: crate::__private::mod_index(id)?,
            poisoned: AtomicBool::new(false),
        })
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
    call_typed(owner, index, poisoned, kind, operation, invoke).map_err(|error| match error {
        CallbackFailure::Refused(reason) => reason,
        CallbackFailure::Panicked => format!("{kind} callback panicked; it is disabled"),
        CallbackFailure::Poisoned => format!("{kind} owner is disabled"),
    })
}

/// Failure of a retained callback, distinct from the result returned by the mod itself.
// no wire: an in-process dispatch outcome; never sent to the peer.
// exhaustive: engine-only dispatch result.
#[derive(Debug)]
pub enum CallbackFailure {
    /// A refusal, not a panic.
    Refused(String),
    /// The callback just panicked and the shared boundary reported it.
    Panicked,
    /// A previous panic disabled the callback; it did not run.
    Poisoned,
}

pub(crate) fn call_typed<T>(
    owner: &'static str,
    index: usize,
    poisoned: &AtomicBool,
    kind: &'static str,
    operation: &'static str,
    invoke: impl FnOnce() -> T,
) -> Result<T, CallbackFailure> {
    if poisoned.load(Ordering::Acquire) {
        return Err(CallbackFailure::Poisoned);
    }
    if !crate::mod_index_on(index) {
        return Err(CallbackFailure::Refused(format!(
            "{kind} owner is disabled"
        )));
    }
    let _guard = crate::mark_mod(owner, operation);
    match invoke_guarded(
        || crate::world::phase_refusal::contain_refusal(owner, invoke),
        |payload| {
            if !poisoned.swap(true, Ordering::AcqRel) {
                log_and_queue_panic(operation, owner, payload);
            }
        },
    ) {
        DispatchOutcome::Ok(value) => Ok(value),
        DispatchOutcome::Refused(refusal) => {
            refusal.report();
            Err(CallbackFailure::Refused(format!(
                "{kind} callback refused: {refusal}"
            )))
        }
        DispatchOutcome::Panicked => Err(CallbackFailure::Panicked),
    }
}
