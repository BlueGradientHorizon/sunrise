use crate::ClientHost;
use std::sync::Arc;

type Task = Box<dyn FnOnce(&mut dyn ClientHost) + Send>;
type Submit = dyn Fn(Task) + Send + Sync;

/// A handle to the client's task drain, obtained from `host.client_executor()`.
/// Clone it into workers; it never borrows a host or a connection.
// no wire: submits closures to the live client in this process.
// exhaustive: its field is private, so mods cannot build or destructure it.
#[derive(Clone)]
pub struct ClientExecutor(Arc<Submit>);

impl ClientExecutor {
    /// Game-side constructor for the calling mod's executor.
    #[doc(hidden)]
    pub fn new(submit: impl Fn(Task) + Send + Sync + 'static) -> Self {
        Self(Arc::new(submit))
    }

    /// Queues `task` for the next client-thread drain, even when called on the client
    /// thread. Tasks run in submission order with a fresh host. Move owned data into
    /// the task, not the caller's host or payload context. The task is dropped if the
    /// client stops before it can run. A task that panics is reported and disables further
    /// executor callbacks from its mod for this client session, not other mods' tasks.
    pub fn execute(&self, task: Box<dyn FnOnce(&mut dyn ClientHost) + Send>) {
        (self.0)(task);
    }
}
