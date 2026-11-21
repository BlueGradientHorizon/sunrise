//! Owner enablement, attribution and panic containment for retained resources.
use crate::runtime::{self, DispatchOutcome};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Owner identity and poison state for one retained mod callback.
// no wire: the callback's enablement and poison state belong to this process.
// exhaustive: engine-only callback bookkeeping; private fields prevent downstream construction.
pub struct CallbackOwner {
    id: &'static str,
    index: Option<usize>,
    poisoned: Arc<AtomicBool>,
}

impl CallbackOwner {
    /// Resolve the installed mod owning this callback.
    pub fn new(id: &'static str) -> Option<Self> {
        Some(Self {
            id,
            index: Some(crate::__private::mod_index(id)?),
            poisoned: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Bind a callback owner by mod id, resolving its installed slot when present.
    pub fn for_mod(id: &'static str) -> Self {
        Self::with_shared_poison(id, Arc::default())
    }

    /// Bind a callback owner by mod id with session-persistent poison state shared across refreshes.
    pub fn with_shared_poison(id: &'static str, poisoned: Arc<AtomicBool>) -> Self {
        Self {
            id,
            index: crate::__private::mod_index(id),
            poisoned,
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
    match runtime::invoke_callback(true, invoke, |payload| {
        if !poisoned.swap(true, Ordering::AcqRel) {
            runtime::log_and_queue_panic(operation, owner, payload);
        }
    }) {
        DispatchOutcome::Ok(value) => Ok(value),
        DispatchOutcome::Refused(refusal) => {
            refusal.report();
            Err(format!("{kind} callback refused: {refusal}"))
        }
        DispatchOutcome::Panicked => Err(format!("{kind} callback panicked; it is disabled")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::__private::{HookPanicReport, take_hook_panics};
    use crate::world::phase::ModPhase;

    #[test]
    fn retained_callback_reports_through_hook_diagnostics_and_only_panics_latch() {
        crate::testing::with_dispatch(Vec::new().into(), || {
            let bad = CallbackOwner::for_mod("screen_mod");
            let sibling = CallbackOwner::for_mod("screen_mod");
            assert!(bad.call::<()>("screen", "draw", || panic!("boom")).is_err());
            assert!(!bad.is_enabled());
            assert!(bad.call("screen", "draw", || ()).is_err());
            assert_eq!(sibling.call("screen", "click", || 42), Ok(42));
            assert_eq!(
                take_hook_panics(),
                [HookPanicReport {
                    mod_id: "screen_mod",
                    hook: "draw",
                }]
            );

            let mut attempts = 0;
            for _ in 0..2 {
                let err = sibling
                    .call::<()>("screen", "click", || {
                        attempts += 1;
                        crate::world::phase_refusal::refuse("level", ModPhase::Setup);
                    })
                    .unwrap_err();
                assert!(err.contains("refused"), "{err}");
            }
            assert_eq!(attempts, 2);
            assert!(take_hook_panics().is_empty());
            assert_eq!(sibling.call("screen", "click", || 7), Ok(7));
        });
    }
}
