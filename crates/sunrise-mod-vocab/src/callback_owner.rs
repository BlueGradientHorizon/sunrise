//! Owner enablement, attribution and panic containment for retained resources.
use crate::runtime::{DispatchOutcome, invoke_callback, log_and_queue_panic};
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
    if !enabled(index, poisoned) {
        return Err(format!("{kind} owner is disabled"));
    }
    let _guard = crate::mark_mod(owner, operation);
    match invoke_callback(
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
            Err(format!("{kind} callback refused: {refusal}"))
        }
        DispatchOutcome::Panicked => Err(format!("{kind} callback panicked; it is disabled")),
    }
}
