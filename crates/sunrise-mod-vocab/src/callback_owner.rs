//! Owner enablement, attribution and panic containment for retained resources.
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

static MOD_ERROR_TOASTS: Mutex<Vec<(&'static str, &'static str)>> = Mutex::new(Vec::new());

pub(crate) fn report_mod_panic(mod_id: &'static str, kind: &'static str, operation: &'static str) {
    log::error!("mod {mod_id}: {kind} `{operation}` panicked; it is disabled for this session");
    MOD_ERROR_TOASTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push((mod_id, operation));
}

/// Drain pending `(mod_id, operation)` panic reports to show as in-game error toasts.
pub fn take_mod_error_toasts() -> Vec<(&'static str, &'static str)> {
    let mut guard = MOD_ERROR_TOASTS.lock().unwrap_or_else(|e| e.into_inner());
    std::mem::take(&mut *guard)
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
    let result = {
        let _contained = crate::content::crash::enter_contained();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::world::phase_refusal::contain_refusal(owner, invoke)
        }))
    };
    match result {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(refusal)) => {
            log::warn!("mod {owner}: {kind} `{operation}` refused: {refusal}");
            Err(format!("{kind} callback refused: {refusal}"))
        }
        Err(_) => {
            poisoned.store(true, Ordering::Release);
            report_mod_panic(owner, kind, operation);
            Err(format!("{kind} callback panicked; it is disabled"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::phase::ModPhase;
    use std::sync::atomic::AtomicUsize;

    static TOAST_LOCK: Mutex<()> = Mutex::new(());

    struct Quiet;
    #[mod_api::__subscriptions]
    impl crate::Hooks for Quiet {}

    #[test]
    fn phase_refusal_does_not_poison_or_queue_toast() {
        let _lock = TOAST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _ = take_mod_error_toasts();
        crate::testing::with_dispatch(
            vec![crate::ModEntry::new("refusal_mod", Box::new(Quiet))].into(),
            || {
                let owner = CallbackOwner::new("refusal_mod").unwrap();
                let calls = AtomicUsize::new(0);
                let refused = owner.call("callback", "refusal_op", || {
                    calls.fetch_add(1, Ordering::Relaxed);
                    crate::world::phase_refusal::refuse("level", ModPhase::Setup);
                });
                assert!(refused.is_err());
                assert!(
                    !take_mod_error_toasts()
                        .iter()
                        .any(|&(id, _)| id == "refusal_mod"),
                    "phase refusal must not queue an error toast"
                );
                let ok = owner.call("callback", "refusal_op", || {
                    calls.fetch_add(1, Ordering::Relaxed);
                    42
                });
                assert_eq!(ok, Ok(42));
                assert_eq!(calls.load(Ordering::Relaxed), 2);
            },
        );
    }

    #[test]
    fn panic_poisons_owner_and_queues_error_toast() {
        let _lock = TOAST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _ = take_mod_error_toasts();
        crate::testing::with_dispatch(
            vec![crate::ModEntry::new("panic_mod", Box::new(Quiet))].into(),
            || {
                let owner = CallbackOwner::new("panic_mod").unwrap();
                let calls = AtomicUsize::new(0);
                let panicked = owner.call::<()>("callback", "panic_op", || {
                    calls.fetch_add(1, Ordering::Relaxed);
                    panic!("callback boom");
                });
                assert!(panicked.is_err());
                assert_eq!(calls.load(Ordering::Relaxed), 1);
                let toasts = take_mod_error_toasts();
                assert!(
                    toasts.contains(&("panic_mod", "panic_op")),
                    "expected error toast for panic_mod, got {toasts:?}"
                );
                let after = owner.call("callback", "panic_op", || {
                    calls.fetch_add(1, Ordering::Relaxed);
                    99
                });
                assert!(after.is_err());
                assert_eq!(
                    calls.load(Ordering::Relaxed),
                    1,
                    "poisoned owner must not run subsequent calls"
                );
            },
        );
    }
}
