//! Owner enablement, attribution and panic containment for retained resources.
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

static MOD_ERROR_TOASTS: Mutex<Vec<(&'static str, &'static str)>> = Mutex::new(Vec::new());
static HAS_TOAST_MANAGER: AtomicBool = AtomicBool::new(true);

/// Disable toast storage at process startup when there is no client.
pub fn set_mod_error_toasts_enabled(enabled: bool) {
    HAS_TOAST_MANAGER.store(enabled, Ordering::Relaxed);
}

/// Report a contained callback panic to the log and queue its mod error toast.
pub(crate) fn report_mod_panic(
    mod_id: &'static str,
    kind: impl std::fmt::Display,
    operation: &'static str,
) {
    log::error!("mod {mod_id}: {kind} `{operation}` panicked; it is disabled for this session");
    queue_mod_error_toast(mod_id, operation);
}

/// Queue a mod error for an in-process client's toast manager.
pub fn queue_mod_error_toast(mod_id: &'static str, operation: &'static str) {
    if !HAS_TOAST_MANAGER.load(Ordering::Relaxed) {
        return;
    }
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

#[cfg(test)]
pub(crate) fn assert_toast_count(owner: &str, operation: &str, expected: usize) {
    let toasts = MOD_ERROR_TOASTS.lock().unwrap();
    assert_eq!(
        toasts
            .iter()
            .filter(|&&(id, op)| id == owner && op == operation)
            .count(),
        expected
    );
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
    contain(
        owner,
        kind,
        operation,
        || poisoned.store(true, Ordering::Release),
        invoke,
    )
}

/// Run a callback with an explicit owner and the caller's existing poison slot.
/// A phase refusal is reported but never poisons the slot.
pub(crate) fn contain<T>(
    owner: &'static str,
    kind: impl std::fmt::Display,
    operation: &'static str,
    poison: impl FnOnce(),
    invoke: impl FnOnce() -> T,
) -> Result<T, String> {
    let _contained = crate::content::crash::enter_contained();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::world::phase_refusal::contain_refusal(owner, invoke)
    })) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(refusal)) => {
            log::warn!("mod {owner}: {kind} `{operation}` refused: {refusal}");
            Err(format!("{kind} callback refused: {refusal}"))
        }
        Err(_) => {
            poison();
            report_mod_panic(owner, kind, operation);
            Err(format!("{kind} callback panicked; it is disabled"))
        }
    }
}

/// A named callback that can be rebuilt (a resource reload or a new connection).
/// Its poison is owned by the installed session, not by the short-lived instance.
#[doc(hidden)]
pub fn session_callback_enabled(
    hook: crate::HookId,
    owner: &'static str,
    id: &crate::Identifier,
) -> bool {
    crate::runtime::installed().is_some_and(|installed| {
        installed
            .by_id
            .get(owner)
            .is_some_and(|&index| crate::mod_index_on(index))
            && !installed
                .named_poison
                .lock()
                .unwrap()
                .get(&(hook as usize, owner))
                .is_some_and(|poisoned| poisoned.contains(id))
    })
}

/// Invoke one named callback, attributing and remembering a panic across instances.
#[doc(hidden)]
pub fn call_session_callback<T>(
    hook: crate::HookId,
    owner: &'static str,
    id: &crate::Identifier,
    operation: &'static str,
    invoke: impl FnOnce() -> T,
) -> Result<T, String> {
    let _guard = crate::mark_mod(owner, operation);
    contain(
        owner,
        id,
        operation,
        || {
            if let Some(installed) = crate::runtime::installed() {
                installed
                    .named_poison
                    .lock()
                    .unwrap()
                    .entry((hook as usize, owner))
                    .or_default()
                    .insert(id.clone());
            }
        },
        invoke,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::phase::ModPhase;
    use std::sync::atomic::AtomicUsize;

    struct Quiet;
    #[mod_api::__subscriptions]
    impl crate::Hooks for Quiet {}

    #[test]
    fn phase_refusal_does_not_poison_or_queue_toast() {
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
                assert_toast_count("refusal_mod", "refusal_op", 0);
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
                assert_toast_count("panic_mod", "panic_op", 1);
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
