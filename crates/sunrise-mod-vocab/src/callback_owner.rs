//! Owner enablement, attribution and panic containment for retained resources.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::{
    any::Any,
    cell::{Cell, RefCell},
};

thread_local! {
    static RETAINED_CALLBACK: Cell<bool> = const { Cell::new(false) };
    // Keep the original fatal payload in the engine-owned invocation, not in the
    // unwind a wrapper is free to catch. It is taken on exit from the outermost wrapper.
    static DELEGATED_ENGINE_PANIC: RefCell<Option<Box<dyn Any + Send>>> = const { RefCell::new(None) };
}

/// Whether the current panic will be reported and contained by the retained callback owner.
pub fn retained_callback_active() -> bool {
    RETAINED_CALLBACK.try_with(Cell::get).unwrap_or(false)
}

// Interrupt wrapper code without invoking the panic hook a second time.
fn unwind_through_wrapper() -> ! {
    struct DelegatedEnginePanic;
    std::panic::resume_unwind(Box::new(DelegatedEnginePanic))
}

fn engine_panic_pending() -> bool {
    DELEGATED_ENGINE_PANIC.with(|panic| panic.borrow().is_some())
}

fn drop_contained<T>(value: T) {
    if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(value))) {
        // A panic payload can itself have a panicking destructor.
        std::mem::forget(payload);
    }
}

#[derive(PartialEq, Eq)]
enum Invocation {
    Isolated,
    Dispose,
}

struct RetainedCallbackScope(bool);
impl Drop for RetainedCallbackScope {
    fn drop(&mut self) {
        RETAINED_CALLBACK.set(self.0);
    }
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
            Invocation::Isolated,
            invoke,
        )
        .map_err(|why| format!("mod {}: {why}", self.id))
    }

    /// Dispose of an owned mod value under its panic policy, even if ordinary calls are disabled.
    pub fn dispose<T>(&self, kind: &'static str, operation: &'static str, value: T) {
        let _ = call_opt(
            self.id,
            self.index,
            &self.poisoned,
            kind,
            operation,
            Invocation::Dispose,
            || drop(value),
        );
    }

    /// Execute the engine body delegated by a retained wrapper, outside all mod scopes.
    pub fn call_engine<T>(invoke: impl FnOnce() -> T) -> T {
        if !retained_callback_active() {
            return invoke();
        }
        if engine_panic_pending() {
            unwind_through_wrapper();
        }
        let _contained = RetainedCallbackScope(RETAINED_CALLBACK.replace(false));
        let _guard = crate::dispatch_scope::ModGuard::unattributed();
        let result = crate::runtime::without_callback_scope(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(invoke))
        });
        result.unwrap_or_else(|payload| {
            DELEGATED_ENGINE_PANIC.set(Some(payload));
            unwind_through_wrapper();
        })
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
    call_opt(
        owner,
        Some(index),
        poisoned,
        kind,
        operation,
        Invocation::Isolated,
        invoke,
    )
}

fn call_opt<T>(
    owner: &'static str,
    index: Option<usize>,
    poisoned: &AtomicBool,
    kind: &'static str,
    operation: &'static str,
    invocation: Invocation,
    invoke: impl FnOnce() -> T,
) -> Result<T, String> {
    if invocation != Invocation::Dispose && !enabled_opt(index, poisoned) {
        return Err(format!("{kind} owner is disabled"));
    }
    let _guard = crate::mark_mod(owner, operation);
    let contained = RetainedCallbackScope(RETAINED_CALLBACK.replace(true));
    let mut failure = None;
    let result = crate::runtime::contain_callback(invoke, |payload| {
        if engine_panic_pending() {
            return;
        }
        let first = !poisoned.swap(true, Ordering::AcqRel);
        if first || invocation == Invocation::Dispose {
            crate::runtime::log_and_queue_panic(operation, owner, payload);
        }
        failure = Some(panic_message(payload));
    });
    if engine_panic_pending() {
        drop_contained(result);
        if contained.0 {
            unwind_through_wrapper();
        }
        let payload = DELEGATED_ENGINE_PANIC.take().expect("pending engine panic");
        std::panic::resume_unwind(payload);
    }
    result.ok_or_else(|| {
        let message = failure.unwrap_or_else(|| "panic".to_owned());
        format!("{kind} callback panicked ({message}); it is disabled")
    })
}
