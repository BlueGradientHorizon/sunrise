//! Owner enablement, attribution and panic containment for retained resources.
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

static PANIC_NOTICES: Mutex<Vec<(&'static str, &'static str)>> = Mutex::new(Vec::new());

/// Drain every `(mod_id, callback_kind)` panic notice recorded since the last call.
pub fn take_callback_panic_notices() -> Vec<(&'static str, &'static str)> {
    std::mem::take(
        &mut *PANIC_NOTICES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    )
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
        Some(Self {
            id,
            index: crate::__private::mod_index(id)?,
            poisoned: AtomicBool::new(false),
        })
    }

    /// The owning mod's ID.
    pub fn id(&self) -> &'static str {
        self.id
    }

    /// Whether this callback is still enabled for the session.
    pub fn is_enabled(&self) -> bool {
        enabled(self.index, &self.poisoned)
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
    // A refusal by the phase gate refuses this call and nothing more; a real panic disables the owner.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::world::phase_refusal::contain_refusal(owner, invoke)
    })) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(refusal)) => {
            refusal.report();
            Err(format!("{kind} callback refused: {refusal}"))
        }
        Err(_) => {
            poisoned.store(true, Ordering::Release);
            log::error!(
                "Sunrise: mod {owner}: {kind} callback ({operation}) panicked and was disabled for this session"
            );
            PANIC_NOTICES
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push((owner, kind));
            Err(format!("{kind} callback panicked; it is disabled"))
        }
    }
}
